//! User-selected external Java executables (per-instance custom Java).
//!
//! The managed runtime stays the default and is still used for installation
//! tooling. A custom executable is admitted for *launching the game* only
//! after every check below passes, and it is re-validated immediately before
//! each launch:
//!
//! 1. path shape (absolute, plain, `java(.exe)`/`javaw.exe`);
//! 2. a regular file with no symlink or reparse point on it or any ancestor;
//! 3. a native executable for this machine (PE machine type on Windows,
//!    executable permission elsewhere);
//! 4. execution of `-XshowSettings:properties -version` with a structured
//!    command, cleared environment, no console window, a bounded timeout and
//!    bounded output;
//! 5. the *reported* `java.specification.version` major equals the game
//!    plan's required major, and `os.arch` matches this machine.
//!
//! Folder names, stored metadata and version strings are never trusted.
//! [`ValidatedExternalJava`] can only be constructed by [`validate`].

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;

use crate::instances::settings::JavaSelection;

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const PROBE_OUTPUT_LIMIT: usize = 64 * 1024;
const EXECUTABLE_SIZE_LIMIT: u64 = 64 * 1024 * 1024;

/// A custom executable that passed every check for one required major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExternalJava {
    path: PathBuf,
    facts: JavaFacts,
    identity: [u8; 32],
}

impl ValidatedExternalJava {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn facts(&self) -> &JavaFacts {
        &self.facts
    }

    /// Recheck the selected bytes. On Windows keep this read handle alive
    /// until spawn returns: ordinary writers/deleters cannot replace the EXE.
    pub fn lock_for_launch(&self) -> Result<std::fs::File, ExternalJavaError> {
        let (file, identity) = executable_identity(&self.path)?;
        if identity != self.identity {
            return Err(fail(
                "custom_java_changed",
                "The custom Java executable changed. Check it again before launching.",
            ));
        }
        Ok(file)
    }
}

fn executable_identity(path: &Path) -> Result<(std::fs::File, [u8; 32]), ExternalJavaError> {
    check_file(path.to_str().ok_or_else(|| {
        fail(
            "custom_java_path_invalid",
            "The Java path cannot be represented.",
        )
    })?)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ, no write/delete sharing
    }
    let mut file = options.open(path).map_err(|_| {
        fail(
            "custom_java_not_executable",
            "The selected Java could not be opened for validation.",
        )
    })?;
    let size = file
        .metadata()
        .map_err(|_| fail("custom_java_missing", "The custom Java is unavailable."))?
        .len();
    if size == 0 || size > EXECUTABLE_SIZE_LIMIT {
        return Err(fail(
            "custom_java_not_executable",
            "The selected Java executable has an unsupported size.",
        ));
    }
    let mut hasher = Sha256::new();
    let copied = std::io::copy(
        &mut (&mut file).take(EXECUTABLE_SIZE_LIMIT + 1),
        &mut hasher,
    )
    .map_err(|_| fail("custom_java_missing", "The custom Java could not be read."))?;
    if copied != size {
        return Err(fail(
            "custom_java_changed",
            "The custom Java changed while being checked.",
        ));
    }
    Ok((file, hasher.finalize().into()))
}

/// What the executable reported about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaFacts {
    pub major_version: u32,
    pub version: String,
    pub vendor: String,
    pub architecture: String,
}

/// Why a custom executable is not usable. Messages are user-facing and
/// never quote raw process output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalJavaError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for ExternalJavaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ExternalJavaError {}

fn fail(code: &'static str, message: impl Into<String>) -> ExternalJavaError {
    ExternalJavaError {
        code,
        message: message.into(),
    }
}

/// The serializable result shown in Settings and readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalJavaReport {
    pub path: String,
    pub required_major_version: u32,
    pub valid: bool,
    pub facts: Option<JavaFacts>,
    pub problem: Option<String>,
    pub code: Option<String>,
}

impl ExternalJavaReport {
    pub fn from_result(
        path: &str,
        required: u32,
        result: &Result<ValidatedExternalJava, ExternalJavaError>,
    ) -> Self {
        match result {
            Ok(java) => Self {
                path: path.to_owned(),
                required_major_version: required,
                valid: true,
                facts: Some(java.facts.clone()),
                problem: None,
                code: None,
            },
            Err(error) => Self {
                path: path.to_owned(),
                required_major_version: required,
                valid: false,
                facts: None,
                problem: Some(error.message.clone()),
                code: Some(error.code.to_owned()),
            },
        }
    }
}

fn redirected(meta: &std::fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Steps 1–3: everything that can be proven without executing the file.
pub fn check_file(path: &str) -> Result<PathBuf, ExternalJavaError> {
    if !matches!(std::env::consts::OS, "windows" | "linux" | "macos")
        || expected_machine().is_none()
    {
        return Err(fail(
            "custom_java_unsupported_platform",
            "Custom Java is unavailable on this operating system or architecture.",
        ));
    }
    JavaSelection::Custom {
        path: path.to_owned(),
    }
    .validate()
    .map_err(|error| fail("custom_java_path_invalid", error.to_string()))?;
    let path = PathBuf::from(path);
    for ancestor in path.ancestors().skip(1) {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match std::fs::symlink_metadata(ancestor) {
            Ok(meta) if redirected(&meta) => {
                return Err(fail(
                    "custom_java_redirected",
                    "The custom Java path goes through a link or junction. Choose the real file inside the Java installation.",
                ));
            }
            Ok(_) => {}
            Err(_) => {
                return Err(fail(
                    "custom_java_missing",
                    "The custom Java executable no longer exists.",
                ));
            }
        }
    }
    let meta = std::fs::symlink_metadata(&path).map_err(|_| {
        fail(
            "custom_java_missing",
            "The custom Java executable no longer exists.",
        )
    })?;
    if redirected(&meta) {
        return Err(fail(
            "custom_java_redirected",
            "The custom Java executable is a link. Choose the real file inside the Java installation.",
        ));
    }
    if !meta.is_file() {
        return Err(fail(
            "custom_java_missing",
            "The custom Java path is not a file.",
        ));
    }
    check_native_executable(&path)?;
    Ok(path)
}

/// The PE machine type the running launcher can execute, if known.
fn expected_machine() -> Option<u16> {
    match std::env::consts::ARCH {
        "x86_64" => Some(0x8664),
        "aarch64" => Some(0xAA64),
        _ => None,
    }
}

fn machine_name(machine: u16) -> &'static str {
    match machine {
        0x8664 => "x64",
        0xAA64 => "ARM64",
        0x014c => "32-bit x86",
        0x01c4 => "ARM (32-bit)",
        _ => "another architecture",
    }
}

#[cfg(windows)]
fn check_native_executable(path: &Path) -> Result<(), ExternalJavaError> {
    use std::io::{Read, Seek, SeekFrom};
    let not_executable = || {
        fail(
            "custom_java_not_executable",
            "The selected file is not a Windows program. Choose java.exe or javaw.exe from a Java installation.",
        )
    };
    let mut file = std::fs::File::open(path).map_err(|_| not_executable())?;
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos).map_err(|_| not_executable())?;
    if &dos[..2] != b"MZ" {
        return Err(not_executable());
    }
    let offset = u32::from_le_bytes([dos[60], dos[61], dos[62], dos[63]]) as u64;
    if offset > 64 * 1024 {
        return Err(not_executable());
    }
    let mut header = [0u8; 6];
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| not_executable())?;
    file.read_exact(&mut header).map_err(|_| not_executable())?;
    if &header[..4] != b"PE\0\0" {
        return Err(not_executable());
    }
    let machine = u16::from_le_bytes([header[4], header[5]]);
    match expected_machine() {
        Some(expected) if machine != expected => Err(fail(
            "custom_java_wrong_architecture",
            format!(
                "This Java is built for {}, but Aurora runs on {}. Choose a {} Java.",
                machine_name(machine),
                machine_name(expected),
                machine_name(expected)
            ),
        )),
        _ => Ok(()),
    }
}

#[cfg(not(windows))]
fn check_native_executable(path: &Path) -> Result<(), ExternalJavaError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)
        .map_err(|_| {
            fail(
                "custom_java_missing",
                "The custom Java executable no longer exists.",
            )
        })?
        .permissions()
        .mode();
    if mode & 0o111 == 0 {
        return Err(fail(
            "custom_java_not_executable",
            "The selected file is not executable.",
        ));
    }
    Ok(())
}

/// Parses `-XshowSettings:properties` output (`key = value` lines).
pub fn parse_properties(output: &str) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    for line in output.lines() {
        if let Some((key, value)) = line.trim().split_once(" = ") {
            if !key.is_empty() && !key.contains(' ') {
                properties
                    .entry(key.to_owned())
                    .or_insert_with(|| value.trim().to_owned());
            }
        }
    }
    properties
}

/// Whether the JVM's `os.arch` is this machine's architecture.
fn architecture_matches(os_arch: &str) -> bool {
    match std::env::consts::ARCH {
        "x86_64" => matches!(os_arch, "amd64" | "x86_64"),
        "aarch64" => matches!(os_arch, "aarch64" | "arm64"),
        _ => false,
    }
}

/// Step 5 over already-captured output.
pub fn evaluate(output: &str, required_major: u32) -> Result<JavaFacts, ExternalJavaError> {
    if output.len() > PROBE_OUTPUT_LIMIT * 2
        || output
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(fail(
            "custom_java_probe_failed",
            "The selected Java reported unexpected diagnostic output.",
        ));
    }
    for key in [
        "java.specification.version",
        "java.version",
        "java.vendor",
        "java.vm.vendor",
        "os.arch",
    ] {
        let values: Vec<_> = output
            .lines()
            .filter_map(|line| line.trim().split_once(" = "))
            .filter(|(k, _)| *k == key)
            .map(|(_, v)| v.trim())
            .collect();
        if values.len() > 1 || values.iter().any(|v| v.is_empty() || v.len() > 256) {
            return Err(fail(
                "custom_java_probe_failed",
                "The selected Java reported ambiguous or malformed properties.",
            ));
        }
    }
    let properties = parse_properties(output);
    let specification = properties.get("java.specification.version").ok_or_else(|| {
        fail(
            "custom_java_probe_failed",
            "The selected program did not report Java properties. Choose java.exe or javaw.exe from a Java installation.",
        )
    })?;
    let major = crate::runtime::plan::parse_java_major(specification).ok_or_else(|| {
        fail(
            "custom_java_probe_failed",
            "The selected Java reported an unreadable version.",
        )
    })?;
    if specification != &major.to_string() && specification != &format!("1.{major}") {
        return Err(fail(
            "custom_java_probe_failed",
            "The selected Java reported an unreadable specification version.",
        ));
    }
    let architecture = properties.get("os.arch").cloned().unwrap_or_default();
    if !architecture_matches(&architecture) {
        return Err(fail(
            "custom_java_wrong_architecture",
            format!(
                "This Java runs as {}, but Aurora needs {}.",
                if architecture.is_empty() {
                    "an unknown architecture"
                } else {
                    &architecture
                },
                std::env::consts::ARCH
            ),
        ));
    }
    if major != required_major {
        return Err(fail(
            "custom_java_wrong_version",
            format!(
                "This Java is version {major}; this Minecraft version requires Java {required_major}. Choose a Java {required_major} installation or use managed Java."
            ),
        ));
    }
    Ok(JavaFacts {
        major_version: major,
        version: properties.get("java.version").cloned().unwrap_or_default(),
        vendor: properties
            .get("java.vendor")
            .or_else(|| properties.get("java.vm.vendor"))
            .cloned()
            .unwrap_or_default(),
        architecture,
    })
}

async fn probe(path: &Path, timeout: Duration) -> Result<String, ExternalJavaError> {
    use crate::install::processors::{
        CancelSignal, SupervisedExit, SupervisorLimits, run_supervised,
    };
    let mut command = tokio::process::Command::new(path);
    #[cfg(windows)]
    let error_file = "-XX:ErrorFile=NUL";
    #[cfg(not(windows))]
    let error_file = "-XX:ErrorFile=/dev/null";
    command
        .args([
            error_file,
            "-XX:-CreateCoredumpOnCrash",
            "-XshowSettings:properties",
            "-version",
        ])
        .env_clear()
        .current_dir(
            path.parent().ok_or_else(|| {
                fail("custom_java_path_invalid", "The Java directory is invalid.")
            })?,
        );
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    #[cfg(windows)]
    for key in ["SYSTEMROOT", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(unix)]
    command.env("LANG", "C").env("LC_ALL", "C");
    let limits = SupervisorLimits {
        timeout,
        shutdown_grace: Duration::from_secs(1),
        retained_output_bytes: PROBE_OUTPUT_LIMIT,
        ..SupervisorLimits::default()
    };
    let run = run_supervised(command, &limits, &CancelSignal::default()).await;
    match run.exit {
        SupervisedExit::Exited(status) if status.success() => {
            if run.stderr.truncated || run.stdout.truncated {
                return Err(fail(
                    "custom_java_probe_failed",
                    "The selected Java produced too much diagnostic output.",
                ));
            }
            let mut text = String::from_utf8(run.stderr.retained_tail).map_err(|_| {
                fail(
                    "custom_java_probe_failed",
                    "The Java diagnostic output is unreadable.",
                )
            })?;
            text.push('\n');
            text.push_str(&String::from_utf8(run.stdout.retained_tail).map_err(|_| {
                fail(
                    "custom_java_probe_failed",
                    "The Java diagnostic output is unreadable.",
                )
            })?);
            Ok(text)
        }
        SupervisedExit::Exited(_) => Err(fail(
            "custom_java_probe_failed",
            "The selected Java started but reported an error. It may be damaged or incomplete.",
        )),
        SupervisedExit::SpawnFailed(_) => Err(fail(
            "custom_java_not_executable",
            "The selected Java could not be started.",
        )),
        SupervisedExit::TimedOut => Err(fail(
            "custom_java_probe_failed",
            "The selected Java did not answer within 15 seconds.",
        )),
        SupervisedExit::Cancelled => Err(fail(
            "custom_java_probe_failed",
            "The Java check was cancelled.",
        )),
    }
}

/// Full validation, always executing the file. Used by Settings checks and
/// immediately before every launch.
pub async fn validate(
    path: &str,
    required_major: u32,
) -> Result<ValidatedExternalJava, ExternalJavaError> {
    validate_with_timeout(path, required_major, PROBE_TIMEOUT).await
}

async fn validate_with_timeout(
    path: &str,
    required_major: u32,
    timeout: Duration,
) -> Result<ValidatedExternalJava, ExternalJavaError> {
    let checked = check_file(path)?;
    let (_guard, identity) = executable_identity(&checked)?;
    let output = probe(&checked, timeout).await?;
    let facts = evaluate(&output, required_major)?;
    if executable_identity(&checked)?.1 != identity {
        return Err(fail(
            "custom_java_changed",
            "The custom Java executable changed during the check.",
        ));
    }
    Ok(ValidatedExternalJava {
        path: checked,
        facts,
        identity,
    })
}

type CacheKey = (PathBuf, [u8; 32], u32);

fn cache() -> &'static Mutex<HashMap<CacheKey, Result<ValidatedExternalJava, ExternalJavaError>>> {
    static CACHE: OnceLock<
        Mutex<HashMap<CacheKey, Result<ValidatedExternalJava, ExternalJavaError>>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Readiness-only variant: reuses a successful result while executable bytes
/// are unchanged, so Home does not start Java on every
/// refresh. Launch always calls [`validate`].
pub async fn validate_for_readiness(
    path: &str,
    required_major: u32,
) -> Result<ValidatedExternalJava, ExternalJavaError> {
    let key = (
        PathBuf::from(path),
        executable_identity(Path::new(path))?.1,
        required_major,
    );
    if let Some(hit) = cache().lock().expect("custom Java cache").get(&key) {
        return hit.clone();
    }
    let result = validate(path, required_major).await;
    if result.as_ref().is_ok_and(|java| java.identity == key.1) {
        let mut cache = cache().lock().expect("custom Java cache");
        if cache.len() > 64 {
            cache.clear();
        }
        cache.insert(key, result.clone());
    }
    result
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const JAVA21: &str = "Property settings:\n    file.encoding = UTF-8\n    java.specification.version = 21\n    java.vendor = Eclipse Adoptium\n    java.version = 21.0.4\n    os.arch = amd64\n    sun.boot.library.path = C:\\jdk\\bin\n\nopenjdk version \"21.0.4\" 2024-07-16 LTS\n";

    #[test]
    fn reported_properties_decide_version_and_architecture() {
        if std::env::consts::ARCH != "x86_64" {
            return;
        }
        let facts = evaluate(JAVA21, 21).unwrap();
        assert_eq!(facts.major_version, 21);
        assert_eq!(facts.vendor, "Eclipse Adoptium");
        assert_eq!(
            evaluate(JAVA21, 17).unwrap_err().code,
            "custom_java_wrong_version"
        );
        let java8 = "    java.specification.version = 1.8\n    os.arch = amd64\n";
        assert_eq!(
            evaluate(java8, 21).unwrap_err().code,
            "custom_java_wrong_version"
        );
        let x86 = JAVA21.replace("os.arch = amd64", "os.arch = x86");
        assert_eq!(
            evaluate(&x86, 21).unwrap_err().code,
            "custom_java_wrong_architecture"
        );
        assert_eq!(
            evaluate("Error: no properties", 21).unwrap_err().code,
            "custom_java_probe_failed"
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aurora-custom-java-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A real native executable, compiled once, with controlled sibling-file
    /// behavior. It never contacts a provider or needs an account.
    pub(crate) fn fixture(mode: &str) -> PathBuf {
        static BINARY: OnceLock<PathBuf> = OnceLock::new();
        let binary = BINARY.get_or_init(|| {
            let dir = temp_dir("fixture-build");
            let source = dir.join("fixture.rs");
            std::fs::write(&source, r#"
use std::{fs, io::Write, time::Duration};
fn main() {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap();
    let mode = fs::read_to_string(dir.join("mode.txt")).unwrap();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-version") {
        let mut count = fs::OpenOptions::new().create(true).append(true).open(dir.join("probes.txt")).unwrap();
        writeln!(count, "probe").unwrap();
        match mode.as_str() {
            "timeout" => std::thread::sleep(Duration::from_secs(30)),
            "nonzero" => std::process::exit(7),
            "malformed" => { eprintln!("not Java"); return; },
            "flood" => { for _ in 0..20000 { eprintln!("bounded diagnostic fixture output"); } },
            _ => ()
        }
        let major = if mode == "wrong" { "17" } else { "21" };
        let arch = if mode == "arch" { "x86" } else { std::env::consts::ARCH };
        eprintln!("java.specification.version = {major}\njava.version = {major}.0.1\njava.vendor = Aurora test fixture\nos.arch = {arch}");
    } else {
        fs::write(dir.join("launch-args.txt"), args.join("\n")).unwrap();
        fs::write(dir.join("launch-cwd.txt"), std::env::current_dir().unwrap().to_str().unwrap()).unwrap();
    }
}
"#).unwrap();
            let output = dir.join(if cfg!(windows) { "java.exe" } else { "java" });
            let result = std::process::Command::new("rustc").arg(&source).args(["--edition=2024", "-o"]).arg(&output).output().unwrap();
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            output
        });
        let dir = temp_dir("with spaces 日本語");
        let output = dir.join(binary.file_name().unwrap());
        std::fs::copy(binary, &output).unwrap();
        std::fs::write(dir.join("mode.txt"), mode).unwrap();
        output
    }

    #[tokio::test]
    async fn bounded_native_diagnostics_cover_success_failure_timeout_and_output() {
        for (mode, expected) in [
            ("valid", None),
            ("wrong", Some("custom_java_wrong_version")),
            ("arch", Some("custom_java_wrong_architecture")),
            ("nonzero", Some("custom_java_probe_failed")),
            ("malformed", Some("custom_java_probe_failed")),
            ("flood", Some("custom_java_probe_failed")),
            ("timeout", Some("custom_java_probe_failed")),
        ] {
            let path = fixture(mode);
            let timeout = if mode == "timeout" {
                Duration::from_millis(150)
            } else {
                Duration::from_secs(5)
            };
            let result = validate_with_timeout(path.to_str().unwrap(), 21, timeout).await;
            assert_eq!(
                result.as_ref().err().map(|e| e.code),
                expected,
                "{mode}: {result:?}"
            );
            if mode == "valid" {
                assert!(result.unwrap().lock_for_launch().is_ok());
            }
        }
    }

    #[tokio::test]
    async fn readiness_reuses_probe_but_rejects_removed_replaced_or_incompatible_java() {
        let path = fixture("valid");
        let text = path.to_str().unwrap();
        let checked = validate_for_readiness(text, 21).await.unwrap();
        validate_for_readiness(text, 21).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(path.with_file_name("probes.txt"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert_eq!(
            validate_for_readiness(text, 17).await.unwrap_err().code,
            "custom_java_wrong_version"
        );
        // Same-length change with the original timestamp: size/mtime cannot
        // authorize a cached success. The PE still parses, the digest differs.
        let mut bytes = std::fs::read(&path).unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(
            checked.lock_for_launch().unwrap_err().code,
            "custom_java_changed"
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            validate_for_readiness(text, 21).await.unwrap_err().code,
            "custom_java_missing"
        );
        assert_eq!(
            checked.lock_for_launch().unwrap_err().code,
            "custom_java_missing"
        );
    }

    #[test]
    fn malformed_ambiguous_or_control_bearing_properties_are_rejected() {
        for output in [
            format!("{JAVA21}\njava.specification.version = 17"),
            JAVA21.replace("Eclipse Adoptium", "bad\u{1b}vendor"),
            JAVA21.replace("21\n", "garbage\n"),
        ] {
            assert!(evaluate(&output, 21).is_err());
        }
    }

    #[test]
    fn shape_and_file_checks_refuse_before_execution() {
        let dir = temp_dir("shape");
        assert_eq!(
            check_file("relative\\java.exe").unwrap_err().code,
            "custom_java_path_invalid"
        );
        let missing = dir.join(if cfg!(windows) { "java.exe" } else { "java" });
        assert_eq!(
            check_file(missing.to_str().unwrap()).unwrap_err().code,
            "custom_java_missing"
        );
        let wrong_name = dir.join("notepad.exe");
        std::fs::write(&wrong_name, b"MZ").unwrap();
        assert_eq!(
            check_file(wrong_name.to_str().unwrap()).unwrap_err().code,
            "custom_java_path_invalid"
        );
        #[cfg(windows)]
        {
            std::fs::write(&missing, b"plain text, not a program").unwrap();
            assert_eq!(
                check_file(missing.to_str().unwrap()).unwrap_err().code,
                "custom_java_not_executable"
            );
            // A minimal 32-bit PE header: refused for architecture, never run.
            let mut pe = vec![0u8; 128];
            pe[..2].copy_from_slice(b"MZ");
            pe[60..64].copy_from_slice(&64u32.to_le_bytes());
            pe[64..68].copy_from_slice(b"PE\0\0");
            pe[68..70].copy_from_slice(&0x014cu16.to_le_bytes());
            std::fs::write(&missing, &pe).unwrap();
            if std::env::consts::ARCH == "x86_64" {
                let error = check_file(missing.to_str().unwrap()).unwrap_err();
                assert_eq!(error.code, "custom_java_wrong_architecture");
                assert!(error.message.contains("32-bit x86"));
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_on_the_path_is_refused() {
        let dir = temp_dir("junction");
        let real = dir.join("real");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(real.join("java.exe"), b"MZ").unwrap();
        let link = dir.join("link");
        let created = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if created {
            let through = link.join("java.exe");
            assert_eq!(
                check_file(through.to_str().unwrap()).unwrap_err().code,
                "custom_java_redirected"
            );
            std::fs::remove_dir(&link).unwrap();
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
