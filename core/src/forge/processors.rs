//! Bounded execution of the official Forge installer's client processors.
//!
//! Forge's client installation is a real processor chain (download Mojang
//! mappings, rename the client jar to official names, apply binary
//! patches). Aurora executes those processor definitions as narrowly
//! modeled build steps through the F1A supervised boundary — never as a
//! general "run installer" capability:
//!
//! - the argument vector comes only from the normalized plan, substituted
//!   with Aurora-owned values; whole-argument tokens only, bracketed
//!   data references resolved under the staged libraries root, quoted
//!   literals passed verbatim, installer-embedded entries extracted from
//!   a private re-verified installer copy — the shared verified store is
//!   never an argument or write surface of a tool process;
//! - every step runs with the exact managed Java executable, an empty
//!   environment (plus OS floor variables), a bounded working directory,
//!   concurrent bounded output capture, a wall-clock timeout that
//!   terminates the whole process tree, and cooperative cancellation;
//! - chain integrity is explicit: a step's published outputs are verified
//!   against their official SHA-1 before the next step runs, an in-place
//!   modification of any consumed input fails the pass, and the staging
//!   snapshot records size and link status so destruction, modification,
//!   and symlink/reparse outputs are all hard failures;
//! - server-only processor steps never run during a client installation.
//!
//! Tools run with the user's own authority: this boundary is bounded and
//! verifiable supervision, not an operating-system sandbox. One step
//! legitimately performs its own Mojang download (the mappings fetch);
//! its output is verified against the digest Forge or Mojang published before the
//! step ran, which is the honest extent of that step's verification.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::forge::plan::{ForgeDataValue, ForgePlan, ForgeProcessorArgument, ForgeProcessorStep};
use crate::install::processors::{
    CancelSignal, SupervisedExit, SupervisorLimits, capture_text, run_supervised, tail_chars,
};
use crate::integrity::Sha1Digest;

/// The default wall-clock bound for one processor step.
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
    /// The per-step wall-clock bound. Defaults to [`PROCESSOR_TIMEOUT`].
    pub timeout: Duration,
    /// Cooperative cancellation, checked between steps and inside the
    /// supervised execution.
    pub cancel: CancelSignal,
}

impl ProcessorContext {
    /// A context with the production timeout and a never-firing
    /// cancellation signal.
    #[allow(clippy::too_many_arguments)]
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
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
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
    /// Outputs that additionally verified against their official
    /// published SHA-1, mapped by game-relative path.
    pub published_sha1: BTreeMap<String, Sha1Digest>,
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
                "a Forge processor argument could not be resolved safely ('{argument}'): {reason}"
            ),
            Self::MissingInput { relative } => write!(
                formatter,
                "a Forge processor input is missing from staging: {relative}"
            ),
            Self::UnsafeInput { relative, reason } => write!(
                formatter,
                "a Forge processor input is unusable ({relative}): {reason}"
            ),
            Self::UnusableTool { jar, reason } => {
                write!(
                    formatter,
                    "the Forge processor tool '{jar}' is unusable: {reason}"
                )
            }
            Self::InstallerIntegrity { reason } => write!(
                formatter,
                "the verified Forge installer could not be staged privately: {reason}"
            ),
            Self::Spawn { jar, reason } => write!(
                formatter,
                "the Forge processor '{jar}' could not be started: {reason}"
            ),
            Self::Failed {
                jar,
                exit_code,
                diagnostics,
            } => write!(
                formatter,
                "the Forge processor '{jar}' failed with exit code {exit_code:?}: {diagnostics}"
            ),
            Self::Timeout { jar } => write!(
                formatter,
                "the Forge processor '{jar}' exceeded its wall-clock bound and its process tree was terminated"
            ),
            Self::Cancelled { jar } => match jar {
                Some(jar) => write!(formatter, "the Forge processor '{jar}' was cancelled"),
                None => write!(formatter, "Forge processing was cancelled"),
            },
            Self::OutputMissing { relative } => write!(
                formatter,
                "the Forge processors did not produce the expected output {relative}"
            ),
            Self::OutputHashMismatch { relative, reason } => write!(
                formatter,
                "the Forge processor output {relative} failed verification: {reason}"
            ),
            Self::UnsafeOutput { relative, reason } => write!(
                formatter,
                "a Forge processor produced an unusable output ({relative}): {reason}"
            ),
            Self::Destroyed { relative } => write!(
                formatter,
                "Forge processing removed the managed file {relative}"
            ),
            Self::Modified { relative } => write!(
                formatter,
                "Forge processing modified the managed file {relative} in place"
            ),
            Self::Escape { path } => write!(
                formatter,
                "a Forge processor effect escaped the staging roots: {path}"
            ),
            Self::Io { context, source } => write!(
                formatter,
                "a Forge processor filesystem step failed ({context}): {source}"
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
///
/// Steps execute in profile order; each step's published outputs are
/// verified before the next step begins, so a failed step never has its
/// dependents run against partial output.
pub async fn run_client_processors(
    plan: &ForgePlan,
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

    // Tool processes see only a private, re-verified copy of the
    // installer: the shared verified store stays outside every tool's
    // argument list and write surface.
    let installer_copy = materialize_private_installer(context)?;

    let before = snapshot_tree(&context.staging_game)?;
    let mut published = BTreeMap::new();

    for step in plan.processors() {
        if context.cancel.is_cancelled() {
            return Err(ProcessorError::Cancelled {
                jar: Some(step.jar().as_maven_string()),
            });
        }
        progress(&step.jar().as_maven_string());
        let step_published = run_one_processor(plan, step, context, &installer_copy).await?;
        published.extend(step_published);
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
    Ok(ProcessorOutcome {
        generated,
        published_sha1: published,
    })
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

/// Stream-hashes one generated output under a hard size bound.
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
            reason: format!("the output exceeds the {MAX_GENERATED_OUTPUT_BYTES} byte bound"),
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

/// The resolved value of one whole-argument `{TOKEN}`: always a final
/// literal (a tool needs its destination path even when the artifact is
/// one this step is expected to produce), plus the record that the
/// artifact did not exist yet.
struct ResolvedArgument {
    value: String,
    expected_output: Option<String>,
    input_path: Option<PathBuf>,
}

/// Substitutes one processor argument into its final literal form,
/// validating every path it names.
fn substitute_argument(
    plan: &ForgePlan,
    argument: &ForgeProcessorArgument,
    context: &ProcessorContext,
    installer_copy: &Path,
) -> Result<ResolvedArgument, ProcessorError> {
    let plain = |value: String| ResolvedArgument {
        value,
        expected_output: None,
        input_path: None,
    };
    let input = |path: PathBuf, root: &Path| -> Result<ResolvedArgument, ProcessorError> {
        require_regular_input(&path, root, "processor input")?;
        Ok(ResolvedArgument {
            value: path.to_string_lossy().into_owned(),
            expected_output: None,
            input_path: Some(path),
        })
    };
    let token = match argument {
        ForgeProcessorArgument::Literal(value) => return Ok(plain(value.clone())),
        ForgeProcessorArgument::Artifact(coordinate) => {
            let path = context
                .staging_game
                .join("libraries")
                .join(coordinate.repository_path());
            return input(path, &context.staging_game);
        }
        ForgeProcessorArgument::Token(token) => token,
    };
    match token.as_str() {
        "SIDE" => Ok(plain("client".into())),
        "MINECRAFT_VERSION" => Ok(plain(context.minecraft_version.clone())),
        "MINECRAFT_JAR" => input(context.client_jar.clone(), &context.staging_game),
        "INSTALLER" => input(installer_copy.to_path_buf(), &context.scratch_directory),
        "ROOT" => Ok(plain(context.staging_game.to_string_lossy().into_owned())),
        "LIBRARY_DIR" => Ok(plain(
            context
                .staging_game
                .join("libraries")
                .to_string_lossy()
                .into_owned(),
        )),
        other => match plan.data().get(other) {
            None => Err(ProcessorError::Substitution {
                argument: format!("{{{other}}}"),
                reason: "unknown processor token".into(),
            }),
            Some(ForgeDataValue::Literal(value)) => Ok(plain(value.clone())),
            Some(ForgeDataValue::InstallerFile(name)) => {
                let path = context.scratch_directory.join(name.trim_start_matches('/'));
                require_within(&path, &context.scratch_directory, name)?;
                if !path.exists() {
                    crate::forge::metadata::extract_installer_entry(installer_copy, name, &path)
                        .map_err(|error| ProcessorError::Substitution {
                            argument: name.clone(),
                            reason: error.to_string(),
                        })?;
                }
                input(path, &context.scratch_directory)
            }
            Some(ForgeDataValue::Artifact { coordinate }) => {
                let relative = format!("libraries/{}", coordinate.repository_path());
                let path = context.staging_game.join(&relative);
                require_within(&path, &context.staging_game, &relative)?;
                if path.exists() {
                    input(path, &context.staging_game)
                } else {
                    Ok(ResolvedArgument {
                        value: path.to_string_lossy().into_owned(),
                        expected_output: Some(relative),
                        input_path: None,
                    })
                }
            }
        },
    }
}

/// Executes one processor step under bounded supervision and verifies its
/// declared inputs and published outputs. Returns the step's published
/// output digests keyed by game-relative path.
async fn run_one_processor(
    plan: &ForgePlan,
    step: &ForgeProcessorStep,
    context: &ProcessorContext,
    installer_copy: &Path,
) -> Result<BTreeMap<String, Sha1Digest>, ProcessorError> {
    let tool_relative = format!("libraries/{}", step.jar().repository_path());
    let tool_path = context
        .staging_game
        .join(tool_relative.split('/').collect::<PathBuf>());
    require_regular_input(&tool_path, &context.staging_game, &tool_relative)?;

    let mut classpath = vec![tool_path.clone()];
    for coordinate in step.classpath() {
        if coordinate == step.jar() {
            continue;
        }
        let relative = format!("libraries/{}", coordinate.repository_path());
        let path = context
            .staging_game
            .join(relative.split('/').collect::<PathBuf>());
        require_regular_input(&path, &context.staging_game, &relative)?;
        classpath.push(path);
    }

    let mut expected_outputs = Vec::new();
    let mut arguments = Vec::with_capacity(step.args().len());
    let mut consumed_inputs = classpath.clone();
    for (index, argument) in step.arguments().iter().enumerate() {
        let resolved = substitute_argument(plan, argument, context, installer_copy)?;
        let previous = index
            .checked_sub(1)
            .and_then(|i| step.args().get(i))
            .map(String::as_str);
        if matches!(
            previous,
            Some("--input" | "--left" | "--right" | "--names" | "--srg" | "--clean" | "--apply")
        ) && resolved.expected_output.is_some()
        {
            return Err(ProcessorError::MissingInput {
                relative: resolved.expected_output.unwrap(),
            });
        }
        if let Some(path) = resolved.input_path {
            consumed_inputs.push(path);
        }
        if let Some(relative) = resolved.expected_output {
            expected_outputs.push(relative);
        }
        arguments.push(resolved.value);
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
    for input in &consumed_inputs {
        let relative = display_relative(&context.staging_game, input);
        input_digests.insert(
            input.clone(),
            (
                relative,
                stream_sha256(input, "a processor input").map_err(|error| ProcessorError::Io {
                    context: format!("hashing the processor input {input:?}"),
                    source: std::io::Error::other(error.to_string()),
                })?,
            ),
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

    for (absolute, (relative, before)) in &input_digests {
        let root = if absolute.starts_with(&context.staging_game) {
            &context.staging_game
        } else {
            &context.scratch_directory
        };
        require_regular_input(absolute, root, relative)?;
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
    let mut verified = BTreeMap::new();
    for rule in step.outputs() {
        let Some(value) = plan.data().get(rule.token()) else {
            return Err(ProcessorError::Substitution {
                argument: format!("{{{}}}", rule.token()),
                reason: "the published output expectation names an undefined token".to_owned(),
            });
        };
        let ForgeDataValue::Artifact { coordinate } = value else {
            return Err(ProcessorError::Substitution {
                argument: format!("{{{}}}", rule.token()),
                reason: "the published output expectation does not name an artifact".to_owned(),
            });
        };
        let relative = format!("libraries/{}", coordinate.repository_path());
        let absolute = context
            .staging_game
            .join(relative.split('/').collect::<PathBuf>());
        require_regular_output(&absolute, &context.staging_game, &relative)?;
        crate::integrity::verify_file_sha1(&absolute, rule.sha1(), None).map_err(|error| {
            ProcessorError::OutputHashMismatch {
                relative: relative.clone(),
                reason: error.to_string(),
            }
        })?;
        verified.insert(relative, *rule.sha1());
    }
    Ok(verified)
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
    if metadata.len() > MAX_GENERATED_OUTPUT_BYTES {
        return Err(ProcessorError::UnsafeOutput {
            relative: relative.into(),
            reason: "output exceeds the processor size bound".into(),
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
    // Reject links/reparse points in every existing ancestor, including the
    // root, rather than relying on a lexical starts_with check.
    let mut cursor = Some(path);
    while let Some(current) = cursor {
        if let Ok(metadata) = std::fs::symlink_metadata(current) {
            #[cfg(windows)]
            let reparse = {
                use std::os::windows::fs::MetadataExt;
                metadata.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let reparse = false;
            if metadata.file_type().is_symlink() || reparse {
                return Err(ProcessorError::Escape {
                    path: described.into(),
                });
            }
        }
        if current == root {
            break;
        }
        cursor = current.parent();
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
            let metadata = std::fs::symlink_metadata(&path)?;
            #[cfg(windows)]
            let reparse = {
                use std::os::windows::fs::MetadataExt;
                metadata.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let reparse = false;
            if file_type.is_symlink() || reparse {
                return Err(std::io::Error::other(format!(
                    "the staged processor tree contains a link or reparse point: {relative}"
                )));
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
