//! Offline execution tests: synthetic Java tools exercise the production
//! Forge adapter/supervisor. These are not Minecraft gameplay evidence.
use super::metadata::{
    ForgeMavenEndpoints, ForgeVersionId, InstallProfileDocument, VersionProfileDocument,
};
use super::plan::{ForgeCoordinate, ForgePlan};
use super::processors::{ProcessorContext, ProcessorError, run_client_processors};
use crate::integrity::Sha1Digest;
use crate::minecraft::metadata::MinecraftVersionId;
use serde_json::json;
use sha1::{Digest, Sha1};
use std::path::PathBuf;

struct Fixture {
    root: PathBuf,
    context: ProcessorContext,
    plan: ForgePlan,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-f1c-chain-")
        );
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

fn fixture(modes: &[&str], wrong_hash: bool) -> Option<Fixture> {
    let Some((tool, java)) = crate::neoforge::processors::test_tools::tool_and_java() else {
        eprintln!("F1C synthetic execution requires a JDK; unavailable on this host");
        return None;
    };
    let root = std::env::temp_dir().join(format!("aurora-f1c-chain-{}", uuid::Uuid::new_v4()));
    let staging = root.join("stage/game");
    std::fs::create_dir_all(&staging).unwrap();
    let tool_path = staging.join("libraries/test/tool/1/tool-1.jar");
    std::fs::create_dir_all(tool_path.parent().unwrap()).unwrap();
    std::fs::copy(tool, tool_path).unwrap();
    let client = staging.join("client.jar");
    std::fs::write(&client, b"untouched client input").unwrap();
    let input = staging.join("libraries/test/input/1/input-1.zip");
    std::fs::create_dir_all(input.parent().unwrap()).unwrap();
    std::fs::write(input, b"inline Maven input").unwrap();
    let installer = root.join("verified-installer.jar");
    std::fs::write(&installer, b"private installer integrity fixture").unwrap();
    let installer_hash = Sha1Digest::parse(&format!(
        "{:x}",
        Sha1::digest(std::fs::read(&installer).unwrap())
    ))
    .unwrap();
    let hash = if wrong_hash {
        "0000000000000000000000000000000000000000".into()
    } else {
        format!("{:x}", Sha1::digest(b"patched client artifact bytes"))
    };
    let mut profile: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/1.20.4-49.2.0-install_profile.json")).unwrap();
    let mut data = serde_json::Map::new();
    let mut processors = Vec::new();
    // A server-only failing step must never execute.
    processors.push(json!({"sides":["server"],"jar":"test:tool:1","args":["--mode","fail"]}));
    for (index, mode) in modes.iter().enumerate() {
        let token = format!("OUT{index}");
        data.insert(
            token.clone(),
            json!({"client":format!("[test:result:1:{index}]"),"server":"'unused'"}),
        );
        let input = if index == 0 {
            "[test:input:1@zip]".into()
        } else {
            format!("{{OUT{}}}", index - 1)
        };
        processors.push(
            json!({"sides":["client"],"jar":"test:tool:1","classpath":[],
            "args":["--mode",mode,"--input",input,"--output",format!("{{{token}}}")],
            "outputs":{format!("{{{token}}}"):hash}}),
        );
    }
    profile["data"] = data.into();
    profile["processors"] = processors.into();
    let plan = ForgePlan::from_documents(
        &InstallProfileDocument::from_json(&profile.to_string()).unwrap(),
        &VersionProfileDocument::from_json(include_str!("fixtures/1.20.4-49.2.0-version.json"))
            .unwrap(),
        &MinecraftVersionId::new("1.20.4").unwrap(),
        &ForgeVersionId::new("1.20.4-49.2.0").unwrap(),
        &ForgeMavenEndpoints::official(),
        installer_hash,
    )
    .unwrap();
    let context = ProcessorContext::new(
        staging,
        root.join("stage/scratch"),
        installer,
        installer_hash,
        java,
        client,
        "1.20.4".into(),
    );
    Some(Fixture {
        root,
        context,
        plan,
    })
}

fn output(fixture: &Fixture, index: usize) -> PathBuf {
    fixture.context.staging_game.join("libraries").join(
        ForgeCoordinate::parse(&format!("test:result:1:{index}"))
            .unwrap()
            .repository_path(),
    )
}

#[tokio::test]
async fn five_and_six_step_chains_verify_each_output_and_filter_server_steps() {
    for count in [5, 6] {
        let Some(f) = fixture(&vec!["write"; count], false) else {
            return;
        };
        let mut calls = 0;
        let result = run_client_processors(&f.plan, &f.context, &mut |_| calls += 1)
            .await
            .unwrap();
        assert_eq!(calls, count);
        assert_eq!(result.generated.len(), count);
        assert_eq!(result.published_sha1.len(), count);
        assert!(output(&f, count - 1).is_file());
        assert_eq!(
            std::fs::read(&f.context.client_jar).unwrap(),
            b"untouched client input"
        );
    }
}

#[tokio::test]
async fn intermediate_failures_missing_outputs_and_bad_digests_stop_dependents() {
    for (mode, wrong_hash) in [
        ("fail", false),
        ("noop", false),
        ("write", true),
        ("modify", false),
    ] {
        let Some(f) = fixture(&["write", mode, "write"], wrong_hash) else {
            return;
        };
        let mut calls = 0;
        let error = run_client_processors(&f.plan, &f.context, &mut |_| calls += 1)
            .await
            .unwrap_err();
        match (mode, wrong_hash) {
            (_, true) => assert!(matches!(error, ProcessorError::OutputHashMismatch { .. })),
            ("fail", _) => assert!(matches!(error, ProcessorError::Failed { .. })),
            ("noop", _) => assert!(matches!(error, ProcessorError::OutputMissing { .. })),
            ("modify", _) => assert!(matches!(error, ProcessorError::Modified { .. })),
            _ => unreachable!(),
        }
        assert_eq!(calls, if wrong_hash { 1 } else { 2 });
        assert!(!output(&f, 2).exists());
        assert!(!f.context.staging_game.join("installed-game.json").exists());
    }
}

#[tokio::test]
async fn timeout_cancellation_and_corrupt_installer_never_complete() {
    let Some(mut f) = fixture(&["sleep", "write"], false) else {
        return;
    };
    f.context.timeout = std::time::Duration::from_millis(250);
    assert!(matches!(
        run_client_processors(&f.plan, &f.context, &mut |_| {}).await,
        Err(ProcessorError::Timeout { .. })
    ));
    assert!(!output(&f, 1).exists());
    let Some(mut f) = fixture(&["sleep", "write"], false) else {
        return;
    };
    let (handle, signal) = crate::install::processors::cancellation_channel();
    f.context.cancel = signal;
    let cancel = async {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        handle.cancel();
    };
    let mut progress = |_: &str| {};
    let (result, ()) = tokio::join!(
        run_client_processors(&f.plan, &f.context, &mut progress),
        cancel
    );
    assert!(matches!(result, Err(ProcessorError::Cancelled { .. })));
    assert!(!output(&f, 1).exists());
    let Some(f) = fixture(&["write"], false) else {
        return;
    };
    std::fs::write(&f.context.installer_path, b"corrupt installer").unwrap();
    let mut calls = 0;
    assert!(matches!(
        run_client_processors(&f.plan, &f.context, &mut |_| calls += 1).await,
        Err(ProcessorError::InstallerIntegrity { .. })
    ));
    assert_eq!(calls, 0);
}

#[cfg(windows)]
#[tokio::test]
async fn output_through_a_junction_is_rejected_before_spawn() {
    let Some(f) = fixture(&["write"], false) else {
        return;
    };
    let outside = f.root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    let result_parent = f.context.staging_game.join("libraries/test/result");
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(result_parent.to_string_lossy().replace('/', "\\"))
        .arg(outside.to_string_lossy().replace('/', "\\"))
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "junction test prerequisite failed: {} {}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
    let mut calls = 0;
    assert!(
        run_client_processors(&f.plan, &f.context, &mut |_| calls += 1)
            .await
            .is_err()
    );
    assert_eq!(calls, 0);
    assert_eq!(std::fs::read_dir(outside).unwrap().count(), 0);
    // Remove the exact junction only so Fixture cleanup never traverses it.
    std::fs::remove_dir(result_parent).unwrap();
}
