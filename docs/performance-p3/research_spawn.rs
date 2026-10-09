//! Included only by prepare.py in a disposable checkout with performance-p3.
use std::{io, path::PathBuf};

fn trial() -> io::Result<PathBuf> {
    let root = std::env::var_os("AURORA_P3_ROOT").ok_or(io::ErrorKind::InvalidInput)?;
    let root = PathBuf::from(root).canonicalize()?;
    let path = std::env::var_os("AURORA_P3_TRIAL_ROOT").ok_or(io::ErrorKind::InvalidInput)?;
    let supplied_path = PathBuf::from(path);
    let path = supplied_path.canonicalize()?;
    if !root.starts_with(std::env::temp_dir().canonicalize()?)
        || !root.file_name().unwrap_or_default().to_string_lossy().starts_with("aurora-p0-2-p3-")
        || path.parent() != Some(root.join("trials").as_path())
        || !root.join("jna-isolation.json").is_file()
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // Java's Windows classpath parser does not accept verbatim-path prefixes.
    // Canonicalization proves containment; retain the ordinary absolute spelling.
    Ok(supplied_path)
}

pub(super) fn arguments() -> io::Result<Vec<String>> {
    #[cfg(test)]
    if std::env::var_os("AURORA_P3_ROOT").is_none() {
        return Ok(Vec::new());
    }
    let trial = trial()?;
    for name in ["jna", "tmp", "lwjgl"] {
        std::fs::create_dir(trial.join(name))?;
    }
    let root = PathBuf::from(std::env::var_os("AURORA_P3_ROOT").unwrap());
    let mut args = vec![
        format!("-Djna.tmpdir={}", trial.join("jna").display()),
        format!("-Djava.io.tmpdir={}", trial.join("tmp").display()),
        format!("-Dorg.lwjgl.system.SharedLibraryExtractPath={}", trial.join("lwjgl").display()),
        format!("-javaagent:{}={}", root.join("agent.jar").display(), trial.display()),
    ];
    if std::env::var("AURORA_P3_JFR").as_deref() == Ok("1") {
        args.push(format!("-XX:StartFlightRecording=name=AuroraP3,settings={},filename={},disk=true,dumponexit=true,maxsize=128m",
            root.join("startup.jfc").display(), trial.join("startup.jfr").display()));
        args.push(format!("-XX:FlightRecorderOptions=repository={}", trial.join("jfr-repository").display()));
    }
    Ok(args)
}

pub(super) fn environment(command: &mut tokio::process::Command) {
    #[cfg(test)]
    if std::env::var_os("AURORA_P3_ROOT").is_none() {
        return;
    }
    let path = trial().expect("validated research trial").join("tmp");
    command.env("TMP", &path).env("TEMP", &path);
}

pub(super) fn spawned(pid: Option<u32>) {
    #[cfg(test)]
    if std::env::var_os("AURORA_P3_ROOT").is_none() {
        return;
    }
    use std::io::Write;
    let unix_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_micros();
    let path = trial().expect("validated trial").join("spawn-private.json");
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
    write!(file, "{{\"processId\":{},\"confirmedUnixUs\":{unix_us}}}", pid.unwrap()).unwrap();
}
