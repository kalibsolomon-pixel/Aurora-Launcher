//! Launcher-owned low-latency GC preset for Ephemera-tuned launches.
//!
//! Ephemera JFR attribution showed heavy chunk/worldgen play allocates
//! ~450–570 MB/s with Eden filling to 384–519 MB, producing G1 evacuation
//! pauses of 4–20.7 ms that stop the world (including the render thread).
//! Two separate levers improve frame-time consistency:
//!
//! 1. **Equal heap + pre-touch** (`-Xms == -Xmx -XX:+AlwaysPreTouch`): removes
//!    heap-expansion lag and commit-time page faults when host RAM has at
//!    least 2 GiB of headroom above the configured heap and the heap is at
//!    most 16 GiB (avoiding swap/OOM or multi-second boot stalls on oversized
//!    heaps).
//! 2. **Generational ZGC** on JDK 21+ when both host RAM (≥ 6 GiB) and heap
//!    size (≥ 4 GiB) have enough headroom to avoid `ZAllocationStall`:
//!    - JDK 23+ (JEP 474): `-XX:+UseZGC` (generational is the default;
//!      `-XX:+ZGenerational` is deprecated in 23, obsoleted in 24, removed in 25).
//!    - JDK 21–22: `-XX:+UseZGC -XX:+ZGenerational`.
//!    - JDK 17–20 (or lower RAM/heap): explicit G1 low-latency fallback
//!      (`-XX:+UseG1GC`, 4 MiB regions, 12 ms pause target, 15% reserve).
//!    - Below JDK 17 (e.g. Minecraft 1.13–1.16.5 on Java 8): no preset is
//!      applied, preventing `Unrecognized VM option` startup failures.
//!
//! Custom OpenJ9/Semeru builds ignore unrecognized `-XX:` options by default.
//! Before emitting the preset, `launch::resolve` also checks assembled loader,
//! Mojang, and user JVM arguments so conflicting collectors are never combined.

/// Minimum Java major version eligible for any launcher GC preset.
pub const MIN_PRESET_JAVA_MAJOR: u32 = 17;

/// Minimum Java major version supporting Generational ZGC (JDK 21–22 require
/// `-XX:+ZGenerational`; JDK 23+ makes generational the default).
pub const ZGC_MIN_JAVA_MAJOR: u32 = 21;

/// First Java major version where Generational ZGC is the default for
/// `-XX:+UseZGC` (JEP 474).
pub const ZGC_DEFAULT_GENERATIONAL_JAVA_MAJOR: u32 = 23;

/// Minimum total system RAM (MiB) for the Generational ZGC preset: ZGC needs
/// concurrent headroom and disables CompressedOops (+15–25% heap footprint).
pub const ZGC_MIN_TOTAL_RAM_MIB: u32 = 6 * 1024;

/// Minimum configured heap (MiB) for Generational ZGC to avoid allocation
/// stalls under high worldgen/render allocation rates.
pub const ZGC_MIN_HEAP_MIB: u32 = 4 * 1024;

/// Minimum total system RAM (MiB) for the G1 low-latency preset.
pub const G1_MIN_TOTAL_RAM_MIB: u32 = 3 * 1024;

/// Minimum configured heap (MiB) for the G1 low-latency preset.
pub const G1_MIN_HEAP_MIB: u32 = 1536;

/// Minimum host RAM headroom (MiB) above `memory_mib` required before pinning
/// `-Xms == -Xmx` with `-XX:+AlwaysPreTouch`.
pub const PRETOUCH_HEADROOM_MIB: u32 = 2 * 1024;

/// Maximum heap (MiB) for eager `-Xms` + `-XX:+AlwaysPreTouch` so very large
/// configured heaps (e.g. 24–32 GiB) do not stall launch or over-commit RAM.
pub const PRETOUCH_MAX_HEAP_MIB: u32 = 16 * 1024;

/// Collector family selected by the launcher-owned GC preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcCollector {
    /// Generational ZGC on JDK 23+ (`-XX:+UseZGC`).
    GenerationalZgc,
    /// Generational ZGC on JDK 21–22 (`-XX:+UseZGC -XX:+ZGenerational`).
    GenerationalZgcExplicit,
    /// Explicit G1 low-latency preset on JDK 17+.
    G1LowLatency,
}

/// The launcher-owned low-latency collector preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcPreset {
    pub collector: GcCollector,
    pub pin_initial_heap_and_pretouch: bool,
}

/// Selects the preset from host RAM, configured heap size, and validated Java
/// major version. Returns `None` on Java < 17, unknown host RAM, or hosts /
/// heaps below the minimum G1 threshold.
pub fn select_gc_preset(
    total_ram_mib: Option<u32>,
    memory_mib: u32,
    java_major: u32,
) -> Option<GcPreset> {
    let total = total_ram_mib?;
    if java_major < MIN_PRESET_JAVA_MAJOR {
        return None;
    }
    let collector = if total >= ZGC_MIN_TOTAL_RAM_MIB
        && memory_mib >= ZGC_MIN_HEAP_MIB
        && java_major >= ZGC_MIN_JAVA_MAJOR
    {
        if java_major >= ZGC_DEFAULT_GENERATIONAL_JAVA_MAJOR {
            GcCollector::GenerationalZgc
        } else {
            GcCollector::GenerationalZgcExplicit
        }
    } else if total >= G1_MIN_TOTAL_RAM_MIB && memory_mib >= G1_MIN_HEAP_MIB {
        GcCollector::G1LowLatency
    } else {
        return None;
    };
    let pin_initial_heap_and_pretouch = memory_mib <= PRETOUCH_MAX_HEAP_MIB
        && memory_mib.saturating_add(PRETOUCH_HEADROOM_MIB) <= total;
    Some(GcPreset {
        collector,
        pin_initial_heap_and_pretouch,
    })
}

/// Returns `true` if any argument in `arguments` explicitly selects or disables
/// a JVM garbage collector (HotSpot `-XX:+/-Use*GC` or OpenJ9 `-Xgcpolicy:`).
pub fn has_conflicting_collector_flag<'a>(arguments: impl IntoIterator<Item = &'a str>) -> bool {
    arguments.into_iter().any(|arg| {
        ((arg.starts_with("-XX:+Use") || arg.starts_with("-XX:-Use")) && arg.ends_with("GC"))
            || arg.starts_with("-Xgcpolicy:")
    })
}

/// Returns `true` if any argument in `arguments` already sets an initial or
/// minimum heap size (`-Xms...`, `-XX:InitialHeapSize=...`, `-XX:MinHeapSize=...`).
pub fn has_initial_heap_flag<'a>(arguments: impl IntoIterator<Item = &'a str>) -> bool {
    arguments.into_iter().any(|arg| {
        arg.starts_with("-Xms")
            || arg.starts_with("-XX:InitialHeapSize=")
            || arg.starts_with("-XX:MinHeapSize=")
    })
}

/// Launcher-owned GC arguments for the preset, given the authoritative heap
/// size. Equal `-Xms`/`-Xmx` plus pre-touch is included only when guarded by
/// host headroom and not already specified elsewhere.
pub fn gc_preset_arguments(preset: GcPreset, memory_mib: u32) -> Vec<String> {
    let mut arguments = Vec::with_capacity(6);
    if preset.pin_initial_heap_and_pretouch {
        arguments.push(format!("-Xms{memory_mib}m"));
        arguments.push("-XX:+AlwaysPreTouch".to_owned());
    }
    match preset.collector {
        GcCollector::GenerationalZgc => arguments.push("-XX:+UseZGC".to_owned()),
        GcCollector::GenerationalZgcExplicit => {
            arguments.push("-XX:+UseZGC".to_owned());
            arguments.push("-XX:+ZGenerational".to_owned());
        }
        GcCollector::G1LowLatency => arguments.extend([
            "-XX:+UseG1GC".to_owned(),
            "-XX:G1HeapRegionSize=4m".to_owned(),
            "-XX:MaxGCPauseMillis=12".to_owned(),
            "-XX:G1ReservePercent=15".to_owned(),
        ]),
    }
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_java_below_17_never_emits_a_preset() {
        for java_major in [8, 11, 16] {
            assert_eq!(select_gc_preset(Some(16 * 1024), 4096, java_major), None);
            assert_eq!(select_gc_preset(Some(32 * 1024), 8192, java_major), None);
        }
    }

    #[test]
    fn preset_selection_respects_java_major_ram_and_heap_floors() {
        assert_eq!(select_gc_preset(None, 4096, 25), None);
        assert_eq!(select_gc_preset(Some(2048), 1536, 25), None);
        assert_eq!(select_gc_preset(Some(16 * 1024), 1024, 25), None);

        // 4 GiB host or <4 GiB heap falls back to G1 low-latency on JDK 17+.
        assert_eq!(
            select_gc_preset(Some(4096), 2048, 25),
            Some(GcPreset {
                collector: GcCollector::G1LowLatency,
                pin_initial_heap_and_pretouch: true,
            })
        );
        assert_eq!(
            select_gc_preset(Some(16 * 1024), 2048, 25),
            Some(GcPreset {
                collector: GcCollector::G1LowLatency,
                pin_initial_heap_and_pretouch: true,
            })
        );
        assert_eq!(
            select_gc_preset(Some(16 * 1024), 3072, 25),
            Some(GcPreset {
                collector: GcCollector::G1LowLatency,
                pin_initial_heap_and_pretouch: true,
            })
        );

        // JDK 17-20 stays on G1 even with 16 GiB RAM and 4 GiB heap.
        for java_major in [17, 20] {
            assert_eq!(
                select_gc_preset(Some(16 * 1024), 4096, java_major),
                Some(GcPreset {
                    collector: GcCollector::G1LowLatency,
                    pin_initial_heap_and_pretouch: true,
                })
            );
        }

        // JDK 21-22 selects Generational ZGC with explicit -XX:+ZGenerational.
        for java_major in [21, 22] {
            assert_eq!(
                select_gc_preset(Some(16 * 1024), 4096, java_major),
                Some(GcPreset {
                    collector: GcCollector::GenerationalZgcExplicit,
                    pin_initial_heap_and_pretouch: true,
                })
            );
        }

        // JDK 23+ selects Generational ZGC without the deprecated flag.
        for java_major in [23, 24, 25] {
            assert_eq!(
                select_gc_preset(Some(16 * 1024), 4096, java_major),
                Some(GcPreset {
                    collector: GcCollector::GenerationalZgc,
                    pin_initial_heap_and_pretouch: true,
                })
            );
        }
    }

    #[test]
    fn oversized_heap_disables_equal_xms_and_pretouch() {
        // 8 GiB heap on an 8 GiB host has < 2 GiB headroom: keep collector, skip pre-touch.
        let preset = select_gc_preset(Some(8 * 1024), 8 * 1024, 25).unwrap();
        assert_eq!(preset.collector, GcCollector::GenerationalZgc);
        assert!(!preset.pin_initial_heap_and_pretouch);
        assert_eq!(gc_preset_arguments(preset, 8 * 1024), vec!["-XX:+UseZGC"]);

        // 24 GiB heap on a 64 GiB host exceeds PRETOUCH_MAX_HEAP_MIB (16 GiB).
        let huge = select_gc_preset(Some(64 * 1024), 24 * 1024, 25).unwrap();
        assert!(!huge.pin_initial_heap_and_pretouch);
        assert_eq!(gc_preset_arguments(huge, 24 * 1024), vec!["-XX:+UseZGC"]);

        // Exact headroom boundary (6 GiB host, 4 GiB heap -> 2 GiB headroom) allows pre-touch.
        let exact = select_gc_preset(Some(6 * 1024), 4 * 1024, 25).unwrap();
        assert!(exact.pin_initial_heap_and_pretouch);
    }

    #[test]
    fn jdk21_and_jdk25_zgc_arguments_match_jep_474_and_490() {
        let jdk21 = select_gc_preset(Some(16 * 1024), 4096, 21).unwrap();
        assert_eq!(
            gc_preset_arguments(jdk21, 4096),
            vec![
                "-Xms4096m",
                "-XX:+AlwaysPreTouch",
                "-XX:+UseZGC",
                "-XX:+ZGenerational",
            ]
        );

        let jdk25 = select_gc_preset(Some(16 * 1024), 4096, 25).unwrap();
        assert_eq!(
            gc_preset_arguments(jdk25, 4096),
            vec!["-Xms4096m", "-XX:+AlwaysPreTouch", "-XX:+UseZGC"]
        );
    }

    #[test]
    fn g1_preset_explicitly_selects_g1_with_low_latency_bounds() {
        let g1 = select_gc_preset(Some(8 * 1024), 3072, 21).unwrap();
        assert_eq!(
            gc_preset_arguments(g1, 3072),
            vec![
                "-Xms3072m",
                "-XX:+AlwaysPreTouch",
                "-XX:+UseG1GC",
                "-XX:G1HeapRegionSize=4m",
                "-XX:MaxGCPauseMillis=12",
                "-XX:G1ReservePercent=15",
            ]
        );
    }

    #[test]
    fn conflict_detectors_catch_collectors_and_xms() {
        assert!(has_conflicting_collector_flag(["-XX:+UseG1GC"]));
        assert!(has_conflicting_collector_flag(["-XX:-UseG1GC"]));
        assert!(has_conflicting_collector_flag([
            "-ea",
            "-XX:+UseShenandoahGC"
        ]));
        assert!(has_conflicting_collector_flag(["-Xgcpolicy:gencon"]));
        assert!(!has_conflicting_collector_flag([
            "-Dfoo=bar",
            "-XX:+UnlockExperimentalVMOptions",
        ]));
        assert!(has_initial_heap_flag(["-Xms512M"]));
        assert!(has_initial_heap_flag(["-XX:InitialHeapSize=1g"]));
        assert!(has_initial_heap_flag(["-XX:MinHeapSize=512m"]));
        assert!(!has_initial_heap_flag(["-Xmx4096m"]));
    }
}
