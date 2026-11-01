#![cfg_attr(windows, windows_subsystem = "windows")]
// Mirrors the native launcher's update handoff: spawn the installer with this
// exact PID, then exit at once, as the UI does after a successful spawn. Started
// without any surviving handle, its PID is gone before NSIS can open it.
fn main() {
    let installer = std::env::args().nth(1).expect("installer path");
    std::process::Command::new(installer)
        .args(["/S", &format!("/UPDATEPID={}", std::process::id())])
        .spawn()
        .expect("installer spawn");
}
