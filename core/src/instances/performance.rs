//! Explicit instance modes and bounded, advisory-only host memory information.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::settings::{InvalidJvmArguments, parse_jvm_arguments};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PerformanceMode {
    Efficient,
    Balanced,
    Performance,
    Custom,
}

/// Opted-in modes support only portable properties/assertions until VM-specific
/// settings have exact-build validation. Legacy configurations use the old parser.
pub fn validate_arguments(text: &str) -> Result<Vec<String>, InvalidJvmArguments> {
    let args = parse_jvm_arguments(text)?;
    let collectors = args
        .iter()
        .filter(|a| a.starts_with("-XX:+Use") && a.ends_with("GC"))
        .collect::<std::collections::HashSet<_>>();
    if collectors.len() > 1 {
        return Err(InvalidJvmArguments::PerformancePolicy(
            "Choose one garbage collector; multiple collectors are mutually exclusive.",
        ));
    }
    for arg in &args {
        if let Some(property) = arg.strip_prefix("-D") {
            let key = property.split('=').next().unwrap_or("");
            if key.is_empty()
                || !key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            {
                return Err(InvalidJvmArguments::PerformancePolicy(
                    "Use a nonempty JVM property name containing letters, numbers, dots, underscores or hyphens.",
                ));
            }
            if key.starts_with("java.")
                || key.starts_with("jdk.")
                || key.starts_with("sun.")
                || key.starts_with("minecraft.launcher.")
                || matches!(
                    key,
                    "jna.tmpdir"
                        | "org.lwjgl.system.SharedLibraryExtractPath"
                        | "io.netty.native.workdir"
                        | "log4j.configurationFile"
                )
            {
                return Err(InvalidJvmArguments::PerformancePolicy(
                    "This JVM property belongs to managed Java or Minecraft launch paths. Keep the existing configuration or remove it explicitly before choosing a mode.",
                ));
            }
        } else if !matches!(
            arg.as_str(),
            "-ea"
                | "-da"
                | "-esa"
                | "-dsa"
                | "-enableassertions"
                | "-disableassertions"
                | "-enablesystemassertions"
                | "-disablesystemassertions"
        ) {
            return Err(InvalidJvmArguments::PerformancePolicy(
                "This JVM option is not supported in performance modes. GC/VM tuning and argument-file indirection require exact-build validation. Your text has been kept; use Existing Configuration or revise it explicitly.",
            ));
        }
    }
    Ok(args)
}

pub fn argument_notices(text: &str) -> Vec<String> {
    let Ok(args) = parse_jvm_arguments(text) else {
        return Vec::new();
    };
    let mut previous = HashMap::new();
    let mut notices = Vec::new();
    for arg in args {
        if let Some(property) = arg.strip_prefix("-D") {
            let (key, value) = property.split_once('=').unwrap_or((property, ""));
            if let Some(old) = previous.insert(key.to_owned(), value.to_owned()) {
                let message = if old == value {
                    "Repeated identical system properties are retained."
                } else {
                    "Repeated system properties keep their order; the last value takes effect."
                };
                if !notices.iter().any(|n| n == message) {
                    notices.push(message.to_owned());
                }
            }
        }
    }
    notices
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryFacts {
    pub total_mib: Option<u64>,
    pub available_mib: Option<u64>,
    pub architecture: String,
    pub source: String,
    pub age_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAdvice {
    pub facts: MemoryFacts,
    pub recommended_mib: Option<u32>,
    pub explanation: String,
    pub unknowns: Vec<String>,
    pub argument_notices: Vec<String>,
}

pub fn recommend(facts: MemoryFacts, mode: Option<PerformanceMode>) -> MemoryAdvice {
    let budget = match (facts.total_mib, facts.available_mib) {
        (Some(total), Some(available))
            if facts.age_seconds <= 30 && total > 0 && available <= total =>
        {
            let cap = match facts.architecture.as_str() {
                "x86" | "i686" => 1024,
                "x86_64" | "aarch64" => 32768,
                _ => 0,
            };
            Some(
                total
                    .saturating_sub(4096)
                    .min(available.saturating_sub(2048))
                    .min(cap),
            )
        }
        _ => None,
    };
    let target = if mode == Some(PerformanceMode::Efficient) {
        1536
    } else {
        2048
    };
    let recommendation = budget.filter(|b| *b >= 512).map(|b| b.min(target) as u32);
    let explanation = if recommendation.is_some() {
        "Conservative heuristic: reserve 4096 MiB from total RAM and 2048 MiB from currently available RAM for OS/background, native Java and unknown shared-GPU pressure. Efficient targets at most 1536 MiB; other modes at most 2048 MiB. Performance currently matches Balanced. This is not a measured optimum. Only an explicit draft change and Save applies it."
    } else {
        "No recommendation: memory/architecture information is missing, inconsistent, stale or leaves less than 512 MiB after reserves. Your saved heap is unchanged."
    };
    MemoryAdvice {
        facts,
        recommended_mib: recommendation,
        explanation: explanation.into(),
        unknowns: vec![
            "GPU shared-memory demand and native Java peaks are unknown.".into(),
            "Workload live set and burst demand are unmeasured; mod count is not used.".into(),
            "Available RAM is a point-in-time observation, not a guarantee at launch.".into(),
        ],
        argument_notices: Vec::new(),
    }
}

pub fn host_facts() -> MemoryFacts {
    static CACHE: OnceLock<Mutex<Option<(Instant, MemoryFacts)>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if let Some((at, facts)) = &*cache {
        if at.elapsed() < Duration::from_secs(30) {
            let mut facts = facts.clone();
            facts.age_seconds = at.elapsed().as_secs();
            return facts;
        }
    }
    let (total, available, source) = read_memory();
    let facts = MemoryFacts {
        total_mib: total,
        available_mib: available,
        architecture: std::env::consts::ARCH.into(),
        source: source.into(),
        age_seconds: 0,
    };
    *cache = Some((Instant::now(), facts.clone()));
    facts
}

#[cfg(windows)]
fn read_memory() -> (Option<u64>, Option<u64>, &'static str) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return (None, None, "unavailable");
    }
    (
        Some(status.ullTotalPhys / 1048576),
        Some(status.ullAvailPhys / 1048576),
        "Windows GlobalMemoryStatusEx",
    )
}
#[cfg(target_os = "linux")]
fn read_memory() -> (Option<u64>, Option<u64>, &'static str) {
    use std::io::Read;
    let text = std::fs::File::open("/proc/meminfo").and_then(|f| {
        let mut text = String::new();
        f.take(8192).read_to_string(&mut text)?;
        Ok(text)
    });
    let Ok(text) = text else {
        return (None, None, "unavailable");
    };
    let value = |key| {
        text.lines().find_map(|line| {
            let rest = line.strip_prefix(key)?;
            let mut parts = rest.split_whitespace();
            let kib = parts.next()?.parse::<u64>().ok()?;
            if parts.next()? != "kB" || parts.next().is_some() {
                return None;
            }
            Some(kib / 1024)
        })
    };
    (
        value("MemTotal:"),
        value("MemAvailable:"),
        "Linux /proc/meminfo",
    )
}
#[cfg(not(any(windows, target_os = "linux")))]
fn read_memory() -> (Option<u64>, Option<u64>, &'static str) {
    (None, None, "unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn facts(total: u64, available: Option<u64>, arch: &str) -> MemoryFacts {
        MemoryFacts {
            total_mib: Some(total),
            available_mib: available,
            architecture: arch.into(),
            source: "fixture".into(),
            age_seconds: 0,
        }
    }
    #[test]
    fn reserves_modes_pressure_unknown_and_address_space() {
        for total in [8192, 16384, 32768, 65536, u64::MAX] {
            for mode in [
                PerformanceMode::Efficient,
                PerformanceMode::Balanced,
                PerformanceMode::Performance,
                PerformanceMode::Custom,
            ] {
                let advice = recommend(facts(total, Some(total - 2048), "x86_64"), Some(mode));
                assert_eq!(
                    advice.recommended_mib,
                    Some(if mode == PerformanceMode::Efficient {
                        1536
                    } else {
                        2048
                    })
                );
            }
        }
        assert_eq!(
            recommend(facts(8192, Some(3072), "x86_64"), None).recommended_mib,
            Some(1024)
        );
        for available in [None, Some(0), Some(2048), Some(2559), Some(8193)] {
            assert_eq!(
                recommend(facts(8192, available, "x86_64"), None).recommended_mib,
                None
            );
        }
        assert_eq!(
            recommend(facts(8192, Some(2560), "x86_64"), None).recommended_mib,
            Some(512)
        );
        assert_eq!(
            recommend(facts(0, Some(0), "x86_64"), None).recommended_mib,
            None
        );
        assert_eq!(
            recommend(facts(8192, Some(8192), "x86"), None).recommended_mib,
            Some(1024)
        );
        assert_eq!(
            recommend(facts(8192, Some(8192), "unknown"), None).recommended_mib,
            None
        );
        let mut stale = facts(8192, Some(8192), "x86_64");
        stale.age_seconds = 31;
        assert_eq!(recommend(stale, None).recommended_mib, None);
    }
    #[test]
    fn arguments_preserve_order_and_classify_duplicates_without_rewriting() {
        let text = r#"-Dlabel="two words" -Dlabel=last -Dsame=1 -Dsame=1 -ea -ea"#;
        assert_eq!(
            validate_arguments(text).unwrap(),
            parse_jvm_arguments(text).unwrap()
        );
        assert_eq!(argument_notices(text).len(), 2);
        for bad in [
            "-Xmx4G",
            "-Xms1G",
            "-XX:+UseG1GC -XX:+UseZGC",
            "-XX:+UseG1GC",
            "-XX:MaxGCPauseMillis=100",
            "--class-path=other",
            "@args",
            "-XX:Flags=file",
            "-javaagent:agent.jar",
            "-Djava.library.path=other",
            "-Dlog4j.configurationFile=other",
            "-D=bad",
            "-XX:+Unknown",
            "-version",
            "",
        ] {
            if bad.is_empty() {
                assert!(validate_arguments(bad).is_ok());
            } else {
                assert!(validate_arguments(bad).is_err(), "{bad}");
            }
        }
    }

    #[test]
    fn registry_migration_transitions_restart_and_rejected_writes_preserve_settings() {
        use super::super::{InstanceRegistry, settings::InstanceConfiguration};
        let mut registry =
            InstanceRegistry::from_json(include_str!("fixtures/registry-v3-production.json"))
                .unwrap();
        let mut configuration = registry.instances()[0].configuration().clone();
        let legacy_text = r#"-XX:+UseG1GC -Dlabel="two words" -Dlabel=last"#;
        configuration.set_additional_jvm_arguments(legacy_text);
        configuration.set_memory_mib(3217);
        registry.instances_mut()[0].set_configuration(configuration.clone());
        let root =
            std::env::temp_dir().join(format!("aurora-p2-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("instances.json");
        let mut old = serde_json::to_value(&registry).unwrap();
        old["schemaVersion"] = 5.into();
        std::fs::write(&path, old.to_string()).unwrap();
        let migrated = InstanceRegistry::load_and_migrate(&path).unwrap();
        assert_eq!(migrated.instances()[0].configuration(), &configuration);
        assert!(
            migrated.instances()[0]
                .configuration()
                .performance_mode()
                .is_none()
        );
        assert_eq!(
            migrated.instances()[0]
                .configuration()
                .validated_jvm_arguments()
                .unwrap(),
            parse_jvm_arguments(legacy_text).unwrap()
        );
        let bytes = std::fs::read(&path).unwrap();
        let mut rejected = migrated.clone();
        let mut invalid = configuration.clone();
        invalid.set_performance_mode(Some(PerformanceMode::Balanced));
        rejected.instances_mut()[0].set_configuration(invalid);
        assert!(rejected.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        configuration.set_additional_jvm_arguments(r#"-Dlabel="two words" -Dlabel=last -ea -ea"#);
        let original = configuration.clone();
        registry = InstanceRegistry::load(&path).unwrap();
        for mode in [
            PerformanceMode::Balanced,
            PerformanceMode::Efficient,
            PerformanceMode::Performance,
            PerformanceMode::Custom,
            PerformanceMode::Balanced,
            PerformanceMode::Custom,
            PerformanceMode::Efficient,
        ] {
            configuration.set_performance_mode(Some(mode));
            registry.instances_mut()[0].set_configuration(configuration.clone());
            registry.save(&path).unwrap();
            registry = InstanceRegistry::load(&path).unwrap();
            let restored = registry.instances()[0].configuration();
            assert_eq!(restored.performance_mode(), Some(mode));
            assert_eq!(restored.memory_mib(), 3217);
            assert_eq!(
                restored.additional_jvm_arguments(),
                original.additional_jvm_arguments()
            );
            assert_eq!(restored.loader(), original.loader());
            assert_eq!(restored.window(), original.window());
        }
        for damaged in [r#"{"schemaVersion":99,"instances":[]}"#, "not json"] {
            std::fs::write(&path, damaged).unwrap();
            assert!(registry.save(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), damaged);
        }
        let mut unknown = serde_json::to_value(&registry).unwrap();
        unknown["instances"][0]["configuration"]["unrecognizedPreference"] = true.into();
        assert!(InstanceRegistry::from_json(&unknown.to_string()).is_err());
        unknown["instances"][0]["configuration"]
            .as_object_mut()
            .unwrap()
            .remove("unrecognizedPreference");
        unknown["instances"][0]["configuration"]["performanceMode"] = "turbo".into();
        assert!(InstanceRegistry::from_json(&unknown.to_string()).is_err());
        unknown["instances"][0]["configuration"]["performanceMode"] = "balanced".into();
        unknown["schemaVersion"] = 5.into();
        assert!(InstanceRegistry::from_json(&unknown.to_string()).is_err());
        assert!(
            InstanceConfiguration::for_minecraft_version("1.21.11")
                .performance_mode()
                .is_none()
        );
        // Only files this test created beneath its exclusive disposable root.
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(root.join(".instances-write.lock")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
