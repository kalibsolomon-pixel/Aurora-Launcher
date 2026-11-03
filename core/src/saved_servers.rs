//! Read-only discovery of Minecraft's saved multiplayer server list
//! (`servers.dat`) inside managed instance game directories.
//!
//! Saved entries are destinations the player (or a synced profile) stored in
//! the vanilla list. Existence here is *not* gameplay evidence: nothing in
//! this module writes history, marks recency, or treats a saved address as
//! visited. Online status stays a separate, explicitly requested enrichment.

use crate::gameplay_history::ServerTarget;
use crate::instances::InstanceId;
use crate::paths::ManagedPaths;
use crate::{cosmetic_image, nbt};
use base64ct::{Base64, Encoding};

/// `servers.dat` stays small; one mebibyte bounds hostile or corrupt input.
const MAX_DOCUMENT: u64 = 1024 * 1024;
/// The saved list is bounded for presentation; longer lists still parse but
/// only the first entries are surfaced.
const MAX_ENTRIES: usize = 200;

#[derive(Debug)]
pub enum ReadError {
    Missing,
    Malformed,
    Redirected,
}

pub struct SavedServer {
    /// The player-chosen list label, sanitized for display.
    pub name: Option<String>,
    /// Canonical `host:port` destination.
    pub target: ServerTarget,
    /// Canonical PNG bytes when the stored icon decodes within the favicon
    /// policy; unreadable or oversized icons are simply absent.
    pub icon: Option<Vec<u8>>,
}

/// Reads one managed instance's saved server list. A missing file is an
/// empty list; malformed or redirected files are reported deliberately and
/// never rewritten.
pub fn read(paths: &ManagedPaths, instance: &InstanceId) -> Result<Vec<SavedServer>, ReadError> {
    use crate::install::coordination::checked_directory;
    let root = paths.instance_paths(instance);
    let directory = checked_directory(paths.data_root(), root.root()).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ReadError::Missing
        } else {
            ReadError::Redirected
        }
    })?;
    let bytes = nbt::read_file(&directory.join("servers.dat"), MAX_DOCUMENT).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ReadError::Missing
        } else {
            ReadError::Malformed
        }
    })?;
    parse(&bytes).ok_or(ReadError::Malformed)
}

fn parse(bytes: &[u8]) -> Option<Vec<SavedServer>> {
    if bytes.len() as u64 > MAX_DOCUMENT {
        return None;
    }
    let root = nbt::root(bytes).ok()?;
    let list = nbt::find(&root, "servers")?;
    if list.tag != 9 {
        return None;
    }
    let mut reader = nbt::reader_at(list.payload, 0);
    let entries = reader.compound_list().ok()?;
    let mut saved = Vec::new();
    for entry in &entries {
        if saved.len() == MAX_ENTRIES {
            break;
        }
        if nbt::find(entry, "hidden").is_some_and(|field| field.tag == 1 && field.payload != [0]) {
            continue;
        }
        let Some(address) = nbt::find(entry, "ip").and_then(|field| nbt::as_string(field).ok())
        else {
            continue;
        };
        let address = match std::str::from_utf8(address) {
            Ok(address) => address.to_owned(),
            Err(_) => continue,
        };
        let Ok(target) = ServerTarget::parse(address) else {
            // A stored address vanilla cannot join through this launcher is
            // skipped; other entries stay visible.
            continue;
        };
        let name = nbt::find(entry, "name")
            .and_then(|field| nbt::as_string(field).ok())
            .and_then(|bytes| simd_cesu8::mutf8::decode_strict(bytes).ok())
            .and_then(|text| {
                let clean = crate::launch::activity_bridge::sanitize(&text);
                (!clean.is_empty() && clean.len() <= 512).then_some(clean)
            });
        let icon = (saved.len() < 20)
            .then(|| {
                let field = nbt::find(entry, "icon")?;
                let decoded;
                let bytes = if let Ok(bytes) = nbt::as_byte_array(field) {
                    bytes
                } else {
                    let raw = nbt::as_string(field).ok()?;
                    let base64 = raw.strip_prefix(b"data:image/png;base64,").unwrap_or(raw);
                    if base64.len() > 65_535 {
                        return None;
                    }
                    decoded = Base64::decode_vec(std::str::from_utf8(base64).ok()?).ok()?;
                    &decoded
                };
                cosmetic_image::validate(bytes, cosmetic_image::FAVICON)
                    .map(|png| png.bytes().to_vec())
            })
            .flatten();
        saved.push(SavedServer { name, target, icon });
    }
    Some(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(value: &str) -> Vec<u8> {
        let bytes = value.as_bytes();
        let mut out = (bytes.len() as u16).to_be_bytes().to_vec();
        out.extend_from_slice(bytes);
        out
    }

    fn compound(fields: &[(&str, u8, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, tag, payload) in fields {
            let name = name.as_bytes();
            out.extend_from_slice(&[*tag]);
            out.extend_from_slice(&(name.len() as u16).to_be_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(payload);
        }
        out.push(0);
        out
    }

    fn document(entries: &[(&str, &str, Option<Vec<u8>>)]) -> Vec<u8> {
        let mut list = vec![10, 0, 0, 0, entries.len() as u8];
        for (name, address, icon) in entries {
            let mut fields = vec![("name", 8u8, string(name)), ("ip", 8u8, string(address))];
            if let Some(icon) = icon {
                let mut payload = (icon.len() as i32).to_be_bytes().to_vec();
                payload.extend_from_slice(icon);
                fields.push(("icon", 7, payload));
            }
            list.extend_from_slice(&compound(&fields));
        }
        let mut out = vec![10, 0, 0];
        out.extend_from_slice(&compound(&[("servers", 9, list)]));
        out
    }

    #[test]
    fn parses_saved_entries_and_skips_unjoinable_addresses() {
        let bytes = document(&[
            ("Survival", "example.invalid", None),
            ("Survival", "second.invalid:25565", None),
        ]);
        let saved = parse(&bytes).unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].target.as_str(), "example.invalid:25565");
        assert_eq!(saved[0].name.as_deref(), Some("Survival"));
        assert_eq!(saved[1].name.as_deref(), Some("Survival"));
        assert_eq!(saved[1].target.as_str(), "second.invalid:25565");

        let bytes = document(&[("Bad", "not a host", None)]);
        assert!(parse(&bytes).unwrap().is_empty());

        // Unknown/absent icon fields and non-list roots are handled.
        assert!(parse(&[10, 0, 0, 0]).is_none());
    }

    #[test]
    fn missing_file_is_empty_and_corrupt_file_is_malformed() {
        let root =
            std::env::temp_dir().join(format!("aurora-saved-servers-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let instance = InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        let instance_root = managed.instance_paths(&instance).root().to_path_buf();
        std::fs::create_dir_all(&instance_root).unwrap();

        assert!(matches!(read(&managed, &instance), Err(ReadError::Missing)));

        std::fs::write(instance_root.join("servers.dat"), b"not nbt at all").unwrap();
        assert!(matches!(
            read(&managed, &instance),
            Err(ReadError::Malformed)
        ));

        std::fs::write(
            instance_root.join("servers.dat"),
            document(&[("Home", "example.invalid", None)]),
        )
        .unwrap();
        let saved = read(&managed, &instance).unwrap();
        assert_eq!(saved.len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
