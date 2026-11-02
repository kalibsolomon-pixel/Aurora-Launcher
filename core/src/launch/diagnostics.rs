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
        LaunchFailureDiagnostic {category:"unknownCrash".into(),implicated_mod_ids:Vec::new(),requirement:None,message:"Minecraft exited unsuccessfully. Review the preserved instance launch log for the underlying exception.".into()}
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
                };
            }
        }
        if line.contains("Incompatible mods found!")
            || line.contains("Mod resolution encountered an incompatible mod set")
        {
            return LaunchFailureDiagnostic {category:"fabricDependencyResolution".into(),implicated_mod_ids:Vec::new(),requirement:None,message:"Launch failed: Fabric dependency resolution. Fabric rejected the active mod requirements. Review its required/installed versions in the preserved launch log, then adjust the mod set explicitly.".into()};
        }
        if line.contains("UnsupportedClassVersionError:") {
            return LaunchFailureDiagnostic {category:"javaCompatibility".into(),implicated_mod_ids:Vec::new(),requirement:None,message:"Launch failed: Java compatibility. A class requires a newer Java bytecode version. The preserved launch log identifies the class and supported versions.".into()};
        }
    }
    if output.contains("MixinApplyError:")
        || output.contains("MixinInitialisationError:")
        || output.contains("MixinTransformerError:")
    {
        return LaunchFailureDiagnostic {category:"mixinInitialization".into(),implicated_mod_ids:Vec::new(),requirement:None,message:"Launch failed: Mixin initialization. A mod transformation failed; review the underlying exception in the preserved launch log. No mod was disabled automatically.".into()};
    }
    unknown()
}

#[cfg(test)]
mod tests {
    use super::*;
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
