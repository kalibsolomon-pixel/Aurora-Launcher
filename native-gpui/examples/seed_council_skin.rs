//! Imports a supplied sample skin into explicitly marked disposable showcase data.
//! Run with: cargo run --example seed_council_skin -- <fixture-root> <sample.png>
use aurora_launcher_lib::{application, host::Host};
use std::{error::Error, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("Expected disposable fixture root and sample PNG".into());
    }
    let root = PathBuf::from(&args[0]).canonicalize()?;
    let marker = std::fs::read(root.join("AURORA_DISPOSABLE_DIAGNOSTIC"))?;
    if !marker.starts_with(b"Disposable") {
        return Err("Only an explicitly marked disposable fixture is accepted".into());
    }
    let bytes = std::fs::read(PathBuf::from(&args[1]))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let host = Host::native(
        Ok(root),
        env!("CARGO_PKG_VERSION"),
        runtime.handle().clone(),
        |_, _| {},
    );
    let request = serde_json::from_value(serde_json::json!({
        "name": "Workshop sample", "model": "classic", "bytes": bytes
    }))?;
    let imported = application::import_skin_preset(host, request)
        .map_err(|error| format!("Skin import rejected: {error:?}"))?;
    println!("{}", serde_json::to_string(&imported)?);
    Ok(())
}
