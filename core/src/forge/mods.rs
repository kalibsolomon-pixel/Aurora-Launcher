//! Bounded Forge `META-INF/mods.toml` inspection. This never extracts or
//! executes a mod, and does not accept Fabric or NeoForge-only metadata.
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use crate::instance_mods::{ModMetadata, ModRelation, ModWarning};

const MAX_JAR_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_METADATA_BYTES: u64 = 256 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    mod_loader: String,
    mods: Vec<Mod>,
    #[serde(default)]
    dependencies: BTreeMap<String, Vec<Dependency>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mod {
    mod_id: String,
    version: String,
    display_name: Option<String>,
    description: Option<String>,
    authors: Option<String>,
    logo_file: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    mod_id: String,
    #[serde(default = "required")]
    mandatory: bool,
    version_range: String,
    #[serde(default)]
    side: Option<String>,
}

fn required() -> bool {
    true
}

pub(crate) fn inspect(path: &Path, jar_size: u64) -> (Option<ModMetadata>, Vec<ModWarning>) {
    match read_metadata(path, jar_size) {
        Ok(metadata) => (Some(metadata), Vec::new()),
        Err(message) => (
            None,
            vec![ModWarning {
                code: "forge_metadata_invalid".into(),
                message,
            }],
        ),
    }
}

fn read_metadata(path: &Path, jar_size: u64) -> Result<ModMetadata, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Forge JAR is unreadable: {e}"))?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if jar_size > MAX_JAR_BYTES || size > MAX_JAR_BYTES {
        return Err("Forge JAR exceeds the 512 MiB inspection bound".into());
    }
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Forge JAR is malformed: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err("Forge JAR exceeds the 4,096-entry inspection bound".into());
    }
    let text = read_entry(&mut archive, "META-INF/mods.toml")?;
    let document: Document =
        toml::from_str(&text).map_err(|e| format!("Forge mods.toml is malformed: {e}"))?;
    if !matches!(document.mod_loader.as_str(), "javafml" | "lowcodefml") {
        return Err("Forge mods.toml declares an unsupported modLoader".into());
    }
    let [entry] = document.mods.as_slice() else {
        return Err("Forge mods.toml must declare exactly one [[mods]] entry".into());
    };
    if entry.mod_id.is_empty()
        || entry.mod_id.len() > 64
        || !entry
            .mod_id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err("Forge mods.toml has an invalid modId".into());
    }
    let version = if entry.version == "${file.jarVersion}" {
        let manifest = read_entry(&mut archive, "META-INF/MANIFEST.MF")?;
        manifest_version(&manifest)
            .ok_or("Forge JAR has no main Implementation-Version for ${file.jarVersion}")?
    } else {
        entry.version.clone()
    };
    if version.trim().is_empty() || version.contains("${") {
        return Err("Forge mods.toml has an empty or unresolved mod version".into());
    }
    let mut depends = Vec::new();
    let mut recommends = Vec::new();
    for dependency in document
        .dependencies
        .get(&entry.mod_id)
        .into_iter()
        .flatten()
    {
        match dependency.side.as_deref().unwrap_or("BOTH") {
            "SERVER" => continue,
            "CLIENT" | "BOTH" => {}
            _ => return Err("Forge mods.toml has an invalid dependency side".into()),
        }
        if dependency.mod_id.is_empty() || dependency.version_range.trim().is_empty() {
            return Err("Forge mods.toml has an incomplete dependency".into());
        }
        // Reject invalid Maven predicates before they can become capabilities.
        crate::neoforge::versions::satisfies("1", &dependency.version_range)
            .map_err(|e| format!("Forge dependency range is malformed: {e}"))?;
        let relation = ModRelation {
            mod_id: dependency.mod_id.clone(),
            requirement: dependency.version_range.clone(),
        };
        if dependency.mandatory {
            depends.push(relation);
        } else {
            recommends.push(relation);
        }
    }
    Ok(ModMetadata {
        id: entry.mod_id.clone(),
        name: entry.display_name.clone(),
        version: Some(version),
        description: entry.description.clone(),
        authors: entry
            .authors
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        environment: None,
        depends,
        recommends,
        suggests: Vec::new(),
        conflicts: Vec::new(),
        breaks: Vec::new(),
        has_declared_icon: entry.logo_file.is_some(),
        nested_mod_ids: Vec::new(),
        nested_mod_versions: HashMap::new(),
        mixin_java_requirements: Vec::new(),
    })
}

fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String, String> {
    let mut found = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if entry.name() == name {
            if found.replace(index).is_some() {
                return Err(format!("Forge JAR has duplicate {name} metadata"));
            }
        }
    }
    let mut entry = archive
        .by_index(found.ok_or_else(|| format!("Forge JAR has no {name}"))?)
        .map_err(|e| e.to_string())?;
    if entry.is_dir()
        || entry
            .unix_mode()
            .is_some_and(|m| !matches!(m & 0o170000, 0 | 0o100000))
        || entry.size() > MAX_METADATA_BYTES
    {
        return Err(format!(
            "Forge {name} is not a bounded regular metadata file"
        ));
    }
    let mut bytes = Vec::new();
    entry
        .by_ref()
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(format!("Forge {name} exceeds the 256 KiB inspection bound"));
    }
    String::from_utf8(bytes).map_err(|_| format!("Forge {name} is not UTF-8"))
}

fn manifest_version(text: &str) -> Option<String> {
    let mut attributes: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        if let Some(continuation) = line.strip_prefix(' ') {
            attributes.last_mut()?.push_str(continuation);
        } else {
            attributes.push(line.to_owned());
        }
    }
    let values: Vec<_> = attributes
        .iter()
        .filter_map(|line| line.split_once(": "))
        .filter(|(key, _)| key.eq_ignore_ascii_case("Implementation-Version"))
        .collect();
    match values.as_slice() {
        [(_, value)] if !value.trim().is_empty() => Some((*value).to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn forge_followup_metadata_requires_resolved_bounded_unambiguous_content() {
        let path =
            std::env::temp_dir().join(format!("aurora-forge-reader-{}.jar", uuid::Uuid::new_v4()));
        let write = |text: &str, manifest: Option<&str>| {
            let mut jar = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            jar.start_file(
                "META-INF/mods.toml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            jar.write_all(text.as_bytes()).unwrap();
            if let Some(manifest) = manifest {
                jar.start_file(
                    "META-INF/MANIFEST.MF",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
                jar.write_all(manifest.as_bytes()).unwrap();
            }
            jar.finish().unwrap();
        };
        let text = "modLoader=\"javafml\"\nloaderVersion=\"[36,)\"\n[[mods]]\nmodId=\"example\"\nversion=\"${file.jarVersion}\"\n";
        write(
            text,
            Some(
                "Manifest-Version: 1.0\r\nImplementation-Version: 2.\r\n 5\r\n\r\nName: example\r\nImplementation-Version: ignored\r\n",
            ),
        );
        assert_eq!(
            read_metadata(&path, 0).unwrap().version.as_deref(),
            Some("2.5")
        );
        for manifest in [
            None,
            Some("Manifest-Version: 1.0\n\nName: example\nImplementation-Version: 2.5\n"),
            Some("Implementation-Version: 2\nImplementation-Version: 3\n"),
            Some("Implementation-Version: ${unresolved}\n"),
        ] {
            write(text, manifest);
            let (metadata, warnings) = inspect(&path, 0);
            assert!(metadata.is_none());
            assert_eq!(warnings[0].code, "forge_metadata_invalid");
        }
        for invalid in [
            text.replace("${file.jarVersion}", "1")
                + "[[dependencies.example]]\nmodId=\"dep\"\nversionRange=\"[broken\"\n",
            text.replace("${file.jarVersion}", "1") + "[[mods]]\nmodId=\"second\"\nversion=\"1\"\n",
            "!malformed TOML".into(),
            "x".repeat(MAX_METADATA_BYTES as usize + 1),
        ] {
            write(&invalid, None);
            assert!(inspect(&path, 0).0.is_none());
        }
        std::fs::remove_file(path).unwrap();
    }
}
