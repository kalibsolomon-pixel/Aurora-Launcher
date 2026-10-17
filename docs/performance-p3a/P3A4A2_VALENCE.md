# Valence 1.1.0 implementation findings

Read-only javap inspection of the verified distributed JAR; official public source
not located. Inputs/disassembly hashes are retained in
[source audit](evidence/p3a4a2/source-audit.json). Static VERIFIED facts below do
not mean the tool was initialized or benchmarked in Minecraft.

| Question | Finding / evidence strength |
| --- | --- |
| Exact compatibility/dependencies/Java | VERIFIED declarations in [artifact audit](P3A4A2_ARTIFACT_AUDIT.md); actual 1.21.11 initialization/recording UNKNOWN |
| Frame hook | VERIFIED MinecraftClientMixin injects render(Z)V HEAD; refmap maps method_1523(Z)V. Entity count read before timestamp. Injection defaultRequire=0. Reflective/linked world-render END then HUD fallback registration is attempted; actual selected hook UNKNOWN |
| Clock/units | VERIFIED FrameCapture.record calls System.nanoTime and stores consecutive long differences in ns |
| Individual intervals | VERIFIED preallocated long[] durations and int[] entity counts in memory; first callback establishes the previous timestamp |
| Raw export | UNSUPPORTED in audited ReportWriter/ReportBuilder. FrameStats feeds aggregate percentiles, histogram, second buckets and envelope series (600-point target). No raw chronological duration/timestamp array is serialized |
| Capacity at 1000–2000 FPS | VERIFIED allocation min(40000000, durationSeconds * frameBufferMaxFps), minimum 1024. Defaults 600 seconds * 1500 = 900000 intervals: enough for 600000 at 1000; exhausts around 450 seconds at sustained 2000 (endpoint details excluded) |
| Overflow | VERIFIED next interval after capacity disarms capture and sets saturated. Report exports frameBufferSaturated. No total missing-frame count or independent hook coverage |
| Render-path allocation | VERIFIED record method performs clock/array operations without an allocation opcode. This narrow fact does not prove whole-callback or fallback zero allocation or zero overhead |
| Reports | VERIFIED measurement disarms before report worker/analysis/file writing. Self-overhead loop is diagnostic only |
| Timing quantity | Consecutive hook intervals include intervening loop work and waits at a stable boundary; not an active-render stopwatch or physical presentation measure. Fallback boundary differs; actual hook/coexistence/order UNKNOWN |
| Aurora/Sodium coexistence | UNKNOWN; deliberately not installed after static raw-data rejection |
| Game/pacing settings | No automatic limiter rewrite identified in audited capture/start/report paths. Entire behavioral absence is not proven. SceneBuilder is separate optional functionality and was not invoked |
| JVM/workload perturbation | VERIFIED gcBeforeRun=true and heapAfterGc=true defaults. Explicit System.gc before recording and after recording for retained-heap measurement. Golden screenshots and other sampling also exist; default behavior is not an uninfluenced JVM experiment |
| Timestamp alignment | CaptureClock samples epoch ms then nanoTime sequentially, without a measured bracket/drift bound. No raw frame timeline export; precise JFR frame attribution UNSUPPORTED |

Aggregate P99 may be useful for the tool's own report. It cannot supply an
independently recomputed raw P99, arbitrary-window tail review, frame-by-frame
completeness review or precise frame/GC proximity analysis. Reduced series cannot
be expanded back into missing raw data. This static limitation is sufficient to
reject its intended primary role without an unnecessary installation.

The [developer description](https://modrinth.com/mod/valence-benchmark) is not
independent overhead evidence. No runtime failure, crash or compatibility PASS is
invented. No configuration was changed to overcome the limitation.
