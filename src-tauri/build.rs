fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PRIVATE_CANDIDATE");
    let config = std::env::var("TAURI_CONFIG").unwrap_or_default();
    let private = std::env::var_os("CARGO_FEATURE_PRIVATE_CANDIDATE").is_some();
    assert!(
        !config.contains("com.aurora.launcher.private-candidate") || private,
        "the private Tauri overlay requires the private-candidate feature"
    );
    if private {
        assert!(
            config.contains("com.aurora.launcher.private-candidate")
                && config.contains("Aurora Launcher Private Test Candidate"),
            "private-candidate requires src-tauri/tauri.private.conf.json; use the private build script"
        );
    }
    tauri_build::build()
}
