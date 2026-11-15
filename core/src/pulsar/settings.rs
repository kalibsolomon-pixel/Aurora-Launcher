use super::{Error, parse, service, store, view::*};
use crate::paths::ManagedPaths;
use serde_json::{Value, json};

fn defaults() -> Value {
    let mut defaults = json!({"schemaVersion":2,"tracking":true,"recording":true,"perHitChat":false,"summary":true,"sound":true,"alertSeconds":6.0,"perfectReachTarget":3.0,"perfectReachTolerance":0.06,"timingToleranceTicks":1.0,"serverInactivitySeconds":60.0,"resultGraceSeconds":3.0,"exchangeInactivitySeconds":3.0,"jumpWindowTicks":4,"perfectReachSound":true,"animate":true,"floatingText":false,"chatSummary":false,"hudOverlay":false,"soundChannel":"UI","density":"COMFORTABLE","reducedMotion":false,"chartPoints":true,"chartGrid":true,"trendWindow":20});
    defaults.as_object_mut().unwrap().extend(json!({"anonymizeExports":true,"exportMotion":true,"openAfterExport":false,"accent":-7812950,"forwardColor":-7812950,"backwardColor":-1662344,"neutralColor":-6642004,"retention":100,"inactivitySeconds":8.0,"correlationMillis":250.0,"comboSeconds":3.0,"perfectCooldown":0.95,"greatCooldown":0.85,"jumpWindowMillis":150.0,"maxStorageMb":64.0,"volume":0.35,"pitch":1.2,"diagnosticCapture":false,"feedback":[{"color":-7812950,"sound":"minecraft:entity.experience_orb.pickup","volume":0.35,"pitch":1.2,"visual":true,"audio":true},{"color":-5977104,"sound":"minecraft:block.note_block.hat","volume":0.35,"pitch":1.2,"visual":true,"audio":true},{"color":-1781110,"sound":"minecraft:block.note_block.hat","volume":0.35,"pitch":1.2,"visual":true,"audio":true},{"color":-1796208,"sound":"minecraft:block.note_block.bass","volume":0.35,"pitch":1.2,"visual":true,"audio":true}]}).as_object().unwrap().clone());
    for (key, x, y, opacity) in [
        ("analytics", 0.02, 0.2, 0.8),
        ("direction", 0.48, 0.7, 0.8),
        ("alert", 0.65, 0.15, 0.9),
    ] {
        defaults[key] = json!({"x":x,"y":y,"scale":1.0,"opacity":opacity,"visible":true,"background":true,"rightAligned":false});
    }
    defaults
}
fn group(key: &str) -> &'static str {
    match key {
        "tracking" | "recording" | "chatSummary" | "retention" | "maxStorageMb" => "Recording",
        "serverInactivitySeconds" | "resultGraceSeconds" => "Server capture",
        "inactivitySeconds"
        | "exchangeInactivitySeconds"
        | "correlationMillis"
        | "comboSeconds"
        | "jumpWindowTicks" => "Combat exchanges",
        "perfectReachTarget" | "perfectReachTolerance" | "timingToleranceTicks" => {
            "Attack classification"
        }
        "pulsarKey" | "pulsarEnabled" => "Controls",
        "sound" | "soundChannel" | "perfectReachSound" | "volume" | "pitch" => "Audio",
        "feedback" | "perHitChat" => "Per-style audio",
        "anonymizeExports" | "exportMotion" | "openAfterExport" => "Data & exports",
        "diagnosticCapture" => "Diagnostics",
        _ => "Appearance",
    }
}
pub(crate) fn inspect(paths: &ManagedPaths, id: &str) -> Result<SettingsView, Error> {
    let source = service::sources(paths)?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| {
            Error::new(
                "pulsar_instance_not_found",
                "Select a registered instance to inspect its settings.",
            )
        })?;
    let path = source.root.join("config/aurora.json");
    let mut warnings = Vec::new();
    let mut effective = defaults();
    let mut persisted = serde_json::Map::new();
    let mut schema = None;
    let mut source_path = None;
    if path.exists() {
        let bytes = parse::read_bytes(&source.root, &path, 1024 * 1024)?;
        let aurora: Value = serde_json::from_slice(&bytes).map_err(|_| {
            Error::new(
                "pulsar_settings_invalid",
                "Aurora configuration is malformed; it was preserved.",
            )
        })?;
        source_path = Some(path.to_string_lossy().into_owned());
        if let Some(settings) = aurora["pulsarSettings"].as_object() {
            schema = settings
                .get("schemaVersion")
                .and_then(Value::as_i64)
                .and_then(|n| i32::try_from(n).ok());
            let version = schema.unwrap_or(2);
            if matches!(version, 1 | 2) {
                persisted = settings.clone();
                effective.as_object_mut().unwrap().extend(persisted.clone());
                if version == 1 {
                    let chat = matches!(
                        settings.get("display").and_then(Value::as_str),
                        Some("CHAT" | "BOTH")
                    );
                    effective["chatSummary"] = chat.into();
                    effective["hudOverlay"] = false.into();
                    warnings.push("Schema 1 display mode is shown using the client's chat-summary migration; persistent HUD is retired. No file was changed.".into());
                }
            } else {
                warnings.push("Unsupported nested settings schema. Client-compatible defaults are shown; the persisted document was preserved.".into());
            }
        }
        for key in ["pulsarEnabled", "pulsarKey"] {
            if let Some(v) = aurora.get(key) {
                effective[key] = v.clone();
                persisted.insert(key.into(), v.clone());
            }
        }
    } else {
        warnings.push(
            "No persisted Aurora configuration exists. Recorded-client defaults are shown.".into(),
        );
    }
    let mut settings = Vec::new();
    for (key, value) in effective.as_object().unwrap() {
        settings.push(SettingView{key:key.clone(),group:group(key).into(),label:key.clone(),value:value.clone(),persisted:persisted.contains_key(key),runtime_only:true,help:"Read-only persisted preference/default. Change in game; saved recording policies remain frozen.".into()});
    }
    for (key, label) in [
        ("captureState", "Recording state and duration"),
        ("currentServer", "Current server and opponent"),
        ("capturePolicy", "Live auto-capture policy"),
        ("environment", "Live environment and adapter"),
        ("matchRoundManual", "Current match, round and manual state"),
        ("diagnosticsState", "Live diagnostics capture"),
        ("captureLimitation", "Current capture limitation"),
        ("exportAvailability", "In-flight export availability"),
        ("lastSaved", "Last saved this process"),
        ("keyRouting", "Dashboard key routing and shortcuts"),
        ("audioPreview", "Sound preview"),
        ("resetSettings", "Reset settings"),
        ("importStandalone", "Import standalone preferences"),
    ] {
        settings.push(SettingView{key:key.into(),group:if key=="audioPreview"{"Per-style audio"}else if key=="keyRouting"{"Controls"}else if key=="resetSettings"{"Diagnostics"}else if key=="importStandalone"{"Data & exports"}else{"Server capture"}.into(),label:label.into(),value:Value::Null,persisted:false,runtime_only:true,help:"Change in game. Finalized recordings provide no live control or telemetry protocol.".into()});
    }
    // Standalone files are discoverable without silently importing or editing them.
    for filename in ["phenomena.json", "kwamster.json"] {
        let p = source.root.join("config").join(filename);
        if p.exists() {
            match parse::read_bytes(&source.root,&p,1024*1024).and_then(|bytes|serde_json::from_slice::<Value>(&bytes).map_err(|_|Error::new("pulsar_settings_invalid","Standalone preferences are malformed."))){Ok(v)=>settings.push(SettingView{key:format!("standalone:{filename}"),group:"Data & exports".into(),label:format!("Standalone preferences: {filename}"),value:v,persisted:true,runtime_only:true,help:"Read-only standalone preferences. Explicit import is an in-game action; the original is preserved.".into()}),Err(e)=>warnings.push(e.message)}
        }
    }
    let _ = store::now();
    Ok(SettingsView {
        instance_id: id.into(),
        source_path,
        schema_version: schema,
        settings,
        warnings,
        read_only: true,
    })
}
