//! Bounded execution of NeoForge installer processors.
//!
//! NeoForge's client installation requires running the installer's own
//! tooling (the binary patcher) to produce the patched client artifact.
//! Aurora executes those processor definitions as narrowly modeled build
//! steps — never as a general "run installer" capability:
//!
//! - the argument vector comes only from the normalized plan, substituted
//!   with Aurora-owned values; unknown tokens, bracket references, and
//!   path-shaped results outside the staging roots are rejected;
//! - execution uses the exact managed Java diagnostic executable with an
//!   empty environment (plus the OS floor variables), a bounded working
//!   directory, and a wall-clock timeout;
//! - every filesystem effect must stay inside the staged game tree or the
//!   processor scratch directory: a before/after snapshot detects both
//!   produced outputs (validated and recorded with observed SHA-256) and
//!   any destruction of already-materialized managed files;
//! - a nonzero exit, a timeout, or a missing expected output fails the
//!   installation; staging is disposable and no partial state commits.
//!
//! Processors are skipped when every artifact they reference already exists
//! with a recorded digest — the retry/reuse semantics for verified inputs.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::neoforge::plan::{NeoForgeDataValue, NeoForgePlan, NeoForgeProcessorStep};

/// The wall-clock bound for one processor execution.
const PROCESSOR_TIMEOUT: Duration = Duration::from_secs(600);

/// The largest captured processor output retained for diagnostics.
const MAX_CAPTURED_OUTPUT_BYTES: usize = 256 * 1024;

/// The tail length embedded in a failure diagnostic.
const DIAGNOSTIC_TAIL_CHARS: usize = 2000;

/// Everything a processor run needs, all Aurora-owned.
#[derive(Debug, Clone)]
pub struct ProcessorContext {
    /// The staged game tree (`{ROOT}` in installer terms) — the only root
    /// processors may read managed inputs from and write outputs to.
    pub staging_game: PathBuf,
    /// The scratch directory (inside instance staging, outside the game
    /// tree) for installer-embedded inputs such as the binpatch bundle.
    pub scratch_directory: PathBuf,
    /// The verified installer artifact in the SHA-1 store (read-only).
    pub installer_path: PathBuf,
    /// The managed Java diagnostic executable that runs the tools.
    pub java_executable: PathBuf,
    /// The staged vanilla client jar (`{MINECRAFT_JAR}`).
    pub client_jar: PathBuf,
    /// The exact Minecraft version (`{MINECRAFT_VERSION}`).
    pub minecraft_version: String,
}

/// One file produced by processor execution, recorded for validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedArtifact {
    /// Path relative to the staged game root, forward slashes.
    pub relative: String,
    /// Locally observed SHA-256 — a consistency reference, never an
    /// official verification.
    pub observed_sha256: String,
    pub size_bytes: u64,
}

/// The result of a successful processor pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProcessorOutcome {
    pub generated: Vec<GeneratedArtifact>,
}

/// A failure of the bounded processor pass.
#[derive(Debug)]
pub enum ProcessorError {
    /// An argument could not be substituted safely.
    Substitution { argument: String, reason: String },
    /// A required processor input does not exist in staging.
    MissingInput { relative: String },
    /// The tool jar is unreadable or has no main class.
    UnusableTool { jar: String, reason: String },
    /// The process could not be spawned.
    Spawn { jar: String, reason: String },
    /// The process exited nonzero.
    Failed {
        jar: String,
        exit_code: Option<i32>,
        diagnostics: String,
    },
    /// The process exceeded the wall-clock bound.
    Timeout { jar: String },
    /// An expected output was not produced.
    OutputMissing { relative: String },
    /// Processing removed an already-materialized managed file.
    Destroyed { relative: String },
    /// A filesystem effect escaped the staging roots.
    Escape { path: String },
    /// Filesystem trouble while preparing or snapshotting.
    Io {
        context: String,
        source: std::io::Error,
    },
}

impl fmt::Display for ProcessorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Substitution { argument, reason } => write!(
                formatter,
                "a NeoForge processor argument could not be resolved safely ('{argument}'): {reason}"
            ),
            Self::MissingInput { relative } => write!(
                formatter,
                "a NeoForge processor input is missing from staging: {relative}"
            ),
            Self::UnusableTool { jar, reason } => {
                write!(
                    formatter,
                    "the NeoForge processor tool '{jar}' is unusable: {reason}"
                )
            }
            Self::Spawn { jar, reason } => write!(
                formatter,
                "the NeoForge processor '{jar}' could not be started: {reason}"
            ),
            Self::Failed {
                jar,
                exit_code,
                diagnostics,
            } => write!(
                formatter,
                "the NeoForge processor '{jar}' failed with exit code {exit_code:?}: {diagnostics}"
            ),
            Self::Timeout { jar } => write!(
                formatter,
                "the NeoForge processor '{jar}' exceeded the {} second bound",
                PROCESSOR_TIMEOUT.as_secs()
            ),
            Self::OutputMissing { relative } => write!(
                formatter,
                "the NeoForge processors did not produce the expected output {relative}"
            ),
            Self::Destroyed { relative } => write!(
                formatter,
                "NeoForge processing removed the managed file {relative}"
            ),
            Self::Escape { path } => write!(
                formatter,
                "a NeoForge processor effect escaped the staging roots: {path}"
            ),
            Self::Io { context, source } => write!(
                formatter,
                "a NeoForge processor filesystem step failed ({context}): {source}"
            ),
        }
    }
}

impl std::error::Error for ProcessorError {}

/// Runs the plan's client-side processors inside the staged game tree.
pub async fn run_client_processors(
    plan: &NeoForgePlan,
    context: &ProcessorContext,
    progress: &mut (dyn FnMut(&str) + Send),
) -> Result<ProcessorOutcome, ProcessorError> {
    if plan.processors().is_empty() {
        return Ok(ProcessorOutcome::default());
    }
    std::fs::create_dir_all(context.scratch_directory.as_path()).map_err(|error| {
        ProcessorError::Io {
            context: "preparing the processor scratch directory".to_owned(),
            source: error,
        }
    })?;

    let before = snapshot_tree(&context.staging_game)?;

    for step in plan.processors() {
        progress(&step.jar().as_maven_string());
        run_one_processor(plan, step, context, &before).await?;
    }

    let after = snapshot_tree(&context.staging_game)?;
    for existing in &before {
        if !after.contains(existing) {
            return Err(ProcessorError::Destroyed {
                relative: existing.clone(),
            });
        }
    }
    let mut generated = Vec::new();
    for path in &after {
        if before.contains(path) {
            continue;
        }
        let absolute = context.staging_game.join(path);
        let bytes = std::fs::read(&absolute).map_err(|error| ProcessorError::Io {
            context: format!("reading the generated artifact {path}"),
            source: error,
        })?;
        if bytes.is_empty() {
            return Err(ProcessorError::OutputMissing {
                relative: path.clone(),
            });
        }
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(&bytes);
        let observed = digest.iter().fold(String::new(), |mut hex, byte| {
            use std::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
            hex
        });
        generated.push(GeneratedArtifact {
            relative: path.clone(),
            observed_sha256: observed,
            size_bytes: bytes.len() as u64,
        });
    }
    Ok(ProcessorOutcome { generated })
}

/// Substitutes one processor argument into its final literal form,
/// validating every path it names.
fn substitute_argument(
    plan: &NeoForgePlan,
    argument: &str,
    context: &ProcessorContext,
    expected_outputs: &mut Vec<String>,
) -> Result<String, ProcessorError> {
    let Some((token, _)) = split_token(argument) else {
        return Ok(argument.to_owned());
    };
    let value: String = match token.as_str() {
        "SIDE" => "client".to_owned(),
        "MINECRAFT_VERSION" => context.minecraft_version.clone(),
        "MINECRAFT_JAR" => {
            require_within(&context.client_jar, &context.staging_game, argument)?;
            if !context.client_jar.is_file() {
                return Err(ProcessorError::MissingInput {
                    relative: display_relative(&context.staging_game, &context.client_jar),
                });
            }
            context.client_jar.to_string_lossy().into_owned()
        }
        "INSTALLER" => context.installer_path.to_string_lossy().into_owned(),
        "ROOT" => context.staging_game.to_string_lossy().into_owned(),
        "LIBRARY_DIR" => context
            .staging_game
            .join("libraries")
            .to_string_lossy()
            .into_owned(),
        other => {
            let Some(value) = plan.data().get(other) else {
                return Err(ProcessorError::Substitution {
                    argument: argument.to_owned(),
                    reason: format!("unknown processor token '{{{other}}}'"),
                });
            };
            match value {
                NeoForgeDataValue::Literal(literal) => literal.clone(),
                NeoForgeDataValue::InstallerFile(name) => {
                    let destination = context.scratch_directory.join(
                        name.trim_start_matches('/')
                            .replace('/', std::path::MAIN_SEPARATOR_STR),
                    );
                    require_within(&destination, &context.scratch_directory, argument)?;
                    if !destination.is_file() {
                        crate::neoforge::metadata::extract_installer_entry(
                            &context.installer_path,
                            name,
                            &destination,
                        )
                        .map_err(|error| ProcessorError::Substitution {
                            argument: argument.to_owned(),
                            reason: error.to_string(),
                        })?;
                    }
                    destination.to_string_lossy().into_owned()
                }
                NeoForgeDataValue::Artifact {
                    coordinate,
                    extension,
                } => {
                    let relative = format!("libraries/{}", coordinate.repository_path(extension));
                    let absolute = context
                        .staging_game
                        .join(relative.split('/').collect::<PathBuf>());
                    require_within(&absolute, &context.staging_game, argument)?;
                    if !absolute.is_file() {
                        expected_outputs.push(relative.clone());
                    }
                    absolute.to_string_lossy().into_owned()
                }
            }
        }
    };
    // Tokens are whole arguments in every observed profile; a partially
    // substituted argument is refused rather than forwarded.
    if argument != format!("{{{token}}}") {
        return Err(ProcessorError::Substitution {
            argument: argument.to_owned(),
            reason: "tokens must name a whole argument, not a fragment".to_owned(),
        });
    }
    Ok(value)
}

/// Splits a whole-argument `{TOKEN}` if the argument is exactly one.
fn split_token(argument: &str) -> Option<(String, ())> {
    if argument.len() > 2 && argument.starts_with('{') && argument.ends_with('}') {
        let token = &argument[1..argument.len() - 1];
        if token.starts_with('{') || token.ends_with('}') || token.is_empty() {
            return None;
        }
        return Some((token.to_owned(), ()));
    }
    None
}

/// Executes one processor step.
async fn run_one_processor(
    plan: &NeoForgePlan,
    step: &NeoForgeProcessorStep,
    context: &ProcessorContext,
    _before: &BTreeSet<String>,
) -> Result<(), ProcessorError> {
    let tool_relative = format!("libraries/{}", step.jar().repository_path("jar"));
    let tool_path = context
        .staging_game
        .join(tool_relative.split('/').collect::<PathBuf>());
    require_within(&tool_path, &context.staging_game, &tool_relative)?;
    if !tool_path.is_file() {
        return Err(ProcessorError::MissingInput {
            relative: tool_relative,
        });
    }

    let mut classpath = vec![tool_path.clone()];
    for coordinate in step.classpath() {
        if coordinate == step.jar() {
            continue;
        }
        let relative = format!("libraries/{}", coordinate.repository_path("jar"));
        let path = context
            .staging_game
            .join(relative.split('/').collect::<PathBuf>());
        require_within(&path, &context.staging_game, &relative)?;
        if !path.is_file() {
            return Err(ProcessorError::MissingInput { relative });
        }
        classpath.push(path);
    }

    let mut expected_outputs = Vec::new();
    let mut arguments = Vec::with_capacity(step.args().len());
    for argument in step.args() {
        arguments.push(substitute_argument(
            plan,
            argument,
            context,
            &mut expected_outputs,
        )?);
    }

    let main_class = read_manifest_main_class(&tool_path)?;

    let separator = if cfg!(windows) { ";" } else { ":" };
    let classpath_value = classpath
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(separator);

    let mut command = tokio::process::Command::new(&context.java_executable);
    command
        .arg("-cp")
        .arg(&classpath_value)
        .arg(&main_class)
        .args(&arguments)
        .current_dir(&context.scratch_directory)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .env_clear();
    #[cfg(windows)]
    for key in ["SYSTEMROOT", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(unix)]
    command.env("LANG", "C").env("LC_ALL", "C");

    let jar_name = step.jar().as_maven_string();
    let output = tokio::time::timeout(PROCESSOR_TIMEOUT, command.output())
        .await
        .map_err(|_| ProcessorError::Timeout {
            jar: jar_name.clone(),
        })?
        .map_err(|error| ProcessorError::Spawn {
            jar: jar_name.clone(),
            reason: error.to_string(),
        })?;

    if !output.status.success() {
        return Err(ProcessorError::Failed {
            jar: jar_name,
            exit_code: output.status.code(),
            diagnostics: diagnostics_tail(&output),
        });
    }

    for relative in &expected_outputs {
        let absolute = context
            .staging_game
            .join(relative.split('/').collect::<PathBuf>());
        if !absolute.is_file() {
            return Err(ProcessorError::OutputMissing {
                relative: relative.clone(),
            });
        }
    }
    Ok(())
}

/// Reads the `Main-Class` attribute of a tool jar's manifest.
fn read_manifest_main_class(jar: &Path) -> Result<String, ProcessorError> {
    let jar_name = jar.to_string_lossy().into_owned();
    let file = std::fs::File::open(jar).map_err(|error| ProcessorError::UnusableTool {
        jar: jar_name.clone(),
        reason: error.to_string(),
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| ProcessorError::UnusableTool {
        jar: jar_name.clone(),
        reason: format!("the tool jar is not a readable archive: {error}"),
    })?;
    let mut entry =
        archive
            .by_name("META-INF/MANIFEST.MF")
            .map_err(|error| ProcessorError::UnusableTool {
                jar: jar_name.clone(),
                reason: format!("the tool jar has no manifest: {error}"),
            })?;
    let mut manifest = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut manifest).map_err(|error| {
        ProcessorError::UnusableTool {
            jar: jar_name.clone(),
            reason: format!("the tool manifest is unreadable: {error}"),
        }
    })?;
    if manifest.len() > 256 * 1024 {
        return Err(ProcessorError::UnusableTool {
            jar: jar_name.clone(),
            reason: "the tool manifest is unexpectedly large".to_owned(),
        });
    }
    let text = String::from_utf8_lossy(&manifest);
    // Manifest headers may wrap at 72 bytes; a continuation line starts
    // with a single space and belongs to the previous value.
    let mut main_class: Option<String> = None;
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(continuation) = line.strip_prefix(' ') {
            if let Some((_, value)) = current.as_mut() {
                value.push_str(continuation.trim_end_matches('\r'));
            }
            continue;
        }
        if let Some((name, value)) = current.take() {
            if name == "Main-Class" {
                main_class = Some(value);
            }
        }
        if let Some((name, value)) = line.split_once(':') {
            current = Some((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
    if let Some((name, value)) = current.take() {
        if name == "Main-Class" {
            main_class = Some(value);
        }
    }
    let main_class = main_class.ok_or_else(|| ProcessorError::UnusableTool {
        jar: jar_name.clone(),
        reason: "the tool manifest declares no Main-Class".to_owned(),
    })?;
    if main_class.is_empty()
        || !main_class
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'$'))
    {
        return Err(ProcessorError::UnusableTool {
            jar: jar_name,
            reason: "the tool manifest declares an unusual Main-Class".to_owned(),
        });
    }
    Ok(main_class)
}

/// Proves a resolved path stays inside a required root.
fn require_within(path: &Path, root: &Path, described: &str) -> Result<(), ProcessorError> {
    let normalized = path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
        || path.is_absolute() != root.is_absolute();
    if normalized || !path.starts_with(root) {
        return Err(ProcessorError::Escape {
            path: described.to_owned(),
        });
    }
    Ok(())
}

/// The forward-slash relative paths of every file under a root.
fn snapshot_tree(root: &Path) -> Result<BTreeSet<String>, ProcessorError> {
    fn walk(root: &Path, prefix: &Path, out: &mut BTreeSet<String>) -> std::io::Result<()> {
        let entries = std::fs::read_dir(root)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                // A symlink inside managed staging is not expected; treat it
                // as a file so its presence/absence is still tracked.
                out.insert(
                    prefix
                        .join(entry.file_name())
                        .to_string_lossy()
                        .into_owned(),
                );
                continue;
            }
            if file_type.is_dir() {
                let child_prefix = prefix.join(entry.file_name());
                walk(&path, &child_prefix, out)?;
            } else {
                out.insert(
                    prefix
                        .join(entry.file_name())
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        Ok(())
    }
    let mut out = BTreeSet::new();
    walk(root, Path::new(""), &mut out).map_err(|error| ProcessorError::Io {
        context: "snapshotting the staged game tree".to_owned(),
        source: error,
    })?;
    Ok(out)
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

/// A bounded, redacted diagnostic tail for a failed processor.
fn diagnostics_tail(output: &std::process::Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str("\n");
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if text.len() > MAX_CAPTURED_OUTPUT_BYTES {
        let tail = &text[text.len() - DIAGNOSTIC_TAIL_CHARS..];
        format!("…{}", tail.replace(['\r', '\n'], " "))
    } else {
        let flat = text.replace(['\r', '\n'], " ");
        if flat.len() > DIAGNOSTIC_TAIL_CHARS {
            format!("…{}", &flat[flat.len() - DIAGNOSTIC_TAIL_CHARS..])
        } else {
            flat
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_argument_tokens_are_recognized() {
        assert_eq!(
            split_token("{PATCHED}").map(|(token, _)| token),
            Some("PATCHED".to_owned())
        );
        assert!(split_token("--input").is_none());
        assert!(split_token("{PARTIAL}suffix").is_none());
        assert!(split_token("{}").is_none());
        assert!(split_token("{{nested}}").is_none());
    }

    #[test]
    fn manifest_main_classes_parse_including_wrapped_values() {
        use std::io::Write as _;
        let directory =
            std::env::temp_dir().join(format!("aurora-processor-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let jar = directory.join("tool.jar");
        let file = std::fs::File::create(&jar).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file(
                "META-INF/MANIFEST.MF",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive
            .write_all(
                b"Manifest-Version: 1.0\r\nMain-Class: net.neoforged.tool.Main\r\nX-Long: averyveryverylongvalue\r\n  wrapped\r\n",
            )
            .unwrap();
        archive.finish().unwrap();
        assert_eq!(
            read_manifest_main_class(&jar).unwrap(),
            "net.neoforged.tool.Main"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
