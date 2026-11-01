#![cfg_attr(windows, windows_subsystem = "windows")]
// Exact test-owned process for NSIS's update handoff wait. No UI or file writes.
fn main() { std::thread::sleep(std::time::Duration::from_secs(4)); }
