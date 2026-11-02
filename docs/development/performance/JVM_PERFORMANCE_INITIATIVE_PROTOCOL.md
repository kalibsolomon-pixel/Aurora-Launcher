# Java 21 initiative comparison — fixed before collection

2026-10-19. Owner authorizes this cross-repository initiative. This is a bounded
Windows Minecraft comparison, separate from historical strict schema-1 and
practical captures; no old invalid run is promoted.

Six fresh processes, order A/B/C/C/B/A: A default G1, Xmx2048m; B default G1,
Xmx1536m; C `-XX:+UseZGC -XX:+ZGenerational`, Xmx2048m. No Xms override. Same
managed Microsoft Java 21.0.7+6, current Client source, Fabric/Sodium/OpenGL,
pristine copied Cperf1a world/pose, RD4/SD12, 2560×1440, global PARK/500 at 240,
native Unlimited, VSync off. World ticks normally to exercise allocation/GC;
these numbers are not directly comparable to frozen C-PERF.2B. QuickPlay and
completed accessibility onboarding reduce unmeasured setup; no camera correction
after entry. Original worlds/configs/evidence remain untouched.

Each run: observe entry camera, >=60s stationary warmup, observe camera and hide
F3/clear chat, one automatic 60s Overlay capture plus explicit PID/creation-handle
process capture at 0.5s, observe final camera, normal save/exit and exact NBT pose.
No builds, exports, JFR parsing or other benchmarks during capture. Record timing
gaps; internal callbacks/process/JFR windows do not become sample-identical.

All runs use the same startup JFR allowlist: GarbageCollection, GCPhasePause,
GCHeapSummary, DataLoss and SafepointBegin/End. No JVM arguments, environment,
system properties, process enumeration or execution/allocation stack sampling.
Readiness/startup log timestamps are contextual and include startup JFR cost.
Verify effective collector/max heap with the exact JVM's VM.flags, retaining only
numeric/boolean fields. No unsupported flag enters a recommended preset.

Validity: expected heap/collector/mods, stable focus/camera/options, complete CSV
with automatic end/count/footer, positive intervals, complete 121-sample process
capture, bounded parseable JFR with no observed DataLoss, normal exit and exact
saved pose. GC/outliers alone never invalidate. Preserve all failed/partial runs;
one replacement maximum for a specific diagnosed validity defect, declared before
capture. No heap/flag/driver/runtime sweeps to rescue an unfavorable configuration.

Compare the two process results per configuration, not millions of callbacks as
independent trials. Report callback FPS/P95/P99, >1.25T/>2T/>50ms and one-second
stall clustering, process CPU/RSS and host availability/busy context, GC counts,
pause rank statistics and total time in the declared common capture window.
GC pause association is not individual-frame causality. JFR is an observer and
its collector-dependent overhead is uncalibrated; CPU/RSS differences alone are
not accepted performance improvements.

A candidate needs repeatable material benefit beyond observed reference spread,
no >0.1ms repeated callback P95/P99 regression, throughput within 1%, no worsening
stall/GC failure/stability behavior, and a credible memory/CPU explanation. Two
runs per configuration can reject an obvious regression or identify a candidate;
they cannot establish a broadly superior default. If effects overlap variability,
retain 2048 MiB/default G1, Java-chosen initial heap and explicit manual control.
OOM/crash/hang, broken ownership or preservation stops that candidate. No new
runtime is downloaded solely for a vendor comparison; only supported installed
Java 21 is used. No OS/global tuning. No Linux or display-latency acceptance.

Oracle's [Java 21 G1 guide](https://docs.oracle.com/en/java/javase/21/gctuning/garbage-first-garbage-collector-tuning.html)
starts from default G1 settings and diagnoses application behavior before tuning.
[Java 21 release notes](https://www.oracle.com/java/technologies/javase/21-relnote-issues.html)
document the two generational ZGC flags. Exact-build startup validation remains
required. This justifies a small comparison, not a promise that more flags win.
