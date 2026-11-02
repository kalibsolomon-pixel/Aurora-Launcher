fn main() {
    if std::env::var_os("CARGO_FEATURE_NATIVE_UPDATE_ACCEPTANCE").is_some() {
        for name in ["ROOT", "ID", "ORIGIN", "KEY", "VERSION"] {
            let name = format!("AURORA_NATIVE_ACCEPTANCE_{name}");
            println!("cargo:rerun-if-env-changed={name}");
            assert!(
                !std::env::var(&name).unwrap_or_default().is_empty(),
                "explicit local updater acceptance configuration required"
            );
        }
        let root =
            std::path::PathBuf::from(std::env::var("AURORA_NATIVE_ACCEPTANCE_ROOT").unwrap());
        let temp = std::env::temp_dir();
        assert!(
            root.parent() == Some(temp.as_path())
                && root
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("aurora-native9-")
                && root.join("NATIVE-RELEASE-DISPOSABLE.txt").is_file(),
            "marked disposable updater root required"
        );
        let id = std::env::var("AURORA_NATIVE_ACCEPTANCE_ID").unwrap();
        assert!(
            id.len() == 32
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "isolated acceptance package identity required"
        );
        let origin = std::env::var("AURORA_NATIVE_ACCEPTANCE_ORIGIN").unwrap();
        let port: u16 = origin
            .strip_prefix("http://127.0.0.1:")
            .expect("explicit loopback transport only")
            .parse()
            .expect("loopback port required");
        assert!(port > 0);
        assert!(
            std::env::var("AURORA_NATIVE_ACCEPTANCE_KEY").ok()
                != std::env::var("AURORA_UPDATER_PUBKEY").ok(),
            "acceptance must use a separate test identity"
        );
    }
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
    // The native host builds the launcher core without Tauri.
    #[cfg(feature = "tauri-host")]
    tauri_build::build();
}
