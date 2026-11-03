//! Bounded, read-only NBT value extraction for saved-game discovery.
//!
//! Saved-server (`servers.dat`) and saved-world (`level.dat`) browsing walk
//! one level of named fields at a time over raw input spans, so unrelated
//! compounds (for example worldgen settings) never enter memory. The reader
//! also preserves raw spans for instance sync, which owns merge semantics.

/// Every discovery read is bounded by the caller's input cap; these bounds
/// keep a single hostile document from exploding walk state.
const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 32_768;
const MAX_LIST: usize = 4096;
const MAX_ARRAY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct Invalid;

pub(crate) type WalkResult<T> = Result<T, Invalid>;

fn invalid<T>() -> Result<T, Invalid> {
    Err(Invalid)
}

/// One named payload span: the value bytes only, without the leading type
/// tag or name, so callers descend without re-lexing.
#[derive(Clone)]
pub(crate) struct Field<'a> {
    pub tag: u8,
    pub name: &'a [u8],
    pub payload: &'a [u8],
    pub raw: &'a [u8],
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pub(crate) at: usize,
    nodes: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> WalkResult<&'a [u8]> {
        let end = self.at.checked_add(len).ok_or(Invalid)?;
        let value = self.bytes.get(self.at..end).ok_or(Invalid)?;
        self.at = end;
        Ok(value)
    }
    pub(crate) fn byte(&mut self) -> WalkResult<u8> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn string(&mut self) -> WalkResult<&'a [u8]> {
        let len = u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| Invalid)?) as usize;
        let bytes = self.take(len)?;
        // Java modified UTF-8: reject truncated or malformed encodings. The
        // two-byte encodings that vanilla never writes for these fields are
        // still accepted as valid sequences; callers sanitize text.
        let mut at = 0;
        while at < bytes.len() {
            let a = bytes[at];
            if (1..=0x7f).contains(&a) {
                at += 1;
            } else if (0xc0..=0xdf).contains(&a) {
                let b = *bytes.get(at + 1).ok_or(Invalid)?;
                if b & 0xc0 != 0x80 || (a < 0xc2 && !(a == 0xc0 && b == 0x80)) {
                    return invalid();
                }
                at += 2;
            } else if (0xe0..=0xef).contains(&a) {
                let b = *bytes.get(at + 1).ok_or(Invalid)?;
                let c = *bytes.get(at + 2).ok_or(Invalid)?;
                if b & 0xc0 != 0x80 || c & 0xc0 != 0x80 || (a == 0xe0 && b < 0xa0) {
                    return invalid();
                }
                at += 3;
            } else {
                return invalid();
            }
        }
        Ok(bytes)
    }
    pub(crate) fn count(&mut self) -> WalkResult<usize> {
        let n = i32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Invalid)?);
        usize::try_from(n).map_err(|_| Invalid)
    }
    /// Skips one payload of `tag`, bounding node count and list sizes.
    fn skip(&mut self, tag: u8, depth: usize) -> WalkResult<()> {
        self.nodes += 1;
        if depth > MAX_DEPTH || self.nodes > MAX_NODES {
            return invalid();
        }
        match tag {
            1 => {
                self.take(1)?;
            }
            2 => {
                self.take(2)?;
            }
            3 | 5 => {
                self.take(4)?;
            }
            4 | 6 => {
                self.take(8)?;
            }
            7 | 11 | 12 => {
                let width = match tag {
                    7 => 1,
                    11 => 4,
                    _ => 8,
                };
                let n = self.count()?.checked_mul(width).ok_or(Invalid)?;
                if n > MAX_ARRAY_BYTES {
                    return invalid();
                }
                self.take(n)?;
            }
            8 => {
                self.string()?;
            }
            9 => {
                let element = self.byte()?;
                let count = self.count()?;
                if count > MAX_LIST || element > 12 || (element == 0 && count != 0) {
                    return invalid();
                }
                for _ in 0..count {
                    self.skip(element, depth + 1)?;
                }
            }
            10 => {
                self.walk_compound(depth + 1, false)?;
            }
            _ => return invalid(),
        }
        Ok(())
    }
    /// Reads one compound's named fields up to its end marker.
    pub(crate) fn compound(&mut self, depth: usize) -> WalkResult<Vec<Field<'a>>> {
        self.walk_compound(depth, true)
    }

    fn walk_compound(&mut self, depth: usize, collect: bool) -> WalkResult<Vec<Field<'a>>> {
        if depth > MAX_DEPTH {
            return invalid();
        }
        let mut names = std::collections::BTreeSet::new();
        let mut fields = Vec::new();
        loop {
            let start = self.at;
            let tag = self.byte()?;
            if tag == 0 {
                return Ok(fields);
            }
            let name = self.string()?;
            if !names.insert(name) {
                return invalid();
            }
            let payload = self.at;
            self.skip(tag, depth)?;
            if collect {
                fields.push(Field {
                    tag,
                    name,
                    payload: &self.bytes[payload..self.at],
                    raw: &self.bytes[start..self.at],
                });
            }
        }
    }
}

/// Parses the document root: the leading type tag must be a compound, and
/// the whole input must be consumed exactly (truncation is rejection).
pub(crate) fn root(input: &[u8]) -> WalkResult<Vec<Field<'_>>> {
    let mut reader = Reader {
        bytes: input,
        at: 0,
        nodes: 0,
    };
    if reader.byte()? != 10 {
        return invalid();
    }
    reader.string()?;
    let fields = reader.compound(0)?;
    if reader.at != input.len() {
        return invalid();
    }
    Ok(fields)
}

/// Reads a reader positioned at the start of a saved payload. Used to
/// descend into a field of a parent walk.
pub(crate) fn reader_at(input: &[u8], at: usize) -> Reader<'_> {
    Reader {
        bytes: input,
        at,
        nodes: 0,
    }
}

impl<'a> Reader<'a> {
    /// The compounds of a list-of-compound field payload.
    pub(crate) fn compound_list(&mut self) -> WalkResult<Vec<Vec<Field<'a>>>> {
        let tag = self.byte()?;
        let count = self.count()?;
        if count > MAX_LIST || !(tag == 10 || (tag == 0 && count == 0)) {
            return invalid();
        }
        let mut entries = Vec::with_capacity(count.min(64));
        for _ in 0..count {
            let fields = self.compound(1)?;
            entries.push(fields);
        }
        Ok(entries)
    }
}

/// Field accessors. Each validates the tag before converting.
pub(crate) fn as_string<'a>(field: &Field<'a>) -> WalkResult<&'a [u8]> {
    if field.tag != 8 {
        return invalid();
    }
    let mut reader = Reader {
        bytes: field.payload,
        at: 0,
        nodes: 0,
    };
    reader.string()
}

pub(crate) fn as_i64(field: &Field<'_>) -> WalkResult<i64> {
    if field.tag != 4 {
        return invalid();
    }
    let raw = field.payload;
    if raw.len() != 8 {
        return invalid();
    }
    Ok(i64::from_be_bytes(raw.try_into().expect("length checked")))
}

pub(crate) fn as_i32(field: &Field<'_>) -> WalkResult<i32> {
    if field.tag != 3 {
        return invalid();
    }
    let raw = field.payload;
    if raw.len() != 4 {
        return invalid();
    }
    Ok(i32::from_be_bytes(raw.try_into().expect("length checked")))
}

pub(crate) fn as_byte_array<'a>(field: &Field<'a>) -> WalkResult<&'a [u8]> {
    if field.tag != 7 {
        return invalid();
    }
    let mut reader = Reader {
        bytes: field.payload,
        at: 0,
        nodes: 0,
    };
    let count = reader.count()?;
    let bytes = field.payload.get(4..4 + count).ok_or(Invalid)?;
    Ok(bytes)
}

/// Finds the first field with `name` in a walked level.
pub(crate) fn find<'a>(fields: &'a [Field<'a>], name: &str) -> Option<&'a Field<'a>> {
    fields.iter().find(|field| field.name == name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload_string(value: &str) -> Vec<u8> {
        let bytes = value.as_bytes();
        let mut out = vec![0, bytes.len() as u8];
        out.extend_from_slice(bytes);
        out
    }

    fn compound(fields: &[(&str, u8, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, tag, payload) in fields {
            let name = name.as_bytes();
            out.push(*tag);
            out.extend_from_slice(&(name.len() as u16).to_be_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(payload);
        }
        out.push(0);
        out
    }

    #[test]
    fn reads_root_compound_fields() {
        let mut document = vec![10];
        let name = "".as_bytes();
        document.extend_from_slice(&(name.len() as u16).to_be_bytes());
        document.extend_from_slice(name);
        document.extend_from_slice(&compound(&[("LevelName", 8, &payload_string("Home"))]));
        let fields = root(&document).unwrap();
        let level = find(&fields, "LevelName").unwrap();
        assert_eq!(as_string(level).unwrap(), b"Home");
        assert!(as_i64(level).is_err());
    }

    #[test]
    fn walks_list_of_compounds_with_scalars() {
        let entry = compound(&[
            ("name", 8, &payload_string("Hypixel")),
            ("LastPlayed", 4, &1_i64.to_be_bytes()),
        ]);
        let mut list = vec![10, 0, 0, 0, 2];
        list.extend_from_slice(&entry);
        list.extend_from_slice(&entry);
        let mut document = vec![10, 0, 0];
        document.extend_from_slice(&compound(&[("servers", 9, &list)]));
        let fields = root(&document).unwrap();
        let servers = find(&fields, "servers").unwrap();
        assert_eq!(servers.tag, 9);
        // The payload starts at the element tag; compound_list consumes it.
        let mut reader = reader_at(servers.payload, 0);
        let entries = reader.compound_list().unwrap();
        assert_eq!(entries.len(), 2);
        let name = find(&entries[0], "name").unwrap();
        assert_eq!(as_string(name).unwrap(), b"Hypixel");
        let played = find(&entries[0], "LastPlayed").unwrap();
        assert_eq!(as_i64(played).unwrap(), 1);
    }

    #[test]
    fn rejects_truncated_documents_and_bad_tags() {
        assert!(root(b"").is_err());
        assert!(root(&[10, 0, 0, 8, 0, 3, b'a', b'b', b'c']).is_err()); // missing end tag
        let mut document = vec![10, 0, 0, 99, 0, 0];
        document.push(0);
        assert!(root(&document).is_err()); // unknown tag
    }

    #[test]
    fn reads_byte_arrays_with_explicit_count() {
        let payload = {
            let mut out = 3_i32.to_be_bytes().to_vec();
            out.extend_from_slice(&[1, 2, 3]);
            out
        };
        let mut document = vec![10, 0, 0];
        document.extend_from_slice(&compound(&[("icon", 7, &payload)]));
        let fields = root(&document).unwrap();
        let icon = find(&fields, "icon").unwrap();
        assert_eq!(as_byte_array(icon).unwrap(), &[1, 2, 3]);
        assert!(as_i32(icon).is_err());
    }
}

/// Open a bounded snapshot without following a redirected leaf. Callers
/// validate the managed directory ancestry before calling this function.
pub(crate) fn read_file(path: &std::path::Path, max: u64) -> std::io::Result<Vec<u8>> {
    use crate::install::coordination::{redirected, unsafe_path};
    use std::io::Read;
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || redirected(&metadata) {
        return Err(unsafe_path());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || redirected(&metadata) || metadata.len() > max {
        return Err(unsafe_path());
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(unsafe_path());
    }
    Ok(bytes)
}
