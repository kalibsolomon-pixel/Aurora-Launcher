//! Narrowly scoped ownership of Minecraft's enabled-resource-pack list.
//!
//! Minecraft stores enabled resource packs as a `resourcePacks:` line inside
//! the game directory's `options.txt`. The value is a bracketed list; modern
//! Minecraft (verified against 1.21.11's own document) quotes every entry
//! (`resourcePacks:["vanilla","file/Pack.zip"]`), escaping `"` and `\`, while
//! its reader still accepts legacy unquoted entries. Aurora parses both forms
//! into values, matches by value, and writes the modern quoted form. Only
//! this one line is rewritten — spliced byte-exactly so every other byte of
//! the user's document round-trips untouched — and names that cannot be
//! represented unambiguously are refused instead of guessed at. Nothing here
//! watches the file: reads happen on demand inside user-triggered scans and
//! mutations, and writes are atomic (temporary sibling plus rename).

use std::path::{Path, PathBuf};

use crate::instance_content::ContentError;

/// `options.txt` is a tiny configuration file; a larger file is not one.
const MAX_OPTIONS_BYTES: u64 = 1024 * 1024;
const KEY: &str = "resourcePacks";
const FILE_PREFIX: &str = "file/";
const VANILLA: &str = "vanilla";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceChange {
    /// Insert `file/<name>` at the top of the enabled list when absent.
    Enable { name: String },
    /// Drop every `file/<name>` reference while preserving all other entries.
    Disable { name: String },
    /// Move an enabled reference when a managed update renamed the pack file.
    Rename { from: String, to: String },
}

#[derive(Debug)]
enum ActivationError {
    Io(std::io::Error),
    Malformed(&'static str),
    UnsafeName(String),
}

impl From<ActivationError> for ContentError {
    fn from(error: ActivationError) -> Self {
        match error {
            ActivationError::Io(inner) => ContentError::Io(inner),
            ActivationError::Malformed(reason) => ContentError::OptionsMalformed(reason.into()),
            ActivationError::UnsafeName(name) => ContentError::UnsupportedActionWith(format!(
                "'{name}' cannot be represented safely in Minecraft's options.txt"
            )),
        }
    }
}

/// Whether a pack file name can appear in the enabled list unambiguously.
/// Quoting plus `"`/`\` escaping makes ordinary punctuation safe; control
/// characters and padded names stay refused.
pub fn options_representable(name: &str) -> bool {
    !name.is_empty() && name == name.trim() && !name.chars().any(char::is_control)
}

fn reference(name: &str) -> String {
    format!("{FILE_PREFIX}{name}")
}

/// The parsed enabled values plus the exact byte range of their line, so a
/// rewrite splices only that line and leaves every other byte untouched.
struct OptionsDocument {
    text: String,
    /// Whether the document contains a `resourcePacks:` line.
    has_line: bool,
    /// Byte offset where the `resourcePacks:` line starts.
    line_start: usize,
    /// Byte offset just past the line's value (before its terminator).
    value_end: usize,
    /// The line's own terminator (`\r\n`, `\n`, or empty at end of file).
    line_terminator: &'static str,
    /// The document's dominant terminator, used for newly appended lines.
    document_terminator: &'static str,
    /// Enabled values with quoting resolved (`file/Pack.zip`, `vanilla`, …).
    values: Vec<String>,
}

fn options_path(root: &Path) -> PathBuf {
    root.join("options.txt")
}

fn load(root: &Path) -> Result<Option<OptionsDocument>, ActivationError> {
    let path = options_path(root);
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ActivationError::Io(error)),
    };
    if !meta.is_file() {
        return Err(ActivationError::Malformed(
            "options.txt is not a regular file",
        ));
    }
    if meta.len() > MAX_OPTIONS_BYTES {
        return Err(ActivationError::Malformed(
            "options.txt exceeds the 1 MiB read bound",
        ));
    }
    let bytes = std::fs::read(&path).map_err(ActivationError::Io)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| ActivationError::Malformed("options.txt is not valid UTF-8"))?;
    let document_terminator: &'static str = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut document = OptionsDocument {
        has_line: false,
        line_start: 0,
        value_end: 0,
        line_terminator: "",
        document_terminator,
        values: vec![VANILLA.to_owned()],
        text,
    };
    let mut offset = 0usize;
    for raw_line in document.text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let terminator: &'static str = if raw_line.len() > line.len() {
            "\r\n"
        } else if offset + raw_line.len() < document.text.len() {
            "\n"
        } else {
            ""
        };
        if line.starts_with(KEY) && line[KEY.len()..].starts_with(':') {
            if document.has_line {
                return Err(ActivationError::Malformed(
                    "options.txt declares resourcePacks twice",
                ));
            }
            document.has_line = true;
            document.line_start = offset;
            document.value_end = offset + line.len();
            document.line_terminator = terminator;
            document.values = parse_value(&line[KEY.len() + 1..])?;
        }
        offset += raw_line.len() + 1;
    }
    Ok(Some(document))
}

/// Parses the bracketed list into values. Modern Minecraft quotes every
/// entry (`"vanilla"`, `"file/X.zip"`, `\"`/`\\` escapes); legacy unquoted
/// entries are still accepted. A trailing comma — which Minecraft itself
/// writes for empty lists (`["vanilla",]` in some versions) — is tolerated.
fn parse_value(value: &str) -> Result<Vec<String>, ActivationError> {
    let trimmed = value.trim();
    let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return Err(ActivationError::Malformed(
            "the resourcePacks value is not a bracketed list",
        ));
    };
    let inner = inner.strip_suffix(',').unwrap_or(inner);
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;
    let mut started = false;
    for character in inner.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_quotes => {
                escaped = true;
            }
            '"' => {
                in_quotes = !in_quotes;
                started = true;
            }
            ',' if !in_quotes => {
                if started || !current.is_empty() {
                    values.push(current.clone());
                }
                current.clear();
                started = false;
            }
            _ => {
                current.push(character);
                started = true;
            }
        }
    }
    if in_quotes || escaped {
        return Err(ActivationError::Malformed(
            "a resourcePacks entry has an unterminated quote",
        ));
    }
    if started || !current.is_empty() {
        values.push(current);
    }
    Ok(values)
}

/// Serializes values the way modern Minecraft writes them: one quoted,
/// escaped entry per pack, comma separated, inside brackets.
fn serialize_values(values: &[String]) -> String {
    let entries: Vec<String> = values
        .iter()
        .map(|value| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", entries.join(","))
}

/// Reads whether `name` is currently enabled. An `Err` means options.txt
/// could not be read safely; the caller must not offer activation decisions
/// built on a guess.
pub fn is_enabled(root: &Path, name: &str) -> Result<bool, ContentError> {
    let Some(document) = load(root)? else {
        return Ok(false);
    };
    Ok(document
        .values
        .iter()
        .any(|value| value == &reference(name)))
}

/// The enabled reference values (`file/<name>`, `vanilla`, …) for read-only
/// inventory projection. `Ok(None)` when options.txt does not exist yet.
pub fn enabled_references(root: &Path) -> Result<Option<Vec<String>>, ContentError> {
    Ok(load(root)?.map(|document| document.values))
}

/// Applies reference changes atomically and returns whether the file changed.
/// A no-op (nothing to enable/disable/rename) never writes. Rewriting the
/// list also drops duplicate values and normalizes every surviving entry to
/// Minecraft's quoted form, keeping each value's first position.
pub fn apply(root: &Path, changes: &[ReferenceChange]) -> Result<bool, ContentError> {
    for change in changes {
        let name = match change {
            ReferenceChange::Enable { name } | ReferenceChange::Disable { name } => name,
            ReferenceChange::Rename { from, to } => {
                if !options_representable(from) || !options_representable(to) {
                    return Err(ActivationError::UnsafeName(to.clone()).into());
                }
                continue;
            }
        };
        if !options_representable(name) {
            return Err(ActivationError::UnsafeName(name.clone()).into());
        }
    }
    let document = load(root)?;
    let (text, has_line, line_start, value_end, line_terminator, document_terminator, mut values) =
        match document {
            None => (
                String::new(),
                false,
                0,
                0,
                "",
                "\n",
                vec![VANILLA.to_owned()],
            ),
            Some(document) => (
                document.text,
                document.has_line,
                document.line_start,
                document.value_end,
                document.line_terminator,
                document.document_terminator,
                document.values,
            ),
        };
    let original = values.clone();
    for change in changes {
        match change {
            ReferenceChange::Enable { name } => enable_value(&mut values, &reference(name)),
            ReferenceChange::Disable { name } => {
                let target = reference(name);
                values.retain(|value| value != &target);
            }
            ReferenceChange::Rename { from, to } => {
                let (from_value, to_value) = (reference(from), reference(to));
                for value in &mut values {
                    if *value == from_value {
                        *value = to_value.clone();
                    }
                }
            }
        }
    }
    if values == original {
        return Ok(false);
    }
    // One value may legitimately appear only once; when a write happens,
    // duplicates (quoted plus legacy unquoted) collapse to the first
    // occurrence, matching how Minecraft itself rewrites the list.
    let mut seen = std::collections::HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
    let line = format!("{KEY}:{}", serialize_values(&values));
    let bytes = if has_line {
        let mut rebuilt = String::with_capacity(text.len() + line.len() + line_terminator.len());
        rebuilt.push_str(&text[..line_start]);
        rebuilt.push_str(&line);
        rebuilt.push_str(line_terminator);
        rebuilt.push_str(&text[value_end + line_terminator.len()..]);
        rebuilt.into_bytes()
    } else {
        let mut rebuilt =
            String::with_capacity(text.len() + line.len() + 2 * document_terminator.len());
        rebuilt.push_str(&text);
        if !rebuilt.is_empty() && !rebuilt.ends_with('\n') {
            rebuilt.push_str(document_terminator);
        }
        rebuilt.push_str(&line);
        rebuilt.push_str(document_terminator);
        rebuilt.into_bytes()
    };
    write_bytes(&options_path(root), &bytes)?;
    Ok(true)
}

/// A newly enabled pack takes the top position, matching the in-game pack
/// screen; every existing entry keeps its relative order.
fn enable_value(values: &mut Vec<String>, target: &str) {
    if !values.iter().any(|value| value == target) {
        values.insert(0, target.to_owned());
    }
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), ContentError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(ActivationError::Io)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    let outcome = std::fs::write(&temporary, bytes).and_then(|_| std::fs::rename(&temporary, path));
    if outcome.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(ContentError::Io(std::io::Error::other(
            "options.txt could not be written atomically",
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRoot(PathBuf);
    impl TempRoot {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join("aurora-pack-activation-tests")
                .join(uuid::Uuid::new_v4().to_string());
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn path(&self) -> &Path {
            &self.0
        }
        fn write_options(&self, text: &str) {
            std::fs::write(self.0.join("options.txt"), text).unwrap();
        }
        fn read_options(&self) -> String {
            std::fs::read_to_string(self.0.join("options.txt")).unwrap()
        }
    }
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn enabled(name: &str) -> ReferenceChange {
        ReferenceChange::Enable {
            name: name.to_owned(),
        }
    }

    fn disabled(name: &str) -> ReferenceChange {
        ReferenceChange::Disable {
            name: name.to_owned(),
        }
    }

    #[test]
    fn absent_options_means_no_pack_is_enabled() {
        let root = TempRoot::new();
        assert!(!is_enabled(root.path(), "Pack.zip").unwrap());
        assert!(!apply(root.path(), &[disabled("Pack.zip")]).unwrap());
        assert!(!root.path().join("options.txt").exists());
    }

    #[test]
    fn enable_disable_enable_round_trips_and_survives_reload() {
        let root = TempRoot::new();
        assert!(apply(root.path(), &[enabled("Faithful.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "resourcePacks:[\"file/Faithful.zip\",\"vanilla\"]\n"
        );
        assert!(is_enabled(root.path(), "Faithful.zip").unwrap());
        assert!(apply(root.path(), &[disabled("Faithful.zip")]).unwrap());
        assert_eq!(root.read_options(), "resourcePacks:[\"vanilla\"]\n");
        assert!(!is_enabled(root.path(), "Faithful.zip").unwrap());
        assert!(apply(root.path(), &[enabled("Faithful.zip")]).unwrap());
        assert!(is_enabled(root.path(), "Faithful.zip").unwrap());
    }

    #[test]
    fn minecrafts_quoted_document_is_understood_verbatim() {
        // This is the shape Minecraft 1.21.11 itself writes.
        let root = TempRoot::new();
        root.write_options(
            "version:4671\nresourcePacks:[\"vanilla\",\"file/§bCyanOutlineCobwebs.zip\"]\n",
        );
        assert!(is_enabled(root.path(), "§bCyanOutlineCobwebs.zip").unwrap());
        assert!(!is_enabled(root.path(), "vanilla pack").unwrap());
        assert!(apply(root.path(), &[disabled("§bCyanOutlineCobwebs.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "version:4671\nresourcePacks:[\"vanilla\"]\n"
        );
    }

    #[test]
    fn mixed_and_duplicated_entries_match_by_value_and_dedupe_on_write() {
        // Repair shape: Minecraft's quoted originals plus legacy unquoted
        // duplicates. A write keeps each value once, at its first position,
        // and normalizes to the quoted form.
        let root = TempRoot::new();
        root.write_options("resourcePacks:[file/A.zip,\"vanilla\",\"file/A.zip\",file/B.zip]\n");
        assert!(is_enabled(root.path(), "A.zip").unwrap());
        assert!(apply(root.path(), &[enabled("C.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "resourcePacks:[\"file/C.zip\",\"file/A.zip\",\"vanilla\",\"file/B.zip\"]\n"
        );
    }

    #[test]
    fn enabling_preserves_order_of_unrelated_packs_and_defaults() {
        let root = TempRoot::new();
        root.write_options(
            "volume:0.7\nresourcePacks:[file/Other.zip,file/Kept.zip,vanilla]\nfov:70.0\n",
        );
        assert!(apply(root.path(), &[enabled("New.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "volume:0.7\nresourcePacks:[\"file/New.zip\",\"file/Other.zip\",\"file/Kept.zip\",\"vanilla\"]\nfov:70.0\n"
        );
    }

    #[test]
    fn enabling_an_already_enabled_pack_keeps_its_position() {
        let root = TempRoot::new();
        root.write_options("resourcePacks:[file/A.zip,file/B.zip,vanilla]\n");
        assert!(!apply(root.path(), &[enabled("B.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "resourcePacks:[file/A.zip,file/B.zip,vanilla]\n"
        );
    }

    #[test]
    fn disabling_keeps_unrelated_entries_without_touching_other_lines() {
        let root = TempRoot::new();
        root.write_options(
            "volume:0.7\nresourcePacks:[\"file/A.zip\",\"file/B.zip\",\"server packs/x\",\"file/C.zip\",\"vanilla\"]\nfov:70.0\n",
        );
        assert!(apply(root.path(), &[disabled("B.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "volume:0.7\nresourcePacks:[\"file/A.zip\",\"server packs/x\",\"file/C.zip\",\"vanilla\"]\nfov:70.0\n"
        );
    }

    #[test]
    fn duplicate_references_are_all_removed() {
        let root = TempRoot::new();
        root.write_options("resourcePacks:[file/A.zip,\"file/A.zip\",file/A.zip,vanilla]\n");
        assert!(apply(root.path(), &[disabled("A.zip")]).unwrap());
        assert_eq!(root.read_options(), "resourcePacks:[\"vanilla\"]\n");
    }

    #[test]
    fn names_with_commas_and_quotes_round_trip_through_escaping() {
        let root = TempRoot::new();
        root.write_options("resourcePacks:[\"vanilla\"]\n");
        assert!(apply(root.path(), &[enabled("A,B.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "resourcePacks:[\"file/A,B.zip\",\"vanilla\"]\n"
        );
        assert!(is_enabled(root.path(), "A,B.zip").unwrap());
        root.write_options("resourcePacks:[\"file/a\\\"b.zip\",\"vanilla\"]\n");
        assert!(is_enabled(root.path(), "a\"b.zip").unwrap());
        assert!(!apply(root.path(), &[enabled("a\"b.zip")]).unwrap());
    }

    #[test]
    fn windows_line_endings_are_preserved() {
        let root = TempRoot::new();
        root.write_options("volume:0.7\r\nresourcePacks:[vanilla]\r\n");
        assert!(apply(root.path(), &[enabled("A.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "volume:0.7\r\nresourcePacks:[\"file/A.zip\",\"vanilla\"]\r\n"
        );
    }

    #[test]
    fn files_without_a_resourcepacks_line_gain_one_without_other_edits() {
        let root = TempRoot::new();
        root.write_options("volume:0.7\n");
        assert!(apply(root.path(), &[enabled("A.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "volume:0.7\nresourcePacks:[\"file/A.zip\",\"vanilla\"]\n"
        );
    }

    #[test]
    fn files_without_a_trailing_newline_gain_a_well_formed_one() {
        let root = TempRoot::new();
        root.write_options("volume:0.7");
        assert!(apply(root.path(), &[enabled("A.zip")]).unwrap());
        assert_eq!(
            root.read_options(),
            "volume:0.7\nresourcePacks:[\"file/A.zip\",\"vanilla\"]\n"
        );
    }

    #[test]
    fn rename_migrates_the_reference_in_place() {
        let root = TempRoot::new();
        root.write_options("resourcePacks:[\"file/Old.zip\",\"vanilla\"]\n");
        assert!(
            apply(
                root.path(),
                &[ReferenceChange::Rename {
                    from: "Old.zip".into(),
                    to: "New.zip".into()
                }]
            )
            .unwrap()
        );
        assert_eq!(
            root.read_options(),
            "resourcePacks:[\"file/New.zip\",\"vanilla\"]\n"
        );
    }

    #[test]
    fn empty_list_round_trips() {
        let root = TempRoot::new();
        root.write_options("resourcePacks:[]\n");
        assert!(apply(root.path(), &[enabled("A.zip")]).unwrap());
        assert_eq!(root.read_options(), "resourcePacks:[\"file/A.zip\"]\n");
        assert!(apply(root.path(), &[disabled("A.zip")]).unwrap());
        assert_eq!(root.read_options(), "resourcePacks:[]\n");
    }

    #[test]
    fn malformed_values_fail_safely_without_writing() {
        for malformed in [
            "resourcePacks:file/A.zip\n",
            "resourcePacks:[file/A.zip\n",
            "resourcePacks:file/A.zip]\n",
            "resourcePacks:[\"unterminated]\n",
        ] {
            let root = TempRoot::new();
            root.write_options(malformed);
            let error = apply(root.path(), &[enabled("B.zip")]).unwrap_err();
            assert!(
                matches!(error, ContentError::OptionsMalformed(_)),
                "{malformed} must be malformed, got {error:?}"
            );
            assert_eq!(root.read_options(), malformed);
        }
    }

    #[test]
    fn malformed_documents_never_report_activation_state() {
        let root = TempRoot::new();
        root.write_options("resourcePacks:[broken\n");
        assert!(matches!(
            is_enabled(root.path(), "A.zip"),
            Err(ContentError::OptionsMalformed(_))
        ));
    }

    #[test]
    fn duplicate_resourcepacks_lines_are_refused() {
        let root = TempRoot::new();
        root.write_options("resourcePacks:[vanilla]\nresourcePacks:[vanilla]\n");
        assert!(matches!(
            apply(root.path(), &[enabled("A.zip")]),
            Err(ContentError::OptionsMalformed(_))
        ));
    }

    #[test]
    fn non_utf8_documents_are_refused() {
        let root = TempRoot::new();
        std::fs::write(root.0.join("options.txt"), b"resourcePacks:[\xff\xfe]\n").unwrap();
        assert!(matches!(
            is_enabled(root.path(), "A.zip"),
            Err(ContentError::OptionsMalformed(_))
        ));
    }

    #[test]
    fn oversized_documents_are_refused() {
        let root = TempRoot::new();
        let large = format!("misc:{}", "a".repeat(1024 * 1024 + 8));
        root.write_options(&large);
        assert!(matches!(
            is_enabled(root.path(), "A.zip"),
            Err(ContentError::OptionsMalformed(_))
        ));
    }

    #[test]
    fn unrepresentable_names_are_refused_rather_than_escaped() {
        let root = TempRoot::new();
        for name in [" lead.zip", "trail.zip ", "\u{7}.zip"] {
            let error = apply(root.path(), &[enabled(name)]).unwrap_err();
            assert!(
                matches!(error, ContentError::UnsupportedActionWith(_)),
                "{name:?} must be refused"
            );
            assert!(!root.path().join("options.txt").exists());
        }
    }

    #[test]
    fn representable_names_allow_unicode_spaces_and_punctuation() {
        assert!(options_representable("Faithful 32x.zip"));
        assert!(options_representable("和風.zip"));
        assert!(options_representable("§bBetter PvP Axes.zip"));
        assert!(options_representable("SmallFood&Items.zip"));
        assert!(options_representable("A,B.zip"));
        assert!(!options_representable(""));
    }
}
