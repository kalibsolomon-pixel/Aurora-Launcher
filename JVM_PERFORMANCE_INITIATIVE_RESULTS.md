# Java 21 comparison — retain the established default

2026-10-19, Windows. Four eligible fresh-process captures completed: A1/B2/B5/A6,
where A is G1/Xmx2048 MiB and B is G1/Xmx1536 MiB. All use managed Microsoft
Java 21.0.7+6, Java-selected initial heap (510 MiB), Client `9513af4`, Minecraft
1.21.11, Loader 0.19.2, API 0.141.4, Sodium 0.8.14, normal world ticks, original
PARK/500 at 240, native Unlimited, VSync off, 2560×1440, RD4/SD12. The
[protocol](JVM_PERFORMANCE_INITIATIVE_PROTOCOL.md) preceded preparation/collection.

| Run | Max heap MiB | Callback FPS | P95 / P99 ms | Max ms | >2T / >50ms | RSS max MiB | GC pauses / total ms | GC pause P95 / max ms |
| --- | ---: | ---: | --- | ---: | --- | ---: | --- | --- |
| A1 | 2048 | 239.987 | 4.805 / 5.046 | 9.382 | 1 / 0 | 1668.758 | 14 / 30.868 | 4.302 / 4.424 |
| B2 | 1536 | 239.931 | 4.811 / 5.013 | 18.409 | 4 / 0 | 1891.090 | 9 / 44.046 | 12.852 / 16.819 |
| B5 | 1536 | 239.949 | 4.829 / 5.057 | 20.152 | 4 / 0 | 1792.691 | 10 / 41.962 | 10.998 / 15.254 |
| A6 | 2048 | 239.988 | 4.860 / 5.123 | 9.851 | 1 / 0 | 1633.734 | 14 / 32.337 | 4.222 / 4.378 |

Callback P95/P99 overlap reference variation; throughput differences are small.
The smaller maximum did **not** reduce observed RSS: B's heap committed
1020/950 MiB versus A's 795/783 MiB. Java ergonomics and process/native memory
matter; maximum heap is not resident memory. Both B runs have longer GC pause
tails and four >2T callbacks versus one in each A. This does not establish
individual-frame GC causality. Retain 2048/default G1 and no Xms override.
1536 remains a user-selectable bounded-memory option, not a demonstrated speed
or resident-memory improvement. No new GC flag bundle or CPU-specific policy.

Whole-process CPU was 72.27/77.42/58.15/64.43% of one logical core; machine CPU
5.14/5.61/4.93/4.86%, available RAM minima 16724/16446/17459/17257 MiB.
The interruption separated the first and second pairs by about two hours and
changed ambient applications/load. Normal world simulation also evolves. CPU
differences are contextual, not attributable wins. This is a stationary,
allocation-active lightweight Aurora workload, not a demanding modpack, low-RAM
host, long-session, vendor comparison or broad preset acceptance.

## Validity, interruptions and observers

Every eligible run has >=60s recorded warmup, entry/pre/post camera evidence,
exact normally saved NBT position/rotation, expected effective heap/collector,
automatic complete 60s CSV, 121 retained-handle process samples, bracketed JFR,
zero observed DataLoss, normal save and Gradle exit 0. No original world changed.

The prescribed C3/C4 generational ZGC comparison has **no eligible timed run**.
Original C3 was interrupted before entry/warmup/capture and exited after JFR's
600s duration. Its sole replacement used pristine inputs and unchanged flags,
declared before launch, but the operator recorded warmup-end at 53.189s, below
the fixed 60s gate. Primary trigger occurred later (69.237s); the declared gate
was not relaxed retrospectively. C4 was stopped by physical Escape before
collection and later exited normally during the pause. All three directories,
logs and recordings remain retained. No additional replacement or parameter
sweep was performed. ZGC startup compatibility was observed, but its performance
is unproven; do not call it slower, faster, rejected on performance or validated.

All runs use the same startup JFC enabling selected JDK GC/safepoint events.
Minecraft registers its own enabled custom JFR events (tick/FPS/network/world
loading), discovered in retained recordings. Therefore raw files are not a strict
JDK-only allowlist. Exports select JDK events only; the review verifies no JVM
argument/environment/property/process-list/execution/allocation-sample events
and no observed DataLoss. Private copied singleplayer was used. JFR's overhead,
including collector dependence, remains uncalibrated. No observer was changed
mid-matrix and no historical invalid recording is promoted.

GC statistics select events fully contained in the explicit process window,
retain boundary crossing counts, and do not assume exact alignment with Overlay.
Overlay measures internal eligible HUD callbacks, not display presentation or
input-to-photon latency. RSS is working set, not used heap; spin wall is not CPU.

## Reproduction and evidence

[Sanitized results](docs/performance-initiative/jvm-results.json) retain measured
values, raw JFR/CSV hashes, event counts, windows, VM fields and invalid-run reasons.
Raw evidence is in Client `run/jvm-initiative/`: prepared six inputs, unused/failed
data, replacement declaration, camera PNGs, events, CSV/process data, raw JFR,
derived exports and runtime logs. Keep failed data. No tool launches automatically.

Use `python -B tools/benchmark/initiative_review.py C:/Dev/Aurora-Client 1 2 5 6`
for these eligible runs. The explicit `--replacement-three` reader preserves the
warmup gate and correctly fails for the ineligible replacement. Preparation
refuses an existing root. No public artifacts, release manifests or original
instance configuration changed.
