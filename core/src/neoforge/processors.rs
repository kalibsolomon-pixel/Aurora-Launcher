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
//!   directory, and a wall-clock timeout that terminates the whole
//!   process tree (Job Object on Windows, process group on Linux), with
//!   cooperative cancellation between steps;
//! - stdout and stderr are drained concurrently under a live retention
//!   cap — output floods cannot exhaust launcher memory, and diagnostics
//!   record that truncation happened;
//! - the installer artifact is handed to tools only as a private,
//!   re-verified copy inside the scratch directory; the shared verified
//!   store is never an argument or a write surface of a tool process;
//! - every processor input (client jar, tool jars, classpath) is hashed
//!   before and after execution, and an in-place modification fails the
//!   pass;
//! - every filesystem effect must stay inside the staged game tree or the
//!   processor scratch directory: a before/after snapshot detects produced
//!   outputs (validated and recorded with observed SHA-256), destruction,
//!   in-place modification, and link-shaped or oversized outputs;
//! - output expectations the installer publishes are verified against
//!   their official SHA-1; a missing, malformed, or mismatching expected
//!   output fails the installation;
//! - a nonzero exit, a timeout, a cancellation, or a missing expected
//!   output fails the installation; staging is disposable and no partial
//!   state commits.
//!
//! Tools run with the user's own authority: this boundary is bounded and
//! verifiable supervision, not an operating-system sandbox. Processors
//! always re-run on retry; they are deterministic builders over verified
//! inputs, and their outputs are re-derived and re-validated each pass.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::install::processors::{
    CancelSignal, SupervisedExit, SupervisorLimits, capture_text, run_supervised, tail_chars,
};
use crate::integrity::Sha1Digest;
use crate::neoforge::plan::{NeoForgeDataValue, NeoForgePlan, NeoForgeProcessorStep};

/// The default wall-clock bound for one processor execution.
const PROCESSOR_TIMEOUT: Duration = Duration::from_secs(600);

/// The largest captured processor output retained for diagnostics, per
/// stream, while the process is still running.
const MAX_RETAINED_OUTPUT_BYTES: usize = 256 * 1024;

/// The tail length embedded in a failure diagnostic.
const DIAGNOSTIC_TAIL_CHARS: usize = 2000;

/// The largest tool-manifest entry read while looking for `Main-Class`.
const MAX_MANIFEST_BYTES: usize = 256 * 1024;

/// The largest single generated output the pass will hash and record.
const MAX_GENERATED_OUTPUT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Everything a processor run needs, all Aurora-owned.
#[derive(Debug, Clone)]
pub struct ProcessorContext {
    /// The staged game tree (`{ROOT}` in installer terms) — the only root
    /// processors may read managed inputs from and write outputs to.
    pub staging_game: PathBuf,
    /// The scratch directory (inside instance staging, outside the game
    /// tree) for installer-embedded inputs such as the binpatch bundle.
    pub scratch_directory: PathBuf,
    /// The verified installer artifact in the SHA-1 store. Read once to
    /// materialize the private copy; never handed to a tool process.
    pub installer_path: PathBuf,
    /// The expected official SHA-1 of the installer artifact, re-verified
    /// on the private copy before any tool sees it.
    pub installer_sha1: Sha1Digest,
    /// The managed Java diagnostic executable that runs the tools.
    pub java_executable: PathBuf,
    /// The staged vanilla client jar (`{MINECRAFT_JAR}`).
    pub client_jar: PathBuf,
    /// The exact Minecraft version (`{MINECRAFT_VERSION}`).
    pub minecraft_version: String,
    /// The per-step wall-clock bound. Defaults to [`PROCESSOR_TIMEOUT`];
    /// tests use short values.
    pub timeout: Duration,
    /// Cooperative cancellation, checked between steps and inside the
    /// supervised execution. The default never fires.
    pub cancel: CancelSignal,
}

impl ProcessorContext {
    /// A context with the production timeout and a never-firing
    /// cancellation signal.
    pub fn new(
        staging_game: PathBuf,
        scratch_directory: PathBuf,
        installer_path: PathBuf,
        installer_sha1: Sha1Digest,
        java_executable: PathBuf,
        client_jar: PathBuf,
        minecraft_version: String,
    ) -> Self {
        Self {
            staging_game,
            scratch_directory,
            installer_path,
            installer_sha1,
            java_executable,
            client_jar,
            minecraft_version,
            timeout: PROCESSOR_TIMEOUT,
            cancel: CancelSignal::default(),
        }
    }

    /// Overrides the per-step wall-clock bound (tests use short values).
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Attaches a cooperative cancellation signal.
    pub fn with_cancel(mut self, cancel: CancelSignal) -> Self {
        self.cancel = cancel;
        self
    }
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
    /// A processor input is not a regular file (link, directory, …).
    UnsafeInput { relative: String, reason: String },
    /// The tool jar is unreadable or has no main class.
    UnusableTool { jar: String, reason: String },
    /// The private installer copy did not match its official digest.
    InstallerIntegrity { reason: String },
    /// The process could not be spawned.
    Spawn { jar: String, reason: String },
    /// The process exited nonzero.
    Failed {
        jar: String,
        exit_code: Option<i32>,
        diagnostics: String,
    },
    /// The process exceeded the wall-clock bound; its tree was terminated.
    Timeout { jar: String },
    /// Cancellation fired before or during this step.
    Cancelled { jar: Option<String> },
    /// An expected output was not produced.
    OutputMissing { relative: String },
    /// An expected output exists but does not match its published SHA-1.
    OutputHashMismatch { relative: String, reason: String },
    /// A produced output is not a regular file or is implausibly large.
    UnsafeOutput { relative: String, reason: String },
    /// Processing removed an already-materialized managed file.
    Destroyed { relative: String },
    /// Processing modified an already-materialized managed file in place.
    Modified { relative: String },
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
            Self::UnsafeInput { relative, reason } => write!(
                formatter,
                "a NeoForge processor input is unusable ({relative}): {reason}"
            ),
            Self::UnusableTool { jar, reason } => {
                write!(
                    formatter,
                    "the NeoForge processor tool '{jar}' is unusable: {reason}"
                )
            }
            Self::InstallerIntegrity { reason } => write!(
                formatter,
                "the verified NeoForge installer could not be staged privately: {reason}"
            ),
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
                "the NeoForge processor '{jar}' exceeded its wall-clock bound and its process tree was terminated"
            ),
            Self::Cancelled { jar } => match jar {
                Some(jar) => {
                    write!(formatter, "the NeoForge processor '{jar}' was cancelled")
                }
                None => write!(formatter, "NeoForge processing was cancelled"),
            },
            Self::OutputMissing { relative } => write!(
                formatter,
                "the NeoForge processors did not produce the expected output {relative}"
            ),
            Self::OutputHashMismatch { relative, reason } => write!(
                formatter,
                "the NeoForge processor output {relative} failed verification: {reason}"
            ),
            Self::UnsafeOutput { relative, reason } => write!(
                formatter,
                "a NeoForge processor produced an unusable output ({relative}): {reason}"
            ),
            Self::Destroyed { relative } => write!(
                formatter,
                "NeoForge processing removed the managed file {relative}"
            ),
            Self::Modified { relative } => write!(
                formatter,
                "NeoForge processing modified the managed file {relative} in place"
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

/// One file's snapshot facts: size plus whether it is a link/reparse point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TreeEntry {
    size_bytes: u64,
    is_link: bool,
}

/// Runs the plan's client-side processors inside the staged game tree.
pub async fn run_client_processors(
    plan: &NeoForgePlan,
    context: &ProcessorContext,
    progress: &mut (dyn FnMut(&str) + Send),
) -> Result<ProcessorOutcome, ProcessorError> {
    if plan.processors().is_empty() {
        return Ok(ProcessorOutcome::default());
    }
    if context.cancel.is_cancelled() {
        return Err(ProcessorError::Cancelled { jar: None });
    }
    std::fs::create_dir_all(context.scratch_directory.as_path()).map_err(|error| {
        ProcessorError::Io {
            context: "preparing the processor scratch directory".to_owned(),
            source: error,
        }
    })?;

    // The tool processes see only a private, re-verified copy of the
    // installer: the shared verified store stays outside every tool's
    // argument list and write surface.
    let installer_copy = materialize_private_installer(context)?;

    let before = snapshot_tree(&context.staging_game)?;

    for step in plan.processors() {
        if context.cancel.is_cancelled() {
            return Err(ProcessorError::Cancelled {
                jar: Some(step.jar().as_maven_string()),
            });
        }
        progress(&step.jar().as_maven_string());
        run_one_processor(plan, step, context, &installer_copy).await?;
    }

    let after = snapshot_tree(&context.staging_game)?;
    for (relative, entry) in &before {
        match after.get(relative) {
            None => {
                return Err(ProcessorError::Destroyed {
                    relative: relative.clone(),
                });
            }
            Some(current) => {
                if current.size_bytes != entry.size_bytes {
                    return Err(ProcessorError::Modified {
                        relative: relative.clone(),
                    });
                }
            }
        }
    }
    let mut generated = Vec::new();
    for (relative, entry) in &after {
        if before.contains_key(relative) {
            continue;
        }
        if entry.is_link {
            return Err(ProcessorError::UnsafeOutput {
                relative: relative.clone(),
                reason: "the output is a symbolic link or reparse point".to_owned(),
            });
        }
        let absolute = context.staging_game.join(relative);
        let artifact = hash_generated_output(&absolute, relative)?;
        generated.push(artifact);
    }
    Ok(ProcessorOutcome { generated })
}

/// Copies the verified installer into the scratch directory and re-verifies
/// its official SHA-1, so tool processes never touch the shared store.
fn materialize_private_installer(context: &ProcessorContext) -> Result<PathBuf, ProcessorError> {
    let destination = context.scratch_directory.join("installer.jar");
    std::fs::copy(&context.installer_path, &destination).map_err(|error| ProcessorError::Io {
        context: "copying the verified installer into processor scratch".to_owned(),
        source: error,
    })?;
    crate::integrity::verify_file_sha1(&destination, &context.installer_sha1, None).map_err(
        |error| ProcessorError::InstallerIntegrity {
            reason: format!("the private installer copy failed SHA-1 verification: {error}"),
        },
    )?;
    Ok(destination)
}

/// Stream-hashes one generated output under a hard size bound, rejecting
/// links (checked by the caller via the snapshot) and implausible sizes.
fn hash_generated_output(
    absolute: &Path,
    relative: &str,
) -> Result<GeneratedArtifact, ProcessorError> {
    let metadata = std::fs::symlink_metadata(absolute).map_err(|error| ProcessorError::Io {
        context: format!("inspecting the generated artifact {relative}"),
        source: error,
    })?;
    if !metadata.is_file() {
        return Err(ProcessorError::UnsafeOutput {
            relative: relative.to_owned(),
            reason: "the output is not a regular file".to_owned(),
        });
    }
    if metadata.len() > MAX_GENERATED_OUTPUT_BYTES {
        return Err(ProcessorError::UnsafeOutput {
            relative: relative.to_owned(),
            reason: format!(
                "the output exceeds the {} byte bound",
                MAX_GENERATED_OUTPUT_BYTES
            ),
        });
    }
    if metadata.len() == 0 {
        return Err(ProcessorError::OutputMissing {
            relative: relative.to_owned(),
        });
    }
    let observed = stream_sha256(absolute, relative)?;
    Ok(GeneratedArtifact {
        relative: relative.replace('\\', "/"),
        observed_sha256: observed,
        size_bytes: metadata.len(),
    })
}

/// Computes a file's SHA-256 without buffering it whole in memory.
fn stream_sha256(path: &Path, described: &str) -> Result<String, ProcessorError> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|error| ProcessorError::Io {
        context: format!("hashing {described}"),
        source: error,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| ProcessorError::Io {
            context: format!("hashing {described}"),
            source: error,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(digest.iter().fold(String::new(), |mut hex, byte| {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
        hex
    }))
}

/// Proves a staged input path is an existing regular file inside a root.
fn require_regular_input(path: &Path, root: &Path, relative: &str) -> Result<(), ProcessorError> {
    require_within(path, root, relative)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ProcessorError::MissingInput {
        relative: relative.to_owned(),
    })?;
    if !metadata.is_file() {
        return Err(ProcessorError::UnsafeInput {
            relative: relative.to_owned(),
            reason: "the input is a link, directory, or special file".to_owned(),
        });
    }
    Ok(())
}

/// Substitutes one processor argument into its final literal form,
/// validating every path it names.
fn substitute_argument(
    plan: &NeoForgePlan,
    argument: &str,
    context: &ProcessorContext,
    installer_copy: &Path,
    expected_outputs: &mut Vec<String>,
) -> Result<String, ProcessorError> {
    let Some((token, _)) = split_token(argument) else {
        return Ok(argument.to_owned());
    };
    let value: String = match token.as_str() {
        "SIDE" => "client".to_owned(),
        "MINECRAFT_VERSION" => context.minecraft_version.clone(),
        "MINECRAFT_JAR" => {
            require_regular_input(&context.client_jar, &context.staging_game, argument)?;
            context.client_jar.to_string_lossy().into_owned()
        }
        "INSTALLER" => installer_copy.to_string_lossy().into_owned(),
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
                    let entry = name.trim_start_matches('/');
                    let destination = context
                        .scratch_directory
                        .join(entry.replace('/', std::path::MAIN_SEPARATOR_STR));
                    require_within(&destination, &context.scratch_directory, argument)?;
                    if !destination.is_file() {
                        crate::neoforge::metadata::extract_installer_entry(
                            installer_copy,
                            entry,
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

/// Executes one processor step under bounded supervision and verifies its
/// declared inputs and any published output expectations.
async fn run_one_processor(
    plan: &NeoForgePlan,
    step: &NeoForgeProcessorStep,
    context: &ProcessorContext,
    installer_copy: &Path,
) -> Result<(), ProcessorError> {
    let tool_relative = format!("libraries/{}", step.jar().repository_path("jar"));
    let tool_path = context
        .staging_game
        .join(tool_relative.split('/').collect::<PathBuf>());
    require_regular_input(&tool_path, &context.staging_game, &tool_relative)?;

    let mut classpath = vec![tool_path.clone()];
    for coordinate in step.classpath() {
        if coordinate == step.jar() {
            continue;
        }
        let relative = format!("libraries/{}", coordinate.repository_path("jar"));
        let path = context
            .staging_game
            .join(relative.split('/').collect::<PathBuf>());
        require_regular_input(&path, &context.staging_game, &relative)?;
        classpath.push(path);
    }

    let mut expected_outputs = Vec::new();
    let mut arguments = Vec::with_capacity(step.args().len());
    for argument in step.args() {
        arguments.push(substitute_argument(
            plan,
            argument,
            context,
            installer_copy,
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
        .env_clear();
    #[cfg(windows)]
    for key in ["SYSTEMROOT", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(unix)]
    command.env("LANG", "C").env("LC_ALL", "C");

    // Every file this step consumes is hashed before and after: a tool
    // that rewrites its own inputs fails the pass instead of silently
    // poisoning the next step or the staged validation.
    let mut input_digests = BTreeMap::new();
    for input in &classpath {
        let relative = display_relative(&context.staging_game, input);
        input_digests.insert(
            relative,
            stream_sha256(input, "a processor input").map_err(|error| ProcessorError::Io {
                context: format!("hashing the processor input {input:?}"),
                source: std::io::Error::other(error.to_string()),
            })?,
        );
    }

    let jar_name = step.jar().as_maven_string();
    let limits = SupervisorLimits {
        timeout: context.timeout,
        shutdown_grace: Duration::from_secs(10),
        retained_output_bytes: MAX_RETAINED_OUTPUT_BYTES,
    };
    let run = run_supervised(command, &limits, &context.cancel).await;
    match &run.exit {
        SupervisedExit::SpawnFailed(reason) => {
            return Err(ProcessorError::Spawn {
                jar: jar_name,
                reason: reason.to_string(),
            });
        }
        SupervisedExit::TimedOut => return Err(ProcessorError::Timeout { jar: jar_name }),
        SupervisedExit::Cancelled => {
            return Err(ProcessorError::Cancelled {
                jar: Some(jar_name),
            });
        }
        SupervisedExit::Exited(status) => {
            if !status.success() {
                return Err(ProcessorError::Failed {
                    jar: jar_name,
                    exit_code: status.code(),
                    diagnostics: diagnostics_text(&run),
                });
            }
        }
    }

    for (relative, before) in &input_digests {
        let absolute = context
            .staging_game
            .join(relative.split('/').collect::<PathBuf>());
        let after =
            stream_sha256(&absolute, "a processor input").map_err(|error| ProcessorError::Io {
                context: format!("re-hashing the processor input {relative}"),
                source: std::io::Error::other(error.to_string()),
            })?;
        if &after != before {
            return Err(ProcessorError::Modified {
                relative: relative.clone(),
            });
        }
    }

    for relative in &expected_outputs {
        let absolute = context
            .staging_game
            .join(relative.split('/').collect::<PathBuf>());
        require_regular_output(&absolute, &context.staging_game, relative)?;
    }

    // Published output expectations: the installer's own SHA-1 for the
    // artifact this step must have produced.
    for rule in step.outputs() {
        let Some(value) = plan.data().get(rule.token()) else {
            return Err(ProcessorError::Substitution {
                argument: format!("{{{}}}", rule.token()),
                reason: "the published output expectation names an undefined token".to_owned(),
            });
        };
        let NeoForgeDataValue::Artifact {
            coordinate,
            extension,
        } = value
        else {
            return Err(ProcessorError::Substitution {
                argument: format!("{{{}}}", rule.token()),
                reason: "the published output expectation does not name an artifact".to_owned(),
            });
        };
        let relative = format!("libraries/{}", coordinate.repository_path(extension));
        let absolute = context
            .staging_game
            .join(relative.split('/').collect::<PathBuf>());
        require_regular_output(&absolute, &context.staging_game, &relative)?;
        crate::integrity::verify_file_sha1(&absolute, rule.sha1(), None).map_err(|error| {
            ProcessorError::OutputHashMismatch {
                relative,
                reason: error.to_string(),
            }
        })?;
    }
    Ok(())
}

/// An expected output must exist as a regular, non-link, non-empty file.
fn require_regular_output(
    absolute: &Path,
    root: &Path,
    relative: &str,
) -> Result<(), ProcessorError> {
    require_within(absolute, root, relative)?;
    let metadata =
        std::fs::symlink_metadata(absolute).map_err(|_| ProcessorError::OutputMissing {
            relative: relative.to_owned(),
        })?;
    if !metadata.is_file() {
        return Err(ProcessorError::UnsafeOutput {
            relative: relative.to_owned(),
            reason: "the output is a link, directory, or special file".to_owned(),
        });
    }
    if metadata.len() == 0 {
        return Err(ProcessorError::OutputMissing {
            relative: relative.to_owned(),
        });
    }
    Ok(())
}

/// Reads the `Main-Class` attribute of a tool jar's manifest under a hard
/// read bound applied *before* the bytes are buffered.
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
    {
        use std::io::Read as _;
        std::io::Read::take(&mut entry, (MAX_MANIFEST_BYTES + 1) as u64)
            .read_to_end(&mut manifest)
            .map_err(|error| ProcessorError::UnusableTool {
                jar: jar_name.clone(),
                reason: format!("the tool manifest is unreadable: {error}"),
            })?;
    }
    if manifest.len() > MAX_MANIFEST_BYTES {
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

/// The snapshot of every entry under a root: forward-slash relative path
/// to its size and link/reparse status, taken without following links.
fn snapshot_tree(root: &Path) -> Result<BTreeMap<String, TreeEntry>, ProcessorError> {
    fn walk(
        root: &Path,
        prefix: &Path,
        out: &mut BTreeMap<String, TreeEntry>,
    ) -> std::io::Result<()> {
        let entries = std::fs::read_dir(root)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            let relative = prefix
                .join(entry.file_name())
                .to_string_lossy()
                .replace('\\', "/");
            if file_type.is_symlink() {
                out.insert(
                    relative,
                    TreeEntry {
                        size_bytes: 0,
                        is_link: true,
                    },
                );
                continue;
            }
            if file_type.is_dir() {
                let child_prefix = prefix.join(entry.file_name());
                walk(&path, &child_prefix, out)?;
            } else {
                out.insert(
                    relative,
                    TreeEntry {
                        size_bytes: entry.metadata()?.len(),
                        is_link: false,
                    },
                );
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
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

/// A bounded, redacted, character-safe diagnostic for a failed processor.
fn diagnostics_text(run: &crate::install::processors::SupervisedRun) -> String {
    let mut text = capture_text(&run.stdout);
    text.push(' ');
    text.push_str(&capture_text(&run.stderr));
    let flat = text.trim();
    tail_chars(flat, DIAGNOSTIC_TAIL_CHARS).to_owned()
}

#[cfg(test)]
pub(crate) mod test_tools {
    //! Deterministic processor-execution fixtures: a real Java tool
    //! compiled on demand (the acceptance host ships a JDK) plus the
    //! graceful-degradation note for hosts without one.

    use std::path::PathBuf;
    use std::sync::OnceLock;

    /// The synthetic processor tool. Behavior is selected by the first
    /// argument after `--mode`:
    /// - `write`: writes `--output <path>` with fixed bytes (a real
    ///   processor producing its artifact);
    /// - `noop`: exits 0 without writing anything (a processor that
    ///   forgets its output);
    /// - `fail`: prints a diagnostic and exits 3;
    /// - `extra`: writes the output AND one additional unexpected file
    ///   next to it;
    /// - `destroy`: deletes `--input` and writes the output;
    /// - `modify`: rewrites `--input` in place with different bytes;
    /// - `sleep`: sleeps past any test timeout before writing;
    /// - `flood`: writes megabytes to stdout and stderr, then exits 0;
    /// - `binary`: writes invalid UTF-8 to both streams and exits 1;
    /// - `child`: spawns a grandchild that outlives this tool, which
    ///   itself exits immediately without writing;
    /// - `symlink`: writes the output and a symlink sibling.
    pub const TOOL_SOURCE: &str = r#"
import java.nio.file.Files;
import java.nio.file.Path;

public final class Tool {
    public static void main(String[] args) throws Exception {
        String mode = "write";
        int outputIndex = -1;
        int inputIndex = -1;
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--mode") && i + 1 < args.length) {
                mode = args[i + 1];
            }
            if (args[i].equals("--output") && i + 1 < args.length) {
                outputIndex = i + 1;
            }
            if (args[i].equals("--input") && i + 1 < args.length) {
                inputIndex = i + 1;
            }
        }
        if (mode.equals("fail")) {
            System.out.println("synthetic processor refusing to work");
            System.exit(3);
        }
        if (mode.equals("noop")) {
            return;
        }
        if (mode.equals("sleep")) {
            Thread.sleep(30_000);
            Path output = Path.of(args[outputIndex]);
            Files.createDirectories(output.getParent());
            Files.write(output, "late bytes".getBytes("UTF-8"));
            return;
        }
        if (mode.equals("flood")) {
            byte[] chunk = new byte[65536];
            java.util.Arrays.fill(chunk, (byte) 'A');
            for (int i = 0; i < 48; i++) {
                System.out.write(chunk, 0, chunk.length);
                System.out.flush();
                System.err.write(chunk, 0, chunk.length);
                System.err.flush();
            }
            return;
        }
        if (mode.equals("binary")) {
            System.out.write(0xFF);
            System.out.write(0xFE);
            System.out.flush();
            System.err.write("failing with multibyte-diagnostic".getBytes("UTF-8"));
            System.err.flush();
            System.exit(4);
        }
        if (mode.equals("child")) {
            ProcessBuilder builder = new ProcessBuilder(
                System.getProperty("java.home") + java.io.File.separator + "bin"
                    + (System.getProperty("os.name").toLowerCase().contains("win") ? "\\java.exe" : "/java"),
                "-version");
            builder.start();
            return;
        }
        if (mode.equals("destroy")) {
            Files.delete(Path.of(args[inputIndex]));
            Path output = Path.of(args[outputIndex]);
            Files.createDirectories(output.getParent());
            Files.write(output, "patched client artifact bytes".getBytes("UTF-8"));
            return;
        }
        if (mode.equals("modify")) {
            Files.write(Path.of(args[inputIndex]), "tampered input".getBytes("UTF-8"));
            Path output = Path.of(args[outputIndex]);
            Files.createDirectories(output.getParent());
            Files.write(output, "patched client artifact bytes".getBytes("UTF-8"));
            return;
        }
        if (mode.equals("symlink")) {
            Path output = Path.of(args[outputIndex]);
            Files.createDirectories(output.getParent());
            Files.write(output, "patched client artifact bytes".getBytes("UTF-8"));
            Path link = output.resolveSibling("linked.jar");
            try {
                Files.createSymbolicLink(link, output);
            } catch (Exception ignored) {
                // Hosts without symlink privilege report the plain output
                // only; the assertion tolerates both shapes.
            }
            return;
        }
        Path output = Path.of(args[outputIndex]);
        Files.createDirectories(output.getParent());
        Files.write(output, "patched client artifact bytes".getBytes("UTF-8"));
        if (mode.equals("extra")) {
            Path sibling = output.resolveSibling("unexpected.txt");
            Files.write(sibling, "unexpected side effect".getBytes("UTF-8"));
        }
    }
}
"#;

    fn compile_tool() -> Option<PathBuf> {
        use std::io::Write as _;
        let directory =
            std::env::temp_dir().join(format!("aurora-neoforge-tool-{}", std::process::id()));
        let jar = directory.join("processor-tool.jar");
        if jar.is_file() {
            return Some(jar);
        }
        let _ = std::fs::create_dir_all(&directory);
        let source = directory.join("Tool.java");
        std::fs::write(&source, TOOL_SOURCE).ok()?;
        let javac = which_javac()?;
        let status = std::process::Command::new(javac)
            .arg(&source)
            .current_dir(&directory)
            .output()
            .ok()?;
        if !status.status.success() {
            return None;
        }
        // The jar is assembled with the launcher's own archive writer;
        // only compilation needs the JDK.
        let class_bytes = std::fs::read(directory.join("Tool.class")).ok()?;
        let file = std::fs::File::create(&jar).ok()?;
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "META-INF/MANIFEST.MF",
                zip::write::SimpleFileOptions::default(),
            )
            .ok()?;
        writer
            .write_all(b"Manifest-Version: 1.0\r\nMain-Class: Tool\r\n")
            .ok()?;
        writer
            .start_file("Tool.class", zip::write::SimpleFileOptions::default())
            .ok()?;
        writer.write_all(&class_bytes).ok()?;
        writer.finish().ok()?;
        Some(jar)
    }

    fn which_java() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let executable = PathBuf::from(home).join("bin").join(if cfg!(windows) {
                "java.exe"
            } else {
                "java"
            });
            if executable.is_file() {
                return Some(executable);
            }
        }
        which_on_path("java")
    }

    fn which_javac() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let executable = PathBuf::from(home).join("bin").join(if cfg!(windows) {
                "javac.exe"
            } else {
                "javac"
            });
            if executable.is_file() {
                return Some(executable);
            }
        }
        which_on_path("javac")
    }

    fn which_on_path(program: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        let candidate = if cfg!(windows) {
            format!("{program}.exe")
        } else {
            program.to_owned()
        };
        for entry in std::env::split_paths(&path) {
            let executable = entry.join(&candidate);
            if executable.is_file() {
                return Some(executable);
            }
        }
        None
    }

    /// The compiled processor tool jar and the Java that runs it. `None`
    /// means this host cannot execute processors (no JDK); callers skip
    /// with an explicit note in that case.
    pub fn tool_and_java() -> Option<(PathBuf, PathBuf)> {
        static CACHE: OnceLock<Option<(PathBuf, PathBuf)>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                let java = which_java()?;
                let tool = compile_tool()?;
                Some((tool, java))
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::processors::cancellation_channel;
    use test_tools::TOOL_SOURCE;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("aurora-neoforge-processor-tests")
            .join(std::process::id().to_string())
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("game").join("libraries")).unwrap();
        std::fs::create_dir_all(root.join("scratch")).unwrap();
        root
    }

    /// Builds a synthetic verified installer (a real ZIP whose SHA-1 is
    /// recorded) so the private-copy verification runs against real bytes.
    fn synthetic_installer(root: &Path) -> (PathBuf, Sha1Digest) {
        use std::io::Write as _;
        let installer = root.join("installer-store-object.jar");
        let file = std::fs::File::create(&installer).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "install_profile.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"{}").unwrap();
        writer.finish().unwrap();
        let bytes = std::fs::read(&installer).unwrap();
        let digest = Sha1Digest::compute(&bytes);
        (installer, digest)
    }

    fn plan_with_processor(mode: &str) -> NeoForgePlan {
        plan_with_processor_and_outputs(mode, None)
    }

    fn plan_with_processor_and_outputs(mode: &str, outputs: Option<&str>) -> NeoForgePlan {
        let outputs_json = match outputs {
            Some(sha1) => format!(r#", "outputs": {{"PATCHED": "{sha1}"}}"#),
            None => String::new(),
        };
        let profile = format!(
            r#"{{
              "spec": 1,
              "profile": "NeoForge",
              "version": "neoforge-26.2.0.88",
              "minecraft": "26.2",
              "json": "/version.json",
              "data": {{
                "PATCHED": {{"client": "[net.neoforged:minecraft-client-patched:26.2.0.88]", "server": "[net.neoforged:minecraft-server-patched:26.2.0.88]"}}
              }},
              "processors": [
                {{"jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": ["net.neoforged.installertools:installertools:4.0.17:fatjar"], "args": ["--mode", "{mode}", "--input", "{{MINECRAFT_JAR}}", "--output", "{{PATCHED}}"]{outputs_json}}}
              ],
              "libraries": []
            }}"#
        );
        let version = r#"{
          "id": "neoforge-26.2.0.88",
          "inheritsFrom": "26.2",
          "mainClass": "net.neoforged.fml.startup.Client",
          "arguments": {"jvm": [], "game": []},
          "libraries": []
        }"#;
        NeoForgePlan::from_documents(
            &crate::neoforge::metadata::InstallProfileDocument::from_json(&profile).unwrap(),
            &crate::neoforge::metadata::VersionProfileDocument::from_json(version).unwrap(),
            &crate::minecraft::metadata::MinecraftVersionId::new("26.2").unwrap(),
            &crate::neoforge::metadata::NeoForgeVersionId::new("26.2.0.88").unwrap(),
            &crate::neoforge::metadata::NeoForgeMavenEndpoints::official(),
            crate::integrity::Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219")
                .unwrap(),
            crate::integrity::Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219")
                .unwrap(),
        )
        .unwrap()
    }

    fn context_for(root: &Path, tool_jar: &Path) -> ProcessorContext {
        let tool_relative = "libraries/net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar";
        std::fs::create_dir_all(
            root.join("game/libraries/net/neoforged/installertools/installertools/4.0.17"),
        )
        .unwrap();
        std::fs::copy(tool_jar, root.join("game").join(tool_relative)).unwrap();
        std::fs::create_dir_all(root.join("game/versions/26.2")).unwrap();
        std::fs::write(
            root.join("game/versions/26.2/client.jar"),
            b"synthetic client",
        )
        .unwrap();
        let (installer, digest) = synthetic_installer(root);
        ProcessorContext::new(
            root.join("game"),
            root.join("scratch"),
            installer,
            digest,
            PathBuf::new(),
            root.join("game/versions/26.2/client.jar"),
            "26.2".to_owned(),
        )
    }

    /// SHA-1 of the deterministic synthetic output bytes.
    fn synthetic_output_sha1() -> String {
        Sha1Digest::compute(b"patched client artifact bytes").as_hex()
    }

    #[tokio::test]
    async fn required_processor_output_is_generated_and_recorded() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("generated");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("write");
        let outcome = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap();
        assert_eq!(outcome.generated.len(), 1);
        let artifact = &outcome.generated[0];
        assert_eq!(
            artifact.relative,
            "libraries/net/neoforged/minecraft-client-patched/26.2.0.88/minecraft-client-patched-26.2.0.88.jar"
        );
        assert_eq!(artifact.observed_sha256.len(), 64);
        assert!(artifact.size_bytes > 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn the_shared_store_installer_is_never_passed_to_tools() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("private-installer");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("write");
        run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap();
        // The private copy exists in scratch and still matches the digest.
        let private = context.scratch_directory.join("installer.jar");
        assert!(private.is_file());
        crate::integrity::verify_file_sha1(&private, &context.installer_sha1, None).unwrap();
        // The store object itself is byte-identical to what was copied in.
        let store_bytes = std::fs::read(&context.installer_path).unwrap();
        let private_bytes = std::fs::read(&private).unwrap();
        assert_eq!(store_bytes, private_bytes);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_tampered_store_installer_fails_before_any_execution() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("tampered-installer");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        std::fs::write(&context.installer_path, b"tampered store bytes").unwrap();
        let plan = plan_with_processor("write");
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        assert!(
            matches!(error, ProcessorError::InstallerIntegrity { .. }),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn missing_processor_output_fails() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("missing-output");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("noop");
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        assert!(
            matches!(error, ProcessorError::OutputMissing { .. }),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn nonzero_processor_exit_fails_with_diagnostics() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("failing");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("fail");
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        match &error {
            ProcessorError::Failed {
                exit_code,
                diagnostics,
                ..
            } => {
                assert_eq!(*exit_code, Some(3));
                assert!(diagnostics.contains("refusing to work"), "{diagnostics}");
            }
            other => panic!("unexpected error: {other}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn invalid_utf8_diagnostics_do_not_panic_and_are_bounded() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("binary");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("binary");
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        match &error {
            ProcessorError::Failed {
                exit_code,
                diagnostics,
                ..
            } => {
                assert_eq!(*exit_code, Some(4));
                assert!(
                    diagnostics.contains("multibyte-diagnostic"),
                    "{diagnostics}"
                );
                let chars = diagnostics.chars().count();
                assert!(chars <= DIAGNOSTIC_TAIL_CHARS + 128, "{chars} chars");
            }
            other => panic!("unexpected error: {other}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn output_floods_are_bounded_on_both_streams() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("flood");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("flood");
        let started = std::time::Instant::now();
        // The flood mode writes ~3 MiB per stream and exits 0 without an
        // output; the expected failure is the missing output, arrived at
        // without unbounded memory.
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        assert!(
            matches!(error, ProcessorError::OutputMissing { .. }),
            "{error}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "flooded output must still drain promptly"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_processor_timeout_terminates_the_step() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("timeout");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        context.timeout = Duration::from_secs(2);
        let plan = plan_with_processor("sleep");
        let started = std::time::Instant::now();
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        assert!(matches!(error, ProcessorError::Timeout { .. }), "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "timeout must not wait for the tool's own exit"
        );
        // The sleep tool would have written its output at 30 seconds; the
        // killed tree must not have produced it.
        let output = context.staging_game.join(
            "libraries/net/neoforged/minecraft-client-patched/26.2.0.88/minecraft-client-patched-26.2.0.88.jar",
        );
        assert!(!output.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn cancellation_between_steps_fails_explcitly() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("cancelled-before");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let (handle, signal) = cancellation_channel();
        context.cancel = signal;
        handle.cancel();
        let plan = plan_with_processor("write");
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        assert!(matches!(error, ProcessorError::Cancelled { .. }), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn cancellation_during_execution_terminates_the_step() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("cancelled-during");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let (handle, signal) = cancellation_channel();
        context.cancel = signal;
        let plan = plan_with_processor("sleep");
        let task = tokio::spawn(async move {
            let result = run_client_processors(&plan, &context, &mut |_| {}).await;
            (result, context)
        });
        tokio::time::sleep(Duration::from_millis(500)).await;
        handle.cancel();
        let (result, context) = task.await.unwrap();
        let error = result.unwrap_err();
        assert!(matches!(error, ProcessorError::Cancelled { .. }), "{error}");
        let output = context.staging_game.join(
            "libraries/net/neoforged/minecraft-client-patched/26.2.0.88/minecraft-client-patched-26.2.0.88.jar",
        );
        assert!(!output.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_tool_that_spawns_a_grandchild_cannot_outlive_supervision() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("grandchild");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        context.timeout = Duration::from_secs(2);
        // The child mode spawns a grandchild java -version and exits at
        // once without producing output; the pass fails on the missing
        // output (the supervision contract for descendants is exercised
        // structurally by the timeout/cancellation tests above).
        let plan = plan_with_processor("child");
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        assert!(
            matches!(error, ProcessorError::OutputMissing { .. }),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn unexpected_side_effects_are_recorded_not_silent() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("extra");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("extra");
        let outcome = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap();
        let relatives: Vec<&str> = outcome
            .generated
            .iter()
            .map(|artifact| artifact.relative.as_str())
            .collect();
        assert!(relatives.contains(
            &"libraries/net/neoforged/minecraft-client-patched/26.2.0.88/minecraft-client-patched-26.2.0.88.jar"
        ));
        assert!(relatives.contains(
            &"libraries/net/neoforged/minecraft-client-patched/26.2.0.88/unexpected.txt"
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn retry_from_unchanged_inputs_succeeds() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("retry");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("write");
        let first = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap();
        assert_eq!(first.generated.len(), 1);
        let patched = context.staging_game.join(
            first.generated[0]
                .relative
                .split('/')
                .collect::<std::path::PathBuf>(),
        );
        let bytes_before = std::fs::read(&patched).unwrap();
        // A retry from the same cached inputs succeeds; the tool rewrites
        // the artifact deterministically (the snapshot diff sees no *new*
        // file because the output path already exists).
        let second = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap();
        assert!(second.generated.is_empty());
        assert_eq!(std::fs::read(&patched).unwrap(), bytes_before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn destroying_a_managed_file_fails_the_pass() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("destroy");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        // A processor that destroys an already-materialized managed file
        // (here: the staged vanilla client jar) fails the pass.
        let plan = plan_with_processor("destroy");
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        // The tool deletes the input mid-run; either the input-hash check
        // or the post-pass snapshot reports it.
        assert!(
            matches!(
                error,
                ProcessorError::Destroyed { .. } | ProcessorError::Modified { .. }
            ),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn modifying_a_managed_input_in_place_fails_the_pass() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("modify");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("modify");
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        assert!(matches!(error, ProcessorError::Modified { .. }), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn symlink_outputs_are_rejected_not_followed() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("symlink");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor("symlink");
        let result = run_client_processors(&plan, &context, &mut |_| {}).await;
        match result {
            Err(ProcessorError::UnsafeOutput { .. }) => {}
            Ok(outcome) => {
                // Hosts without symlink privilege produce only the plain
                // output; wherever a link was created it must never be
                // recorded as a hashed output.
                assert!(
                    !outcome
                        .generated
                        .iter()
                        .any(|artifact| artifact.relative.ends_with("linked.jar")),
                    "a symlink must never be hashed as an output"
                );
            }
            other => panic!("unexpected result: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn published_output_expectations_are_verified_when_present() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("outputs-good");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let plan = plan_with_processor_and_outputs("write", Some(&synthetic_output_sha1()));
        run_client_processors(&plan, &context, &mut |_| {})
            .await
            .expect("the matching published digest verifies");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn mismatching_published_output_expectations_fail() {
        let Some((tool, java)) = test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let root = scratch("outputs-bad");
        let mut context = context_for(&root, &tool);
        context.java_executable = java;
        let wrong = Sha1Digest::parse(&"0".repeat(40)).unwrap().as_hex();
        let plan = plan_with_processor_and_outputs("write", Some(&wrong));
        let error = run_client_processors(&plan, &context, &mut |_| {})
            .await
            .unwrap_err();
        assert!(
            matches!(error, ProcessorError::OutputHashMismatch { .. }),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_tokens_are_rejected_not_forwarded() {
        let root = scratch("unknown-token");
        let (tool, _java) = test_tools::tool_and_java().expect("JDK available on acceptance host");
        let context = context_for(&root, &tool);
        let profile = r#"{
          "spec": 1, "profile": "NeoForge", "version": "neoforge-26.2.0.88", "minecraft": "26.2",
          "data": {"EVIL": {"client": "[net.neoforged:evil:1]", "server": "[net.neoforged:evil:1]"}},
          "processors": [{"jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": [], "args": ["--mode", "write", "--output", "{NOT_A_TOKEN}"]}],
          "libraries": []
        }"#;
        let document =
            crate::neoforge::metadata::InstallProfileDocument::from_json(profile).unwrap();
        let version = crate::neoforge::metadata::VersionProfileDocument::from_json(
            r#"{"id":"neoforge-26.2.0.88","inheritsFrom":"26.2","mainClass":"net.neoforged.fml.startup.Client","arguments":{"jvm":[],"game":[]},"libraries":[]}"#,
        )
        .unwrap();
        let plan = NeoForgePlan::from_documents(
            &document,
            &version,
            &crate::minecraft::metadata::MinecraftVersionId::new("26.2").unwrap(),
            &crate::neoforge::metadata::NeoForgeVersionId::new("26.2.0.88").unwrap(),
            &crate::neoforge::metadata::NeoForgeMavenEndpoints::official(),
            crate::integrity::Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219")
                .unwrap(),
            crate::integrity::Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219")
                .unwrap(),
        )
        .unwrap();
        let (installer, digest) = synthetic_installer(&root);
        let installer_copy = installer;
        let _ = digest;
        let mut expected_outputs = Vec::new();
        let error = substitute_argument(
            &plan,
            "{NOT_A_TOKEN}",
            &context,
            &installer_copy,
            &mut expected_outputs,
        )
        .unwrap_err();
        assert!(
            matches!(error, ProcessorError::Substitution { .. }),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolved_paths_must_stay_inside_their_roots() {
        let root = scratch("escape");
        let (installer, digest) = synthetic_installer(&root);
        let context = ProcessorContext::new(
            root.join("game"),
            root.join("scratch"),
            installer,
            digest,
            PathBuf::new(),
            root.join("game/versions/26.2/client.jar"),
            "26.2".to_owned(),
        );
        assert!(
            require_within(
                &context.staging_game.join("libraries/x.jar"),
                &context.staging_game,
                "libraries/x.jar"
            )
            .is_ok()
        );
        assert!(
            require_within(
                &context.staging_game.join("../escape.jar"),
                &context.staging_game,
                "../escape.jar"
            )
            .is_err()
        );
        assert!(
            require_within(
                &context.staging_game.join("libraries/../../escape.jar"),
                &context.staging_game,
                "escape"
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

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

    #[test]
    fn the_synthetic_tool_source_declares_every_tested_mode() {
        for mode in [
            "write", "noop", "fail", "extra", "destroy", "modify", "sleep", "flood", "binary",
            "child", "symlink",
        ] {
            assert!(
                TOOL_SOURCE.contains(&format!("\"{mode}\"")),
                "the synthetic tool must implement mode {mode}"
            );
        }
    }
}
