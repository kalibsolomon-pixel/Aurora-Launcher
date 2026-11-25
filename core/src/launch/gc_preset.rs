//! Launcher-owned low-latency GC preset for Ephemera-tuned launches.
//!
//! The wave-5 Ephemera JFR attribution showed the JVM allocates ~450–570 MB/s
//! during worldgen-heavy play with an Eden that fills to 384–519 MB, producing
//! G1 evacuation pauses of 4–20.7 ms that stop the world (including the render
//! thread) 8–9 times per 4 seconds. Two separate levers fix the tail:
//!
//! 1. **Equal heap + pre-touch** (`-Xms == -Xmx -XX:+AlwaysPreTouch`): removes
//!    heap-expansion lag and commit-time page faults (the run that motivated
//!    this preset recorded 62 `EvacuationFailed` events and 20.5 ms `G1Old`
//!    pauses while committed heap lagged at ~1.33 GB).
//! 2. **Generational ZGC** on JDK 21+ with RAM headroom (≥ 6 GiB): collapses
//!    the remaining pauses to sub-millisecond concurrent collections at a
//!    small throughput cost. G1 with a 12 ms pause target is the fallback for
//!    memory-constrained hosts.
//!
//! The preset is launcher-owned exactly like the heap argument: the
//! configuration boundary already rejects user GC flags in performance modes,
//! so no duplicate collector flag can exist in the vector.

/// The launcher-owned low-latency collector preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcPreset {
    /// Generational ZGC (the default ZGC mode on JDK 21+): sub-millisecond
    /// pauses; requires heap headroom for concurrent work.
    GenerationalZgc,
    /// G1 with a 12 ms pause target: the fallback for memory-constrained
    /// hosts where ZGC's heap overhead would risk allocation stalls.
    G1LowLatency,
}

/// Minimum total system RAM (MiB) for the ZGC preset: ZGC needs concurrent
/// work headroom and no CompressedOops (+15–25% heap footprint).
pub const ZGC_MIN_TOTAL_RAM_MIB: u32 = 6 * 1024;

/// Selects the preset from host memory: ZGC with ≥ 6 GiB total RAM, otherwise
/// the G1 low-latency fallback. `None` below 3 GiB total RAM (the preset is
/// pointless on hosts that cannot hold a comfortable heap).
pub fn select_gc_preset(total_ram_mib: Option<u32>) -> Option<GcPreset> {
    let total = total_ram_mib?;
    if total >= ZGC_MIN_TOTAL_RAM_MIB {
        Some(GcPreset::GenerationalZgc)
    } else if total >= 3 * 1024 {
        Some(GcPreset::G1LowLatency)
    } else {
        None
    }
}

/// Launcher-owned GC arguments for the preset, given the authoritative heap
/// size. Equal `-Xms`/`-Xmx` plus pre-touch removes expansion and commit
/// latency; the collector flags follow.
pub fn gc_preset_arguments(preset: GcPreset, memory_mib: u32) -> Vec<String> {
    let mut arguments = vec![
        format!("-Xms{memory_mib}m"),
        "-XX:+AlwaysPreTouch".to_owned(),
    ];
    match preset {
        GcPreset::GenerationalZgc => arguments.push("-XX:+UseZGC".to_owned()),
        GcPreset::G1LowLatency => arguments.extend([
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
    fn preset_selection_follows_host_memory() {
        assert_eq!(select_gc_preset(None), None);
        assert_eq!(select_gc_preset(Some(2048)), None);
        assert_eq!(select_gc_preset(Some(4096)), Some(GcPreset::G1LowLatency));
        assert_eq!(
            select_gc_preset(Some(8192)),
            Some(GcPreset::GenerationalZgc)
        );
    }

    #[test]
    fn zgc_preset_pins_equal_heap_and_pre_touch() {
        let arguments = gc_preset_arguments(GcPreset::GenerationalZgc, 4096);
        assert_eq!(
            arguments,
            vec!["-Xms4096m", "-XX:+AlwaysPreTouch", "-XX:+UseZGC"]
        );
    }

    #[test]
    fn g1_preset_sets_pause_target_region_size_and_reserve() {
        let arguments = gc_preset_arguments(GcPreset::G1LowLatency, 3072);
        assert_eq!(
            arguments,
            vec![
                "-Xms3072m",
                "-XX:+AlwaysPreTouch",
                "-XX:G1HeapRegionSize=4m",
                "-XX:MaxGCPauseMillis=12",
                "-XX:G1ReservePercent=15",
            ]
        );
    }
}
