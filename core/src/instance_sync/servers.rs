//! Bounded, uncompressed named NBT server lists. Raw tag spans preserve unknown
//! fields and modified-UTF-8 bytes. Edits/ambiguous identities defer the file.
use super::*;

fn invalid() -> SyncError {
    fail(
        "sync_servers_invalid",
        "The server list is malformed or unsupported; it was left unchanged.",
    )
}
use crate::nbt::{Field, reader_at};

struct Server<'a> {
    name: &'a [u8],
    ip: &'a [u8],
    raw: &'a [u8],
}
struct List<'a> {
    prefix: &'a [u8],
    fields: Vec<Field<'a>>,
    entries: Vec<Server<'a>>,
}
fn parse(bytes: &[u8]) -> Result<List<'_>, SyncError> {
    if bytes.len() as u64 > MAX_OPTIONS {
        return Err(invalid());
    }
    let mut r = reader_at(bytes, 0);
    if r.byte().map_err(|_| invalid())? != 10 {
        return Err(invalid());
    }
    r.string().map_err(|_| invalid())?;
    let prefix = &bytes[..r.at];
    let fields = r.compound(0).map_err(|_| invalid())?;
    if r.at != bytes.len() {
        return Err(invalid());
    }
    let field = fields
        .iter()
        .find(|f| f.name == b"servers")
        .ok_or_else(invalid)?;
    if field.tag != 9 {
        return Err(invalid());
    }
    let mut r = reader_at(field.payload, 0);
    let tag = r.byte().map_err(|_| invalid())?;
    let count = r.count().map_err(|_| invalid())?;
    if count > 1024 || !(tag == 10 || (tag == 0 && count == 0)) {
        return Err(invalid());
    }
    let mut entries = Vec::new();
    for _ in 0..count {
        let start = r.at;
        let fields = r.compound(0).map_err(|_| invalid())?;
        let value = |key: &[u8]| -> Result<&[u8], SyncError> {
            let f = fields
                .iter()
                .find(|f| f.name == key && f.tag == 8)
                .ok_or_else(invalid)?;
            let mut r = reader_at(f.payload, 0);
            let value = r.string().map_err(|_| invalid())?;
            if value.is_empty() || value.len() > 4096 {
                return Err(invalid());
            }
            Ok(value)
        };
        entries.push(Server {
            name: value(b"name")?,
            ip: value(b"ip")?,
            raw: &field.payload[start..r.at],
        });
    }
    Ok(List {
        prefix,
        fields,
        entries,
    })
}
pub(super) struct Merged {
    pub bytes: Vec<u8>,
    pub conflicts: usize,
}
pub(super) fn merge(source: &[u8], target: Option<&[u8]>) -> Result<Merged, SyncError> {
    let source_list = parse(source)?;
    let target_list = target.map(parse).transpose()?;
    let local = target_list.as_ref().unwrap_or(&source_list);
    let mut entries: Vec<&Server<'_>> = local.entries.iter().collect();
    let mut conflicts = 0;
    // Duplicate identity within either list is ambiguous even when byte-equal.
    for list in [&source_list, local] {
        let mut identities = BTreeSet::new();
        for entry in &list.entries {
            if !identities.insert((entry.name, entry.ip)) {
                conflicts += 1;
            }
        }
    }
    if target.is_some() {
        for entry in &source_list.entries {
            if let Some(existing) = entries
                .iter()
                .find(|e| e.name == entry.name && e.ip == entry.ip)
            {
                if existing.raw != entry.raw {
                    conflicts += 1;
                }
            } else if entries
                .iter()
                .any(|e| e.name == entry.name || e.ip == entry.ip)
            {
                conflicts += 1;
            } else {
                entries.push(entry);
            }
        }
    }
    if conflicts > 0 {
        return Ok(Merged {
            bytes: target.unwrap_or(source).to_vec(),
            conflicts,
        });
    }
    if entries.len() > 1024 {
        return Err(invalid());
    }
    let mut bytes = local.prefix.to_vec();
    for field in &local.fields {
        if field.name != b"servers" {
            bytes.extend_from_slice(field.raw);
            continue;
        }
        bytes.extend_from_slice(&[9, 0, 7]);
        bytes.extend_from_slice(b"servers");
        bytes.push(10);
        bytes.extend_from_slice(&(entries.len() as i32).to_be_bytes());
        for entry in &entries {
            bytes.extend_from_slice(entry.raw);
        }
    }
    bytes.push(0);
    if bytes.len() as u64 > MAX_OPTIONS {
        return Err(invalid());
    }
    // Preserve the exact original if only empty-list element encoding differs.
    if target.is_some() && entries.len() == local.entries.len() {
        bytes = target.unwrap().to_vec();
    }
    Ok(Merged { bytes, conflicts })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture(entries: &[(&str, &str, u8)]) -> Vec<u8> {
        let mut bytes = vec![10, 0, 0, 9, 0, 7];
        bytes.extend_from_slice(b"servers");
        bytes.push(10);
        bytes.extend_from_slice(&(entries.len() as i32).to_be_bytes());
        for (name, ip, unknown) in entries {
            for (key, value) in [("name", *name), ("ip", *ip)] {
                bytes.push(8);
                bytes.extend_from_slice(&(key.len() as u16).to_be_bytes());
                bytes.extend_from_slice(key.as_bytes());
                bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
                bytes.extend_from_slice(value.as_bytes());
            }
            bytes.extend_from_slice(&[1, 0, 1, b'x', *unknown, 0]);
        }
        bytes.extend_from_slice(&[8, 0, 1, b'z', 0, 1, b'q', 0]);
        bytes
    }
    #[test]
    fn empty_additions_and_unknown_fields_preserve_order() {
        let a = fixture(&[("A", "a:1", 42)]);
        let b = fixture(&[("B", "b:1", 7)]);
        let m = merge(&a, Some(&fixture(&[]))).unwrap();
        assert_eq!(m.conflicts, 0);
        assert_eq!(
            parse(&m.bytes).unwrap().entries[0].raw,
            parse(&a).unwrap().entries[0].raw
        );
        let m = merge(&a, Some(&b)).unwrap();
        let parsed = parse(&m.bytes).unwrap();
        assert_eq!(
            parsed.entries.iter().map(|e| e.name).collect::<Vec<_>>(),
            vec![b"B", b"A"]
        );
        assert_eq!(merge(&m.bytes, Some(&m.bytes)).unwrap().bytes, m.bytes);
    }
    #[test]
    fn names_addresses_and_edited_unknown_fields_are_ambiguous() {
        let a = fixture(&[("A", "a", 1)]);
        for b in [
            fixture(&[("A", "b", 1)]),
            fixture(&[("B", "a", 1)]),
            fixture(&[("A", "a", 2)]),
            fixture(&[("A", "a", 1), ("A", "a", 1)]),
        ] {
            let m = merge(&a, Some(&b)).unwrap();
            assert!(m.conflicts > 0);
            assert_eq!(m.bytes, b);
        }
    }
    #[test]
    fn every_truncation_and_invalid_tag_is_rejected() {
        let bytes = fixture(&[("A", "a", 1)]);
        for end in 0..bytes.len() {
            assert!(merge(&bytes[..end], None).is_err(), "{end}");
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(merge(&extra, None).is_err());
        let mut bad = bytes;
        bad[3] = 99;
        assert!(merge(&bad, None).is_err());
    }
    #[test]
    fn all_standard_tag_payloads_round_trip_and_complexity_is_bounded() {
        let mut local = fixture(&[("Local", "local", 7)]);
        local.pop();
        for (tag, payload) in [
            (2, vec![0, 1]),
            (3, vec![0; 4]),
            (4, vec![0; 8]),
            (5, vec![0; 4]),
            (6, vec![0; 8]),
            (7, vec![0, 0, 0, 2, 0, 255]),
            (8, vec![0, 2, 0xc0, 0x80]),
            (9, vec![3, 0, 0, 0, 1, 0, 0, 0, 42]),
            (10, vec![1, 0, 1, b'v', 7, 0]),
            (11, vec![0, 0, 0, 1, 0, 0, 0, 7]),
            (12, vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 8]),
        ] {
            local.extend_from_slice(&[tag, 0, 1, b'a' + tag]);
            local.extend_from_slice(&payload);
        }
        local.push(0);
        let merged = merge(&fixture(&[("New", "new", 1)]), Some(&local)).unwrap();
        let original = parse(&local).unwrap();
        let next = parse(&merged.bytes).unwrap();
        for f in original.fields.iter().filter(|f| f.name != b"servers") {
            assert!(next.fields.iter().any(|n| n.raw == f.raw));
        }
        let mut deep = vec![10, 0, 0];
        for _ in 0..40 {
            deep.extend_from_slice(&[10, 0, 1, b'x']);
        }
        deep.extend_from_slice(&[0; 41]);
        assert!(merge(&deep, None).is_err());
        let mut invalid = local.clone();
        invalid[0] = 12;
        assert!(merge(&invalid, None).is_err());
    }
}
