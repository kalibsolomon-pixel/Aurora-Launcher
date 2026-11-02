//! Opt-in desktop observation only. No command, IPC, or production crash trigger.
//! The external driver terminates its retained process handle while this hook parks.
use std::path::{Path, PathBuf};

const MARKER: &[u8] = b"F3RA isolated desktop acceptance\n";

fn authorized_root(managed: &Path, temp: &Path) -> std::io::Result<PathBuf> {
    let root = managed
        .parent()
        .ok_or_else(|| std::io::Error::other("missing acceptance parent"))?;
    if managed.file_name().and_then(|name| name.to_str()) != Some("managed")
        || !root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("aurora-f3ra-"))
        || root.parent().map(std::fs::canonicalize).transpose()?
            != Some(std::fs::canonicalize(temp)?)
        || std::fs::read(root.join("F3RA-ISOLATED.txt"))? != MARKER
    {
        return Err(std::io::Error::other(
            "unmarked or non-disposable acceptance root",
        ));
    }
    // No redirected managed descendants or marker roots.
    for path in [root, managed] {
        if super::install::coordination::redirected(&std::fs::symlink_metadata(path)?) {
            return Err(std::io::Error::other("redirected acceptance root"));
        }
    }
    Ok(root.to_path_buf())
}

pub(crate) fn authorize() -> std::io::Result<PathBuf> {
    let managed = std::env::var_os("AURORA_DIAGNOSTIC_DATA_ROOT")
        .ok_or_else(|| std::io::Error::other("missing isolated managed root"))?;
    authorized_root(Path::new(&managed), &std::env::temp_dir())
}

pub(crate) fn checkpoint(
    instance_root: &Path,
    point: crate::install::journal::TransactionPoint,
) -> Result<(), crate::install::InstallError> {
    retention_checkpoint(instance_root, &format!("{point:?}"))
}

pub(crate) fn retention_checkpoint(
    instance_root: &Path,
    label: &str,
) -> Result<(), crate::install::InstallError> {
    let Some(requested) = std::env::var_os("AURORA_F3RA_PAUSE") else {
        return Ok(());
    };
    if requested != label {
        return Ok(());
    }
    let root = authorize().map_err(crate::install::InstallError::Storage)?;
    let identity = std::env::var("AURORA_F3RA_INSTANCE").unwrap_or_default();
    let id = crate::instances::InstanceId::new(identity).map_err(|_| {
        crate::install::InstallError::Storage(std::io::Error::other("invalid acceptance target"))
    })?;
    if instance_root != root.join("managed/instances").join(id.as_str()) {
        return Ok(());
    }
    let pid = std::process::id();
    let evidence = root.join("evidence");
    std::fs::create_dir_all(&evidence).map_err(crate::install::InstallError::Storage)?;
    let bytes =
        serde_json::to_vec(&serde_json::json!({"pid":pid,"point":label,"instanceId":id.as_str()}))
            .expect("closed observation shape");
    std::fs::write(
        evidence.join(format!("checkpoint-{pid}-{label}.json")),
        bytes,
    )
    .map_err(crate::install::InstallError::Storage)?;
    eprintln!("[aurora-desktop-acceptance] pid={pid} target={id} point={label}");
    let started = std::time::Instant::now();
    while !evidence.join(format!("resume-{pid}")).exists() {
        if started.elapsed() > std::time::Duration::from_secs(300) {
            return Err(crate::install::InstallError::RecoveryRequired {
                reason: "bounded desktop acceptance pause expired".into(),
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_marked_direct_temporary_children_authorize() {
        let root = std::env::temp_dir().join(format!("aurora-f3ra-{}", uuid::Uuid::new_v4()));
        let managed = root.join("managed");
        std::fs::create_dir_all(&managed).unwrap();
        assert!(authorized_root(&managed, &std::env::temp_dir()).is_err());
        std::fs::write(root.join("F3RA-ISOLATED.txt"), MARKER).unwrap();
        assert_eq!(
            authorized_root(&managed, &std::env::temp_dir()).unwrap(),
            root
        );
        assert!(authorized_root(&managed, &root).is_err());
        assert!(authorized_root(&root.join("other"), &std::env::temp_dir()).is_err());
        std::fs::write(root.join("F3RA-ISOLATED.txt"), b"wrong marker").unwrap();
        assert!(authorized_root(&managed, &std::env::temp_dir()).is_err());
        // Exact disposable path created by this test; no external data.
        std::fs::remove_dir_all(root).unwrap();
    }
}
