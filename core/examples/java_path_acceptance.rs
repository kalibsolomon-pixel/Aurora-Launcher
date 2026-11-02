//! Explicit live acceptance only: official Java 21 in new, isolated roots.
use aurora_launcher_lib::{downloads::DownloadOptions, minecraft, paths::ManagedPaths, runtime};
use std::path::PathBuf;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("new output root required")?,
    );
    if !root.is_absolute() || root.exists() {
        return Err("requires a new absolute disposable directory".into());
    }
    std::fs::create_dir_all(&root)?;
    let managed = ManagedPaths::from_app_local_data_dir(root.join("short"))?;
    let options = DownloadOptions::default();
    let game = minecraft::resolve_install_plan(
        &minecraft::metadata::MetadataEndpoints::official(),
        &minecraft::metadata::MinecraftVersionId::new("1.21.11")?,
        minecraft::rules::PlatformProfile::current()?,
        &options,
    )
    .await?;
    let plan = runtime::resolve_runtime_plan(
        &managed,
        &runtime::RuntimeMetadataEndpoints::official(),
        game.java().component(),
        game.java().major_version(),
        runtime::RuntimePlatform::current()?,
        &options,
    )
    .await?;
    runtime::ensure_runtime(&managed, &plan, &options, false, &mut |_| {}).await?;
    let long_base = ManagedPaths::from_app_local_data_dir(root.join("long"))?;
    let base_length = plan
        .diagnostic_executable_path(&long_base)
        .to_string_lossy()
        .encode_utf16()
        .count();
    let long_root = long_base.data_root().join(
        "a".repeat(
            250usize
                .checked_sub(base_length + 1)
                .ok_or("acceptance root is too long")?,
        ),
    );
    let near_long_root = long_base.data_root().join(
        "a".repeat(
            249usize
                .checked_sub(base_length + 1)
                .ok_or("acceptance root is too long")?,
        ),
    );
    let mut rows = Vec::new();
    for (name, target) in [
        ("short", managed.clone()),
        (
            "spaces",
            ManagedPaths::from_app_local_data_dir(root.join("path with spaces"))?,
        ),
        (
            "unicode",
            ManagedPaths::from_app_local_data_dir(root.join("Java-é-日本語"))?,
        ),
        ("long", ManagedPaths::from_app_local_data_dir(long_root)?),
        (
            "near-long",
            ManagedPaths::from_app_local_data_dir(near_long_root)?,
        ),
    ] {
        if name != "short" {
            copy_tree(
                &plan.installation_dir(&managed),
                &plan.installation_dir(&target),
            )?;
        }
        let bytes = runtime::validate_runtime(
            &target,
            &plan,
            false,
            runtime::install::DEFAULT_JAVA_DIAGNOSTIC_TIMEOUT,
        )
        .await?;
        let reports_before = crash_reports()?;
        let explicit = runtime::validate_runtime(
            &target,
            &plan,
            true,
            runtime::install::DEFAULT_JAVA_DIAGNOSTIC_TIMEOUT,
        )
        .await?;
        let eligibility = runtime::install::validate_runtime_eligibility(&target, &plan).await?;
        assert_eq!(explicit.status, eligibility.status);
        assert_eq!(
            crash_reports()?,
            reports_before,
            "diagnostics must not leave crash files in the launcher's working directory"
        );
        let inherited = tokio::process::Command::new(plan.diagnostic_executable_path(&target))
            .arg("-version")
            .current_dir(&root)
            .output()
            .await;
        let row = serde_json::json!({"scenario":name,"root":target.data_root(),
            "component":plan.component(),"runtimeVersion":plan.runtime_version(),"requiredMajor":plan.required_major_version(),
            "executable":plan.diagnostic_executable_path(&target),
            "executableUtf16Length":plan.diagnostic_executable_path(&target).to_string_lossy().encode_utf16().count(),
            "integrityOnly":bytes.status.as_str(),"explicit":explicit.status.as_str(),"launchEligibility":eligibility.status.as_str(),"problems":explicit.problems,
            "diagnostic":explicit.diagnostic.map(|d|d.summary),"diagnosticCrashFilesCreated":0,"inheritedExit":inherited.as_ref().ok().and_then(|o|o.status.code()),
            "inheritedOutput":inherited.as_ref().map(|o|String::from_utf8_lossy(&o.stderr).into_owned()).unwrap_or_else(|e|e.to_string())});
        println!("{row}");
        rows.push(row);
    }
    let executable = plan.diagnostic_executable_path(&managed);
    let original = std::fs::read(&executable)?;
    let permissions = std::fs::metadata(&executable)?.permissions();
    std::fs::remove_file(&executable)?;
    assert_eq!(
        runtime::install::validate_runtime_eligibility(&managed, &plan)
            .await?
            .status,
        runtime::RuntimeValidationStatus::Damaged
    );
    std::fs::write(&executable, &original)?;
    std::fs::set_permissions(&executable, permissions)?;
    let state_path = plan
        .installation_dir(&managed)
        .join("runtime-installed.json");
    let state = std::fs::read(&state_path)?;
    std::fs::write(&state_path, b"{broken")?;
    assert_eq!(
        runtime::install::validate_runtime_eligibility(&managed, &plan)
            .await?
            .status,
        runtime::RuntimeValidationStatus::Damaged
    );
    std::fs::write(&state_path, &state)?;
    assert_eq!(
        runtime::install::validate_runtime_eligibility(&managed, &plan)
            .await?
            .status,
        runtime::RuntimeValidationStatus::Ready
    );
    rows.push(serde_json::json!({"scenario":"missing executable, malformed metadata, then restored original bytes", "recovered":"ready"}));
    std::fs::write(root.join("results.json"), serde_json::to_vec_pretty(&rows)?)?;
    Ok(())
}

fn crash_reports() -> std::io::Result<std::collections::BTreeSet<std::ffi::OsString>> {
    std::fs::read_dir(std::env::current_dir()?)?
        .filter_map(|entry| match entry {
            Ok(entry)
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("hs_err_pid")
                    || entry.file_name().to_string_lossy().ends_with(".mdmp") =>
            {
                Some(Ok(entry.file_name()))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn copy_tree(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}
