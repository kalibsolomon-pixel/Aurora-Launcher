//! High-confidence explanations of unsuccessful supervised exits. Inputs are
//! bounded, already redacted output; raw exceptions remain in the launch log.
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchFailureDiagnostic {
    pub category: String,
    pub implicated_mod_ids: Vec<String>,
    pub requirement: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
}

fn bounded(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect()
}

pub fn classify(output: &str, java_major: Option<u32>) -> LaunchFailureDiagnostic {
    let unknown = || {
        LaunchFailureDiagnostic {
        category: "unknownCrash".into(),
        implicated_mod_ids: Vec::new(),
        requirement: None,
        message: "Minecraft exited unsuccessfully. Review the preserved instance launch log for the underlying exception.".into(),
        suggested_action: None,
    }
    };
    let output = if output.len() > 1024 * 1024 {
        let mut start = output.len() - 1024 * 1024;
        while !output.is_char_boundary(start) {
            start += 1;
        }
        &output[start..]
    } else {
        output
    };
    for line in output.lines() {
        if let Some(rest) = line
            .split_once("Multiple overrides for option '")
            .map(|(_, rest)| rest)
        {
            if !line.contains("Exception") && !line.contains("ERROR") {
                continue;
            }
            if let Some((option, after)) = rest.split_once("'!") {
                let sources = after.split_once("Sources:").map(|(_, s)| s).unwrap_or("");
                let ids: Vec<_> = sources
                    .split([',', ' '])
                    .filter(|s| !s.is_empty() && *s != "and")
                    .filter(|s| {
                        s.len() <= 64
                            && s.bytes().all(|b| {
                                b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')
                            })
                    })
                    .take(8)
                    .map(str::to_owned)
                    .collect();
                if ids.len() >= 2 {
                    return LaunchFailureDiagnostic {
                        category: "modConfigurationConflict".into(),
                        implicated_mod_ids: ids.clone(),
                        requirement: Some(bounded(option)),
                        message: format!(
                            "Launch failed: mod configuration conflict. {} both attempted to override option '{}'. Disable or remove one of the implicated mods and retry. The launch log preserves the exception.",
                            ids.join(" and "),
                            bounded(option)
                        ),
                        suggested_action: Some("disableMod".into()),
                    };
                }
            }
        }
        if (line.contains("compatibility level") || line.contains("Compatibility level"))
            && (line.contains("not supported")
                || line.contains("could not be set")
                || line.contains("incompatible"))
        {
            if let Some(level) = line
                .split_once("JAVA_")
                .and_then(|(_, rest)| rest.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|n| n.parse::<u32>().ok())
                .filter(|n| (6..=99).contains(n))
            {
                return LaunchFailureDiagnostic {
                    category: "javaCompatibility".into(),
                    implicated_mod_ids: Vec::new(),
                    requirement: Some(format!("JAVA_{level}")),
                    message: format!(
                        "Launch failed: Java compatibility. A declared Mixin configuration requested JAVA_{level}{}; Mixin rejected it. Inspect the preserved launch exception for the config and mod. Aurora will not change managed Java automatically.",
                        java_major.map_or(String::new(), |n| format!(
                            ", but this instance uses managed Java {n}"
                        ))
                    ),
                    suggested_action: Some("openJavaSettings".into()),
                };
            }
        }
        if line.contains("Incompatible mods found!")
            || line.contains("Mod resolution encountered an incompatible mod set")
        {
            return LaunchFailureDiagnostic {
                category: "fabricDependencyResolution".into(),
                implicated_mod_ids: Vec::new(),
                requirement: None,
                message: "Launch failed: Fabric dependency resolution. Fabric rejected the active mod requirements. Review its required/installed versions in the preserved launch log, then adjust the mod set explicitly.".into(),
                suggested_action: Some("openContent".into()),
            };
        }
        if line.contains("Missing or unsupported mandatory dependencies:")
            || line.contains("net.minecraftforge.fml.ModLoadingException")
            || line.contains("net.neoforged.fml.ModLoadingException")
            || line.contains("Mod File Failure")
        {
            return LaunchFailureDiagnostic {
                category: "forgeDependencyResolution".into(),
                implicated_mod_ids: Vec::new(),
                requirement: None,
                message: "Launch failed: Forge/NeoForge mod loading error. One or more mods have missing dependencies or incompatible versions. Inspect the launch log and update or disable the implicated mods.".into(),
                suggested_action: Some("openContent".into()),
            };
        }
        if line.contains("UnsupportedClassVersionError:") {
            return LaunchFailureDiagnostic {
                category: "javaCompatibility".into(),
                implicated_mod_ids: Vec::new(),
                requirement: None,
                message: "Launch failed: Java compatibility. A class requires a newer Java bytecode version. The preserved launch log identifies the class and supported versions.".into(),
                suggested_action: Some("openJavaSettings".into()),
            };
        }
        if (line.contains("ParsingException")
            || line.contains("JsonSyntaxException")
            || line.contains("ConfigLoadingException"))
            && (line.contains("config/")
                || line.contains("config\\")
                || line.contains(".toml")
                || line.contains(".json"))
        {
            return LaunchFailureDiagnostic {
                category: "corruptConfig".into(),
                implicated_mod_ids: Vec::new(),
                requirement: None,
                message: "Launch failed: a mod configuration file could not be parsed. Open the instance config folder, repair or remove the damaged config file named in the launch log, and retry.".into(),
                suggested_action: Some("openConfigFolder".into()),
            };
        }
    }
    if output.contains("MixinApplyError:")
        || output.contains("MixinInitialisationError:")
        || output.contains("MixinTransformerError:")
    {
        return LaunchFailureDiagnostic {
            category: "mixinInitialization".into(),
            implicated_mod_ids: Vec::new(),
            requirement: None,
            message: "Launch failed: Mixin initialization. A mod transformation failed; review the underlying exception in the preserved launch log. No mod was disabled automatically.".into(),
            suggested_action: Some("openContent".into()),
        };
    }
    unknown()
}

/// Explicit local analysis of live/partial output and the newest crash report.
pub fn diagnose(
    output: &str,
    crash_report: &str,
    java_major: Option<u32>,
) -> LaunchFailureDiagnostic {
    for text in [output, crash_report] {
        let known = classify(text, java_major);
        if known.category != "unknownCrash" {
            return known;
        }
        for (pattern, category, message, action) in [
            (
                "OutOfMemoryError",
                "outOfMemory",
                "The game ran out of memory. Increase the instance memory limit if your computer has enough free RAM, or reduce resource packs and mods.",
                Some("increaseMemory"),
            ),
            (
                "nvoglv64.dll",
                "graphicsDriver",
                "The NVIDIA OpenGL driver (nvoglv64.dll) crashed. Update your NVIDIA graphics driver or test disabling shader/rendering mods.",
                None,
            ),
            (
                "atio6axx.dll",
                "graphicsDriver",
                "The AMD OpenGL driver (atio6axx.dll) crashed. Update your AMD Radeon graphics driver or test disabling shader/rendering mods.",
                None,
            ),
            (
                "igxelpicd64.dll",
                "graphicsDriver",
                "The Intel graphics OpenGL driver (igxelpicd64.dll) crashed. Update your Intel graphics driver or test disabling shader/rendering mods.",
                None,
            ),
            (
                "EXCEPTION_ACCESS_VIOLATION",
                "nativeCrash",
                "A native library or graphics driver crashed. Update the graphics driver and review graphics mods in the crash report.",
                None,
            ),
            (
                "GLFW error 65542",
                "graphicsDriver",
                "OpenGL is unavailable. Install the graphics driver supplied by your GPU manufacturer.",
                None,
            ),
            (
                "No space left on device",
                "diskFull",
                "The disk is full. Free space on the instance drive before retrying.",
                Some("openStorage"),
            ),
        ] {
            if text.contains(pattern) {
                return LaunchFailureDiagnostic {
                    category: category.into(),
                    implicated_mod_ids: vec![],
                    requirement: None,
                    message: message.into(),
                    suggested_action: action.map(str::to_owned),
                };
            }
        }
    }
    LaunchFailureDiagnostic {
        category: "noKnownProblem".into(),
        implicated_mod_ids: vec![],
        requirement: None,
        message: "No known problem detected. You can upload the redacted log to get help. Diagnosis runs locally and sends nothing.".into(),
        suggested_action: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_diagnosis_accepts_partial_logs_and_crash_reports() {
        assert_eq!(diagnose("INFO ready", "", None).category, "noKnownProblem");
        assert_eq!(
            diagnose(
                "WARN harmless",
                "java.lang.OutOfMemoryError: heap space",
                None
            )
            .category,
            "outOfMemory"
        );
        assert_eq!(
            diagnose("UnsupportedClassVersionError: wrong bytecode", "", None).category,
            "javaCompatibility"
        );
    }
    #[test]
    fn bounded_unicode_tail_preserves_late_fatal_evidence() {
        let output = format!(
            "{}\nException: Multiple overrides for option 'custom:setting'! Sources: first and second",
            "é".repeat(1024 * 1024)
        );
        assert_eq!(
            classify(&output, Some(21)).implicated_mod_ids,
            ["first", "second"]
        );
    }
    #[test]
    fn arbitrary_override_sources() {
        let result = classify(
            "java.lang.IllegalStateException: Multiple overrides for option 'sodium:general.fullscreen'! Sources: chloride and cubes-without-borders",
            Some(21),
        );
        assert_eq!(
            result.implicated_mod_ids,
            ["chloride", "cubes-without-borders"]
        );
        assert_eq!(classify("Exception: Multiple overrides for option 'another:option'! Sources: alpha and beta",None).implicated_mod_ids,["alpha","beta"]);
    }
    #[test]
    fn runtime_fabric_and_unknown_failures() {
        assert_eq!(classify("Compatibility level JAVA_25 could not be set. Level is not supported by the active JRE",Some(21)).category,"javaCompatibility");
        assert_eq!(
            classify("Incompatible mods found!", None).category,
            "fabricDependencyResolution"
        );
        assert_eq!(
            classify("MixinApplyError: failed", None).category,
            "mixinInitialization"
        );
        assert_eq!(
            classify("Exception: opaque failure", None).category,
            "unknownCrash"
        );
        assert_eq!(
            classify("Compatibility level JAVA_", None).category,
            "unknownCrash"
        );
    }
    #[test]
    fn harmless_warnings_are_not_fatal_evidence() {
        for line in [
            "WARN ClassNotFoundException: net.irisshaders.example",
            "WARN compatibility level set to JAVA_21",
            "WARN Multiple overrides for option 'thing'! Sources: alpha and beta",
        ] {
            assert_eq!(classify(line, Some(21)).category, "unknownCrash");
        }
    }
}
