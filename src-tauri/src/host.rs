//! The application host boundary.
//!
//! `application` implements every launcher operation against [`Host`], never
//! against a specific UI toolkit. A host supplies exactly four things: the
//! platform local-data directory (from which `ManagedPaths` is derived and
//! validated), the running application version, an event sink for progress
//! and state notifications, and a task spawner. Two hosts exist:
//!
//! * the Tauri/WebView host (`tauri-host` feature), which forwards events to
//!   the webview with `Emitter::emit` exactly as before the extraction; and
//! * the native host (`native/`, the `aurora-native` crate, which builds this
//!   core with default features off), whose events are delivered to the
//!   native UI thread.
//!
//! The host never widens authority: paths are still validated by
//! `ManagedPaths`, and nothing here accepts frontend-supplied paths or URLs
//! beyond what the calling operation already validated.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use serde::Serialize;

/// A boxed background task.
pub type Task = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// What a concrete host provides. Dispatch is dynamic on purpose: the Tauri
/// implementation is only instantiated by the Tauri command wrappers, so code
/// paths shared with tests and with the native host never link the WebView
/// runtime.
trait Backend: Send + Sync {
    fn local_data_dir(&self) -> Result<PathBuf, String>;
    fn version(&self) -> String;
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String>;
    fn spawn(&self, task: Task);
    #[cfg(feature = "tauri-host")]
    fn tauri(&self) -> Option<&tauri::AppHandle> {
        None
    }
    fn is_native(&self) -> bool {
        false
    }
}

/// A cheap, cloneable handle to the running application host.
#[derive(Clone)]
pub struct Host {
    backend: Arc<dyn Backend>,
}

#[cfg(feature = "tauri-host")]
struct TauriBackend(tauri::AppHandle);

#[cfg(feature = "tauri-host")]
impl Backend for TauriBackend {
    fn local_data_dir(&self) -> Result<PathBuf, String> {
        use tauri::Manager;
        self.0
            .path()
            .app_local_data_dir()
            .map_err(|error| error.to_string())
    }

    fn version(&self) -> String {
        self.0.package_info().version.to_string()
    }

    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String> {
        use tauri::Emitter;
        // The webview receives the same JSON the typed payload serialized to.
        self.0
            .emit(event, payload)
            .map_err(|error| error.to_string())
    }

    fn spawn(&self, task: Task) {
        tauri::async_runtime::spawn(task);
    }

    fn tauri(&self) -> Option<&tauri::AppHandle> {
        Some(&self.0)
    }
}

struct NativeBackend {
    local_data_dir: Result<PathBuf, String>,
    version: String,
    events: Box<dyn Fn(&str, serde_json::Value) + Send + Sync>,
    runtime: tokio::runtime::Handle,
}

impl Backend for NativeBackend {
    fn local_data_dir(&self) -> Result<PathBuf, String> {
        self.local_data_dir.clone()
    }

    fn version(&self) -> String {
        self.version.clone()
    }

    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), String> {
        (self.events)(event, payload);
        Ok(())
    }

    fn spawn(&self, task: Task) {
        self.runtime.spawn(task);
    }

    fn is_native(&self) -> bool {
        true
    }
}

impl Host {
    /// Wraps the Tauri application handle. Events reach the webview with the
    /// same names and JSON payloads as before the extraction.
    #[cfg(feature = "tauri-host")]
    pub fn from_tauri(app: &tauri::AppHandle) -> Self {
        Self {
            backend: Arc::new(TauriBackend(app.clone())),
        }
    }

    /// Creates a native host. `local_data_dir` must be the platform
    /// application-local data directory for the stable application
    /// identifier (see [`platform_local_data_dir`]).
    pub fn native(
        local_data_dir: Result<PathBuf, String>,
        version: impl Into<String>,
        runtime: tokio::runtime::Handle,
        events: impl Fn(&str, serde_json::Value) + Send + Sync + 'static,
    ) -> Self {
        Self {
            backend: Arc::new(NativeBackend {
                local_data_dir,
                version: version.into(),
                events: Box::new(events),
                runtime,
            }),
        }
    }

    /// The platform application-local data directory for this application.
    pub fn local_data_dir(&self) -> Result<PathBuf, String> {
        self.backend.local_data_dir()
    }

    /// The running application's version.
    pub fn version(&self) -> String {
        self.backend.version()
    }

    /// Emits one host event. Failures are reported but never change the
    /// outcome of the operation that produced the event.
    pub fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<(), String> {
        let value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        self.backend.emit(event, value)
    }

    /// Spawns a long-lived background task on the host's async runtime.
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.backend.spawn(Box::pin(future));
    }

    /// The Tauri handle for Tauri-only integrations (the official updater).
    #[cfg(feature = "tauri-host")]
    pub fn tauri(&self) -> Option<&tauri::AppHandle> {
        self.backend.tauri()
    }

    /// Whether this host is the native (non-WebView) host.
    pub fn is_native(&self) -> bool {
        self.backend.is_native()
    }
}

/// The launcher version compiled into this core (kept equal to package.json
/// by `npm run verify:version`).
#[cfg(not(feature = "native-update-acceptance"))]
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(feature = "native-update-acceptance")]
pub const LAUNCHER_VERSION: &str = env!("AURORA_NATIVE_ACCEPTANCE_VERSION");

/// The application identifier that names the platform data directory. It is
/// a compatibility contract shared by both hosts (see AGENTS.md).
#[cfg(not(feature = "native-preview"))]
pub const APPLICATION_IDENTIFIER: &str = "com.aurora.launcher";
#[cfg(all(feature = "native-preview", not(feature = "native-update-acceptance")))]
pub const APPLICATION_IDENTIFIER: &str = "com.aurora.launcher.native-preview";
#[cfg(feature = "native-update-acceptance")]
pub const APPLICATION_IDENTIFIER: &str = concat!(
    "com.aurora.launcher.native-preview.acceptance.",
    env!("AURORA_NATIVE_ACCEPTANCE_ID")
);

/// Resolves `<platform local data>/<identifier>` the same way Tauri's
/// `app_local_data_dir` does: the OS known folder (Windows
/// `FOLDERID_LocalAppData`), or `$XDG_DATA_HOME` / `~/.local/share` on Linux.
/// Environment variables such as `LOCALAPPDATA` are not consulted on Windows.
pub fn platform_local_data_dir(identifier: &str) -> Result<PathBuf, String> {
    platform_local_data_root().map(|root| root.join(identifier))
}

#[cfg(windows)]
fn platform_local_data_root() -> Result<PathBuf, String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    // SAFETY: SHGetKnownFolderPath returns a CoTaskMem-allocated wide string
    // that is copied and then freed exactly once.
    unsafe {
        let raw = SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None).map_err(
            |error| format!("the local application data folder is unavailable: {error}"),
        )?;
        let path = raw
            .to_string()
            .map_err(|_| "the local application data folder is not valid Unicode".to_owned());
        CoTaskMemFree(Some(raw.0 as *const _));
        path.map(PathBuf::from)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_local_data_root() -> Result<PathBuf, String> {
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        if data.is_absolute() {
            return Ok(data);
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or_else(|| "the home directory is unavailable".to_owned())?;
    Ok(home.join(".local").join("share"))
}

#[cfg(target_os = "macos")]
fn platform_local_data_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or_else(|| "the home directory is unavailable".to_owned())?;
    Ok(home.join("Library").join("Application Support"))
}

/// Opens an already-validated local folder in the platform file manager.
pub fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(feature = "tauri-host")]
    {
        tauri_plugin_opener::open_path(path, None::<&str>).map_err(|error| error.to_string())
    }
    #[cfg(not(feature = "tauri-host"))]
    {
        if !path.is_absolute() {
            return Err("only absolute managed paths can be opened".to_owned());
        }
        native_open::open(path.as_os_str())
    }
}

/// Opens an `https://` URL in the user's default system browser (the OAuth
/// public-client model: no embedded browser). Any other scheme is refused.
pub fn open_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| "the address is not a valid URL".to_owned())?;
    if parsed.scheme() != "https" {
        return Err("only https addresses can be opened in the browser".to_owned());
    }
    #[cfg(feature = "tauri-host")]
    {
        tauri_plugin_opener::open_url(parsed.as_str(), None::<&str>)
            .map_err(|error| error.to_string())
    }
    #[cfg(not(feature = "tauri-host"))]
    {
        native_open::open(std::ffi::OsStr::new(parsed.as_str()))
    }
}

#[cfg(not(feature = "tauri-host"))]
mod native_open {
    use std::ffi::OsStr;

    /// `ShellExecuteW("open", target)`: no shell, no command string.
    #[cfg(windows)]
    pub fn open(target: &OsStr) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::PCWSTR;
        let wide: Vec<u16> = target.encode_wide().chain(Some(0)).collect();
        let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        // SAFETY: both buffers are NUL-terminated and outlive the call.
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // Values greater than 32 indicate success.
        if result.0 as isize > 32 {
            Ok(())
        } else {
            Err(format!(
                "the system could not open the item (code {})",
                result.0 as isize
            ))
        }
    }

    /// `xdg-open <target>` with a structured argument vector.
    #[cfg(all(unix, not(target_os = "macos")))]
    pub fn open(target: &OsStr) -> Result<(), String> {
        std::process::Command::new("xdg-open")
            .arg(target)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("the system could not open the item: {error}"))
    }

    #[cfg(target_os = "macos")]
    pub fn open(target: &OsStr) -> Result<(), String> {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("the system could not open the item: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native(
        events: impl Fn(&str, serde_json::Value) + Send + Sync + 'static,
    ) -> (tokio::runtime::Runtime, Host) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let host = Host::native(
            Ok(PathBuf::from("unused")),
            "9.9.9",
            runtime.handle().clone(),
            events,
        );
        (runtime, host)
    }

    #[test]
    fn native_host_delivers_serialized_events() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        let (_runtime, host) =
            native(move |name, payload| sink.lock().unwrap().push((name.to_owned(), payload)));
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        #[derive(Clone)]
        struct Progress {
            completed_items: u32,
        }
        host.emit("install-progress", Progress { completed_items: 3 })
            .unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "install-progress");
        assert_eq!(seen[0].1, serde_json::json!({ "completedItems": 3 }));
        assert!(host.is_native());
        assert_eq!(host.version(), "9.9.9");
    }

    #[test]
    fn native_host_reports_unresolved_data_directory() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let host = Host::native(
            Err("no folder".into()),
            "1",
            runtime.handle().clone(),
            |_, _| {},
        );
        assert_eq!(host.local_data_dir().unwrap_err(), "no folder");
    }

    #[test]
    fn browser_opening_refuses_non_https_addresses() {
        assert!(open_url("http://example.com").is_err());
        assert!(open_url("file:///C:/Windows").is_err());
        assert!(open_url("javascript:alert(1)").is_err());
        assert!(open_url("not a url").is_err());
    }

    #[test]
    fn platform_data_directory_is_absolute_and_identifier_scoped() {
        let dir = platform_local_data_dir(APPLICATION_IDENTIFIER).unwrap();
        assert!(dir.is_absolute());
        assert!(dir.ends_with(APPLICATION_IDENTIFIER));
    }
}
