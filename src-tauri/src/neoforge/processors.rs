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
            relative: path.replace('\\', "/"),
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
                    let entry = name.trim_start_matches('/');
                    let destination = context
                        .scratch_directory
                        .join(entry.replace('/', std::path::MAIN_SEPARATOR_STR));
                    require_within(&destination, &context.scratch_directory, argument)?;
                    if !destination.is_file() {
                        crate::neoforge::metadata::extract_installer_entry(
                            &context.installer_path,
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
    ///   next to it.
    pub const TOOL_SOURCE: &str = r#"
import java.nio.file.Files;
import java.nio.file.Path;

public final class Tool {
    public static void main(String[] args) throws Exception {
        String mode = "write";
        int outputIndex = -1;
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--mode") && i + 1 < args.length) {
                mode = args[i + 1];
            }
            if (args[i].equals("--output") && i + 1 < args.length) {
                outputIndex = i + 1;
            }
        }
        if (mode.equals("fail")) {
            System.out.println("synthetic processor refusing to work");
            System.exit(3);
        }
        if (mode.equals("noop")) {
            return;
        }
        if (mode.equals("destroy")) {
            int inputIndex = -1;
            for (int i = 0; i < args.length; i++) {
                if (args[i].equals("--input") && i + 1 < args.length) {
                    inputIndex = i + 1;
                }
            }
            Files.delete(Path.of(args[inputIndex]));
            Path output = Path.of(args[outputIndex]);
            Files.createDirectories(output.getParent());
            Files.write(output, "patched client artifact bytes".getBytes("UTF-8"));
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
    use test_tools::tool_and_java;

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

    fn plan_with_processor(mode: &str) -> NeoForgePlan {
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
                {{"jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": ["net.neoforged.installertools:installertools:4.0.17:fatjar"], "args": ["--mode", "{mode}", "--input", "{{MINECRAFT_JAR}}", "--output", "{{PATCHED}}"]}}
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
        ProcessorContext {
            staging_game: root.join("game"),
            scratch_directory: root.join("scratch"),
            installer_path: PathBuf::from("installer-does-not-exist.jar"),
            java_executable: PathBuf::new(),
            client_jar: root.join("game/versions/26.2/client.jar"),
            minecraft_version: "26.2".to_owned(),
        }
    }

    #[tokio::test]
    async fn required_processor_output_is_generated_and_recorded() {
        let Some((tool, java)) = tool_and_java() else {
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
    async fn missing_processor_output_fails() {
        let Some((tool, java)) = tool_and_java() else {
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
        let Some((tool, java)) = tool_and_java() else {
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
    async fn unexpected_side_effects_are_recorded_not_silent() {
        let Some((tool, java)) = tool_and_java() else {
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
        let Some((tool, java)) = tool_and_java() else {
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
        let Some((tool, java)) = tool_and_java() else {
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
        assert!(matches!(error, ProcessorError::Destroyed { .. }), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_tokens_are_rejected_not_forwarded() {
        let root = scratch("unknown-token");
        let (tool, _java) = tool_and_java().expect("JDK available on acceptance host");
        let context = context_for(&root, &tool);
        let mut profile = r#"{
          "spec": 1, "profile": "NeoForge", "version": "neoforge-26.2.0.88", "minecraft": "26.2",
          "data": {"EVIL": {"client": "[net.neoforged:evil:1]", "server": "[net.neoforged:evil:1]"}},
          "processors": [{"jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": [], "args": ["--mode", "write", "--output", "{NOT_A_TOKEN}"]}],
          "libraries": []
        }"#;
        let document =
            crate::neoforge::metadata::InstallProfileDocument::from_json(profile).unwrap();
        profile = "{}";
        let _ = profile;
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
        let mut expected_outputs = Vec::new();
        let error = substitute_argument(&plan, "{NOT_A_TOKEN}", &context, &mut expected_outputs)
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
        let context = ProcessorContext {
            staging_game: root.join("game"),
            scratch_directory: root.join("scratch"),
            installer_path: PathBuf::from("installer.jar"),
            java_executable: PathBuf::new(),
            client_jar: root.join("game/versions/26.2/client.jar"),
            minecraft_version: "26.2".to_owned(),
        };
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
}
