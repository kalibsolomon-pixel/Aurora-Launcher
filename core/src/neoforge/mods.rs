//! `neoforge.mods.toml` parsing: the NeoForge counterpart of
//! `fabric.mod.json` inspection.
//!
//! The parser is a deliberate TOML subset: array-of-table headers
//! (`[[mods]]`, `[[dependencies.<owner>]]`), table headers, `key = "value"`
//! string assignments, and comments. Only the fields Aurora's normalized
//! mod-metadata model consumes are read; everything else is ignored
//! syntactically. Bounds mirror the Fabric inspector (256 KiB).

use std::collections::BTreeMap;

/// The largest `neoforge.mods.toml` document this parser will read.
pub const MAX_MODS_TOML_BYTES: usize = 256 * 1024;

/// The parsed document, shaped for normalization into the shared
/// loader-neutral mod metadata.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NeoForgeModsDocument {
    pub mods: Vec<NeoForgeModEntry>,
    /// Dependency declarations keyed by the owning mod id.
    pub dependencies: BTreeMap<String, Vec<NeoForgeDependency>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NeoForgeModEntry {
    pub mod_id: Option<String>,
    pub version: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub icon_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgeDependency {
    pub mod_id: String,
    /// `required` | `optional` | `incompatible` (discouraged types are
    /// kept verbatim and treated as optional).
    pub kind: String,
    pub version_range: String,
}

impl NeoForgeModsDocument {
    /// Parses the document. Structural garbage is an error (the caller
    /// surfaces a warning); unrecognized well-formed lines are ignored.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_MODS_TOML_BYTES {
            return Err("the neoforge.mods.toml exceeds the parsing bound".to_owned());
        }
        let mut document = Self::default();
        // The current table path, e.g. ["mods", "0"] or
        // ["dependencies", "mymod", "0"].
        let mut path: Vec<String> = Vec::new();
        for (index, raw_line) in text.lines().enumerate() {
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(header) = line.strip_prefix("[[") {
                let name = header
                    .strip_suffix("]]")
                    .ok_or_else(|| format!("line {}: malformed array-table header", index + 1))?;
                let segments = split_header(name)?;
                if segments.is_empty() {
                    return Err(format!("line {}: empty table header", index + 1));
                }
                push_array_entry(&mut document, &segments)?;
                path = segments;
                continue;
            }
            if let Some(header) = line.strip_prefix('[') {
                let name = header
                    .strip_suffix(']')
                    .ok_or_else(|| format!("line {}: malformed table header", index + 1))?;
                let segments = split_header(name)?;
                if segments.is_empty() {
                    return Err(format!("line {}: empty table header", index + 1));
                }
                path = segments;
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {}: expected key = value", index + 1))?;
            let key = key.trim().trim_matches('"').to_owned();
            let value = parse_value(value.trim())
                .ok_or_else(|| format!("line {}: unsupported value syntax", index + 1))?;
            assign(&mut document, &path, &key, value)?;
        }
        Ok(document)
    }
}

fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    for (index, character) in line.char_indices() {
        match character {
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..index],
            _ => {}
        }
    }
    line
}

fn split_header(header: &str) -> Result<Vec<String>, String> {
    let mut segments = Vec::new();
    for segment in header.split('.') {
        let segment = segment.trim();
        let cleaned = segment.trim_matches('"');
        if cleaned.is_empty() || cleaned.contains(['[', ']', '=']) {
            return Err(format!("malformed table header segment '{segment}'"));
        }
        segments.push(cleaned.to_owned());
    }
    Ok(segments)
}

/// Parses a value: quoted strings, bare words, integers, booleans. Arrays
/// and inline tables return `None` (unsupported, and unused by the fields
/// this parser consumes).
fn parse_value(value: &str) -> Option<String> {
    if let Some(quoted) = value.strip_prefix('"') {
        let end = quoted.rfind('"')?;
        if end != quoted.len() - 1 || quoted[..end].contains('"') {
            return None;
        }
        return Some(quoted[..end].replace("\\\"", "\"").replace("\\\\", "\\"));
    }
    if value.starts_with('[') || value.starts_with('{') || value.is_empty() {
        return None;
    }
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'.' | b'-' | b'_' | b'+' | b'/' | b':' | b'@')
    }) {
        return Some(value.to_owned());
    }
    None
}

fn push_array_entry(
    document: &mut NeoForgeModsDocument,
    segments: &[String],
) -> Result<(), String> {
    match segments {
        [table] if table == "mods" => document.mods.push(NeoForgeModEntry::default()),
        [table, owner] if table == "dependencies" => {
            document
                .dependencies
                .entry(owner.clone())
                .or_default()
                .push(NeoForgeDependency {
                    mod_id: String::new(),
                    kind: "required".to_owned(),
                    version_range: String::new(),
                });
        }
        // Real documents carry further tables ([[mixins]],
        // [[accessTransformers]], ...); they are well-formed metadata this
        // model does not consume, so they are ignored, not rejected.
        _ => {}
    }
    Ok(())
}

fn assign(
    document: &mut NeoForgeModsDocument,
    path: &[String],
    key: &str,
    value: String,
) -> Result<(), String> {
    match path {
        [table] if table == "mods" => {
            // Values before any [[mods]] entry are ignored.
            if let Some(entry) = document.mods.last_mut() {
                match key {
                    "modId" => entry.mod_id = Some(value),
                    "version" => entry.version = Some(value),
                    "displayName" => entry.display_name = Some(value),
                    "description" => entry.description = Some(value),
                    "authors" => entry.authors = split_people(&value),
                    "iconFile" | "logoFile" => entry.icon_file = Some(value),
                    _ => {}
                }
            }
        }
        [table, owner] if table == "dependencies" => {
            if let Some(list) = document.dependencies.get_mut(owner) {
                if let Some(entry) = list.last_mut() {
                    match key {
                        "modId" => entry.mod_id = value,
                        "type" => entry.kind = value,
                        "versionRange" => entry.version_range = value,
                        _ => {}
                    }
                }
            }
        }
        // Root-level keys (modLoader, loaderVersion, license, ...) and any
        // other well-formed tables are not consumed.
        _ => {}
    }
    Ok(())
}

fn split_people(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPRESENTATIVE: &str = r#"
modLoader="javafml"
loaderVersion="[3,]"
license="LGPL v2.1"

[[mods]]
    modId="neoforge"
    version="26.2.0.88"
    displayName="NeoForge"
    authors="The NeoForged Team"
    description="A modding API."
    iconFile="neoforge_icon.png"

[[dependencies.mymod]]
    modId="minecraft"
    type="required"
    versionRange="[26.2]"
    ordering="NONE"
    side="BOTH"

[[dependencies.mymod]]
    modId="neoforge"
    type="required"
    versionRange="[26.2.0.88,)"
"#;

    #[test]
    fn a_representative_document_parses() {
        let document = NeoForgeModsDocument::parse(REPRESENTATIVE).unwrap();
        assert_eq!(document.mods.len(), 1);
        let neoforge = &document.mods[0];
        assert_eq!(neoforge.mod_id.as_deref(), Some("neoforge"));
        assert_eq!(neoforge.version.as_deref(), Some("26.2.0.88"));
        assert_eq!(neoforge.icon_file.as_deref(), Some("neoforge_icon.png"));
        let dependencies = document.dependencies.get("mymod").unwrap();
        assert_eq!(dependencies.len(), 2);
        assert_eq!(dependencies[0].mod_id, "minecraft");
        assert_eq!(dependencies[0].version_range, "[26.2]");
        assert_eq!(dependencies[1].mod_id, "neoforge");
        assert_eq!(dependencies[1].kind, "required");
    }

    #[test]
    fn comments_and_unconsumed_tables_are_ignored() {
        let document = NeoForgeModsDocument::parse(
            "# a comment\n[[mixins]]\nconfig = \"neoforge.mixins.json\"\n[[accessTransformers]]\nfile=\"META-INF/at.cfg\"\n[[mods]]\nmodId=\"a\" # trailing\n",
        )
        .unwrap();
        assert_eq!(document.mods.len(), 1);
        assert_eq!(document.mods[0].mod_id.as_deref(), Some("a"));
    }

    #[test]
    fn structural_garbage_is_rejected() {
        assert!(NeoForgeModsDocument::parse("modId").is_err());
        assert!(NeoForgeModsDocument::parse("[[mods").is_err());
        assert!(NeoForgeModsDocument::parse("[[evil.]").is_err());
        assert!(NeoForgeModsDocument::parse("key = \"unterminated").is_err());
    }
}
