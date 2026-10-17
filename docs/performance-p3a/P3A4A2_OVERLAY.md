# Performance Overlay implementation findings

Inspect both current source and exact v1.0.2 source/bytecode. Relevant released
implementations: [entrypoint](https://github.com/itsgeorge22/performanceoverlay/blob/9e923693ece057f84e2a0cd69e86f52ab3bce14b/src/client/java/com/itsgeorge/performanceoverlay/PerformanceOverlayClient.java),
[tracker](https://github.com/itsgeorge22/performanceoverlay/blob/9e923693ece057f84e2a0cd69e86f52ab3bce14b/src/client/java/com/itsgeorge/performanceoverlay/client/FpsTracker.java),
[writer](https://github.com/itsgeorge22/performanceoverlay/blob/9e923693ece057f84e2a0cd69e86f52ab3bce14b/src/client/java/com/itsgeorge/performanceoverlay/client/BenchmarkCsvWriter.java).
Facts here are static VERIFIED unless explicitly labeled runtime or UNKNOWN.

| Question | Finding |
| --- | --- |
| Dependencies | Loader >=0.18.4/Java >=21/API `*`/Cloth `*`; tested exact dependencies in [artifact audit](P3A4A2_ARTIFACT_AUDIT.md) |
| Aurora/Sodium/Lithium | Actual startup, world entry, live overlay and hide key succeeded with Aurora 3.0.0, Sodium 0.8.14 and Lithium 0.21.4; recording coexistence UNKNOWN |
| Hook | Fabric HudElementRegistry.attachElementBefore(CHAT), invoking tracker.onFrame(paused). This is an eligible HUD-callback interval, not render-start or render-to-present timing |
| Clock | System.nanoTime captured near entry to onFrame, delta from previous eligible callback; raw duration retained in ns internally |
| Export precision | elapsed_ms is truncated integer milliseconds relative to benchmarkStartNs; frame_ms rounded to three decimals (1 microsecond formatting), not absolute ns timestamps |
| Rows | First callback after reset omitted. Each later positive eligible dt enqueues one row while active. FREEZE/RESET pause modes exclude callbacks and advance prior timestamp; TRACK retains them. Nonpositive dt silently skipped |
| Synchronous writes | Header/file creation/flush on start; per-frame formatting/write/flush on daemon writer. Finish waits up to 5 seconds after active=false. Queue offer still acquires a lock: it is not lock-free |
| Overlay off | Active benchmark bypasses enabled=false sampling early return and skips display drawing; F7 hide actually verified without a recording. Recording-while-hidden runtime UNKNOWN |
| Allocations | One immutable BenchmarkRow allocated per captured frame on render path. Summary long[] grows/doubles on that path; cached snapshots/text can allocate even with overlay hidden. Writer separately allocates formatted strings |
| Long runs | 4096-row queue, rolling arrays capped 240000, summary 6000 initially then doubles to 5000000 durations (~40 MB payload). CSV continues beyond summary cap; disk use is duration/rate dependent, not fixed bounded storage |
| Loss handling | Queue full/failure aborts with error; footer absent/incomplete on failure. FramesLogged and FramesSummary allow exported-row/summary reconciliation, including summary cap. No independent count of suppressed/missed HUD callbacks; no per-row sequence ID or queue high-water export |
| Metadata | Version, local date to seconds, duration and captured metric settings, end reason/counts. No zoned epoch/nanoTime pair, workload/world/mod/runtime identity, PID, focus, graphics proof or warm-up evidence; external provenance required |
| Limiter waiting | Stable consecutive callbacks span the intervening frame loop, including waits occurring between them. Aurora HEAD waiting can contribute. Phase within loop and callback eligibility affect attribution. Not isolated active work, exact frame-start cadence or physical scanout |
| Game/pacing changes | Inspected entrypoint/tracker change overlay configuration, not game FPS/JVM settings. Progress actionbar at 500 ms is extra work even when overlay is hidden. No changed Aurora PARK settings in actual check |
| GC/memory | MXBean collection-time delta about once/second; not individual stop-the-world pauses. Numeric GC value on polling row, blank on others. Runtime heap used/max at 250-ms cadence; not process RSS. These are context, not JFR replacements |

At 2000 callbacks/s, distinct frames may share the same elapsed_ms. Even identical
adjacent CSV rows can be legitimate after quantization. The format cannot always
distinguish a duplicated row from two equal samples. Row count/footer arithmetic
is valuable but cannot establish every Minecraft iteration was observed. HUD
suppression, pause and world/menu transitions must be reviewed externally.

The source has explicit full-run percentiles/lows, but their definitions/rounding
are not Aurora's type-7 percentile contract. Offline analysis should use exported
frame_ms with its quantization and retain original rolling/summary columns.
Do not silently reinterpret GC deltas as pause durations or elapsed_ms ties as
zero-length frames. [Timebase/overhead limits](P3A4A2_TIMEBASE_OVERHEAD.md).
