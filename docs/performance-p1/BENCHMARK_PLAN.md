# JVM benchmark plan — Performance P1

2026-10-16. This is a measurement protocol, **not approved presets**. No gameplay
result currently supports tuning. Production launch behavior and Settings stay
unchanged. Start with [JVM_LAUNCH_AUDIT.md](JVM_LAUNCH_AUDIT.md), then
[BASELINE_RESULTS.md](BASELINE_RESULTS.md) for the actual host and missing coverage.

## Experiment authority and prerequisites

Use a fresh marked development managed-data root and a separate WebView profile.
The existing debug-only `AURORA_DIAGNOSTIC_DATA_ROOT` path override does not exist
in release builds. Follow the disposable-root pattern in `tools/visual/ui5-native.ps1`
after reviewing its launch options; **do not run old acceptance scripts blindly**.
Do not copy owner configuration, accounts, credentials, worlds, mods or settings.
Create new opaque-ID instances through ordinary native lifecycle operations. Keep
game/runtime acquisition, hash verification, complete validation and session authority.
No test driver may reconstruct or print a full Minecraft command line.

Before a real session, freeze a run manifest for every workload, including exact
mod versions and hashes, pristine world archive/hash, route keyframes, runtime
vendor/full build/manifest SHA-1 and tools/version/configuration. Resolve and verify
official metadata through existing native boundaries; do not infer the runtime from
the Minecraft version alone. Validate the exact runtime's candidate switches with
a bounded structured `java` diagnostic in the **laboratory** (no game credentials,
minimal environment); capture collector/PrintFlagsFinal facts and exit status.
An unsupported switch rejects a candidate, never triggers a runtime substitution.
The current launcher diagnostic's `-version` alone is insufficient for this step.

Real execution requires a quiet owner-approved resource window. A five-run trial
at 5-minute warm-up + 10-minute measurement takes at least **75 minutes per
configuration/workload**, before startup/reset/cooldown. Three candidates across
three workloads take at least **11.25 hours**, with sustained CPU/GPU load, several
GiB of RAM and retained logs/world copies. Screen a small hypothesis set first.
Do not run this while owner games or heavy applications are active. Do not stop
processes by name, change power policies/drivers/BIOS, disable protections, install
capture tools without approval, sign into Microsoft without permission, connect
private servers or upload recordings. None of those actions occurred in this P1.

## Workload definitions

Use Minecraft **1.21.11**, exact Fabric Loader **0.19.5**, and the same validated
managed Java 21 build within each comparison. These are reviewed repository pins,
not a claim that all current mod versions support them. World/route/mod locks below
must be created in disposable instances before trials; they are not populated
benchmark assets in this commit. If a compatible optimization mod is unavailable,
record a revised workload lock before any trial rather than substituting mid-series.

| Workload | Disposable scene and purpose | Content lock |
| --- | --- | --- |
| A — lightweight | New superflat creative world, seed 123456789, fixed midday/clear weather, no entities beyond a frozen count. Pre-generated 512-block square. Fixed camera and repeatable 120-second walking loop inside that square, repeated five times. Ordinary frame-time/launcher overhead | Fabric only, Aurora off, no optional mods; record absence of Fabric API if not needed |
| B — typical Aurora | New normal-terrain world, same seed; pre-generated village/forest route with fixed coordinates, camera keyframes, entity count and terrain snapshot. Repeat a 120-second path five times. Representative client behavior | Aurora Client 3.0.0 and release-pinned Fabric API; freeze an actually compatible Sodium/Lithium/FerriteCore selection with exact versions, SHA-256 and relevant configs before runs. These names define investigation scope, not an installation recommendation |
| C — demanding | Separate normal-terrain snapshot with reproducible dense terrain, a bounded entity/redstone scene and frozen inventory; 24 render / 12 simulation chunks. Repeat a 120-second route five times. Look for pressure, GC and CPU regressions | Same B inventory first to isolate scene pressure; a larger pack is a separately versioned workload, never an in-place change to B or an owner modpack |

Route files must specify start position, yaw/pitch and per-time keyframes/inputs.
Prefer a reviewed local deterministic replay/input method; if only manual input is
available, record route deviation and do not label the experiment deterministic.
Restore each run from the **test-created pristine snapshot**, including options/mod
configs; never restore over owner content. Keep reset and staging helpers confined
to positively created disposable paths. Normal gameplay and chunk-generation tests
are separate cohorts: do not mix first-time generation with pre-generated traversal.

Common A/B settings: 1920×1080 client render area, windowed, render distance 12,
simulation distance 8, graphics Fancy, clouds off, particles all, default textures,
no shaders, no resource packs, no dynamic resolution, FPS unlimited, VSync off.
Record actual client size, monitor refresh/VRR/compositor behavior, FOV, GUI scale,
entity settings and every relevant options/config hash. C changes only its declared
scene/distances. Any mod-specific change becomes a new workload lock. Do not compare
unlimited and capped/VRR workloads as if equivalent; add a separate realistic capped
cohort later if required. Main menu/loading frames never enter gameplay intervals.

Hold OS/build/driver, Java vendor/build/manifest, GPU selection, background workload,
power profile, display, affinity and overlays constant. Inventory actual thermals,
clocks/limits if an approved tool provides them; otherwise mark unknown and record
cooldown intervals/ambient conditions. Record uncontrolled scheduler, background
services and thermal variation explicitly. The Windows host is hybrid CPU/discrete
GPU; this does not stand in for other classes.

## Configurations and hypotheses

Always retain Minecraft-required native/classpath/logging arguments, main class and
session handling. Record the **JVM-only** effective vector with private paths replaced
by semantic labels; never capture account/game arguments, tokens or bridge secrets.
GC/JFR measurement switches are identical across matched candidates and analyzed
separately from the tuning delta.

| ID | JVM tuning delta | Hypothesis / availability |
| --- | --- | --- |
| A0 current launcher | Default saved maximum 2048 MiB; no initial heap, collector or other tuning, empty custom text | Actual source-derived production baseline. A separately chosen per-workload override must be recorded as a new baseline, not silently called the default |
| A1 equivalent Java 21 defaults | Same 2048 MiB maximum and ergonomic initial heap; required Minecraft arguments only | Audit found no default tuning bundle to remove, so for Fabric with empty custom text this is the same treatment as A0. Use it as a blinded A/A noise/control repetition, not an independent optimization |
| B0 explicit-bound Java 21 reference | `-Xms512m -Xmx2048m`, otherwise Java defaults plus required Minecraft arguments | Explicit lower/upper-bound reference. **Different initial-heap policy from A0**; A0 must not be relabeled as setting Xms. Cannot be expressed through current Settings; laboratory-only and deferred until an isolated native test path is reviewed |
| H1/H2 maximum heap | A0 with 1536 / 3072 MiB, respectively, same ergonomic initial heap | Test whether less footprint or reduced pressure improves tails. Screen 1536 first on A; do not expect demanding workloads to fit. If 2048 fails C, retain failure and separately test 3072/4096, not a hidden baseline change |
| I1 initial heap | B0 versus `-Xms256m -Xmx2048m`, same maximum | Test commitment/growth/startup trade-off. Deferred laboratory path; no equal-to-Xmx or large initial heap assumed beneficial |
| G1-100 / G1-50 | A0 plus `-XX:MaxGCPauseMillis=100` / `50`, only if observed collector is G1 | Soft pause-goal experiment: shorter pauses may trade throughput/frequency/CPU. Compare against the **measured** runtime default target, not an assumed flag list |
| Z1 conditional | Explicit supported ZGC mode on the exact Java 21 build, equivalent heap, one collector only | Only after a baseline shows GC-correlated tails and adequate headroom. Verify generational/non-generational mode support and record it. Concurrent CPU/footprint/startup costs may outweigh pause gains. Current G1 text parser cannot analyze ZGC; validated JFR/collector-specific parsing is required first |

Java 21 G1 guidance favors starting with defaults and avoids freezing young-generation
sizes; pause targets are goals, not frame deadlines. This supports controlled heap/
pause-target experiments, not popular flag collections. [Oracle G1 tuning](https://docs.oracle.com/en/java/javase/21/gctuning/garbage-first-garbage-collector-tuning.html).
No forced young generation, worker counts, region sizes, experimental G1/compiler
flags, large Xms, large pages, pre-touch, disabled explicit GC or mod-removal trial
is included. Serial/Parallel/Shenandoah have no established Minecraft hypothesis
here; do not broaden the matrix without evidence and exact build support.

## Measurement channels

| Metric | Capture and interpretation |
| --- | --- |
| Frame times | Actual presented/displayed intervals if supported, or a separately labeled validated in-game timing stream. Mean, median, P95/P99, P99.9 only at >=10,000 samples, population variance in ms², max, sustained FPS = 1000 / mean interval. Report frame counts and tail sample depth |
| Stutter | Predeclared >50 ms threshold. Count affected frames and runs of consecutive affected frames as separate event counts; events/minute. Preserve outliers. This is an operational threshold, not universal perceptual truth |
| GC | Confirm actual collector; collection frequency, individual stop-the-world pauses and P95/P99/max, total pause ms and wall-time percentage, full GC and failures. Distinguish cycle IDs, subphase pauses and concurrent activity |
| Heap | JFR/jcmd where available: sampled used/committed heap, live-set/after-GC occupancy, maximum. G1 summary `before->after(capacity)` is a coarse GC observation, not a continuous heap/RSS series |
| Allocation | Document sampled JFR allocation estimates versus exact counters; capture event/config/version and weighting. Not inferred from heap occupancy deltas |
| Process/system memory | Windows working set/private committed/peak working set and system available/commit pressure; Linux RSS/high-water/private mappings, MemAvailable, swap/paging counters. Heap, resident memory, private commit and VRAM are distinct. Report OS-reported peaks separately from sampled maxima |
| CPU | Exact-child CPU-time deltas over wall time: one-core percentage, 100 = one full core, may exceed 100. Report mean/max and sampling period. Also system/background contention and, when available, attributable GC CPU. GC pause wall percent is not GC CPU percent |
| Startup | Separate markers for JVM entry/VM initialization, Minecraft initialization, native spawn-to-main-menu and world-load start-to-playable. Document each marker and clock; launcher preparation/Java diagnostic are separate costs. Unsupported markers remain null |

For Windows, investigate an owner-approved pinned **PresentMon** console build.
Its documentation distinguishes `DisplayedTime` (including NA for undisplayed
frames) from CPU and Present API timing. Confirm the exact Minecraft OpenGL path,
swapchain/window and display mode in a short calibration before selecting a field.
Do not assume an ETW Present timestamp equals a displayed frame interval; no capture
support was verified here. A pilot must reconcile displayed counts with elapsed time
and VSync/cap changes and report missing/dropped events. Preserve raw CSV and tool
version/command options. Never turn NA into zero or silently drop stutters.
[PresentMon metric definitions](https://github.com/GameTechDev/PresentMon/blob/main/README-ConsoleApplication.md).

For Linux, evaluate a pinned, approved OpenGL-compatible capture such as MangoHud
only in a separate session; validate its actual metric semantics against its release
documentation before use. Swap/render timing is not automatically display timing.
Wayland/X11/compositor/driver and process accounting differ. An in-game frame-timing
integration is acceptable if its boundary/overhead are documented and labeled CPU/
render timing rather than displayed smoothness. No Linux capture was validated.

The existing launcher player/browser probes are excluded. They do not measure
Minecraft. Native supervisor events identify the child/PID; they do not capture
frames or define a main-menu clock. A compatible authenticated bridge can supply
main-menu/world markers, but its delivery latency must be measured and recorded;
it is not present for every mod set (A lacks Aurora). Otherwise use a documented
external visual marker with error bounds, or mark the startup component unmeasured.

Optional benchmark-only unified logging example, one **argument** in disposable
instance additional text (relative output stays under that instance cwd):

```text
-Xlog:gc*,safepoint:file=benchmark/gc.log:time,uptime,level,tags:filecount=0
```

Create the test-owned `benchmark/` directory first. No log rotation during a bounded
trial; monitor disk space. Keep collector startup header and uptime decorators.
`filecount=0` does not make retention bounded; use finite trial duration and a storage
budget, and abort/preserve a run if that budget is exceeded. Never enable this for
ordinary users. Exact syntax/support is subject to the laboratory validation step.
[Java 21 command/logging reference](https://docs.oracle.com/en/java/javase/21/docs/specs/man/java.html).

Optional JFR: bounded recording with reviewed settings/size/duration, using the same
profile for every comparison. Verify the managed runtime contains recorder modules
and whether jcmd/jfr utilities are present; Mojang's runtime is not assumed to be a
full JDK. Parse/export with a separately approved matching utility if required.
Do not put application CLI/account data in exported events. Whole recordings can
contain private paths/system properties/thread names; keep raw recordings local
and sanitize exports before commit. No attach to owner processes.
[JFR export reference](https://docs.oracle.com/en/java/javase/21/docs/specs/man/jfr.html).

Record OOM/crash/unsupported flags from sanitized exit diagnostics and status, not
absence of an OOM string in a GC log. Safepoints that are not GC are a separate
channel. The included parser deliberately handles only HotSpot G1 summary pauses;
it does not claim to parse JFR, every vendor VM, concurrent ZGC or safepoint time.

## Trial order and statistics

1. Freeze workload lock and runtime/tool identities. Run an excluded calibration
   and instrument-on/off A/A pilot to quantify capture overhead/noise. Choose one
   600-second analysis window with at least 300 seconds of repeatable gameplay
   warm-up; warm-up is in-world, not time spent on the menu. Extend it by a
   predeclared rule if JIT/chunk loading has not stabilized, consistently for the cohort.
2. Restart Java for each trial. Reset the disposable snapshot, repeat startup
   markers and route, then warm up and measure. Record monotonic clocks, clock
   mapping to GC uptime, exact analysis window, and capture completeness.
3. At least five **usable** runs per candidate/workload where practical, in paired
   blocks. For three treatments use orders A/B/C, B/C/A, C/A/B, C/B/A, A/C/B;
   then extend with B/A/C to balance positions. Generate and record the order
   before observing results. A/A controls can use blinded labels in separate blocks.
4. Cool down consistently between runs; record duration and thermal observations.
   Do not pause logging or skip a bad segment. Interrupted, crashed, unsupported
   and OOM runs stay in the inventory with reasons and denominator; replacement
   repetitions are appended, never overwrite failed evidence.
5. Compute metrics per run, then median and SD/range across runs. Pair by block,
   compute each candidate/base relative change, median change and exploratory
   run-level paired bootstrap interval. Do not pool all frames as independent
   repeated trials; temporal autocorrelation makes that misleading. Do not pool
   hardware/workloads/OS/collector modes. Avoid selecting the best of many flags
   without a held-out confirmation series.

The offline tool uses type-7 interpolated quantiles and a deterministic 2000-resample
paired median bootstrap, seed 20261016. Intervals require >=5 matched runs; five
does not guarantee significance or trustworthy tail inference. Narrow apparent
intervals from repeated identical fixtures say nothing about real uncertainty.
For zero-baseline metrics, percentage change is undefined: compare absolute changes
and failure counts manually. Startup and gameplay remain distinct tables.

Pre-register P99 frame time as primary, mean/P95/P99.9 and stutter as supporting;
memory, CPU, startup, failures and compatibility are guardrails. Before tuning,
use A/A data to set a minimum useful effect and maximum tolerated regressions.
There is **no fixed numeric win threshold justified yet**. If intervals overlap
zero, the effect is smaller than A/A noise, or a material guardrail worsens, report
inconclusive/trade-off rather than a winner. Repeat on independent scenes/hardware
before proposing defaults. A higher average FPS cannot compensate automatically
for substantially worse P99 or paging/OOM.

## Hardware coverage

| Class | Planned coverage | Current evidence |
| --- | --- | --- |
| 8 GiB | Low-core CPU, iGPU/shared RAM; all workloads including honest failures | Unavailable; unmeasured |
| 16 GiB | Mainstream CPU; discrete and integrated GPU variants | Unavailable; unmeasured |
| >=32 GiB | Hybrid/high-core desktop and a different CPU architecture/topology | Current 32 GiB i7-14700F / RX 9070 XT inventory only; no gameplay |
| Laptop | AC/battery and thermal/power-limited behavior as separate cohorts, no system settings changes | Unavailable; unmeasured |
| Linux x64 | Equivalent pinned workload with validated runtime/capture and documented compositor | Untested; authentication credential store is not implemented on Linux, an additional real-launch blocker |

No architecture fallback, assumed GPU type, remote fabricated result or cross-platform
equivalence. OS accounting is reported in its native semantics before comparison.
Native Java memory, launcher/background load and iGPU shared pressure require explicit
headroom. CPU topology motivates strata and explanations, not forced worker counts.

## Offline analysis and raw data contract

New tooling is isolated in [jvm-benchmark/](jvm-benchmark/README.md). It uses Python
3.12's standard library, performs no capture/launch/network/install and writes JSON
to stdout. Tests use uniquely created temporary roots and remove only their own
fixture files. Existing startup study scripts/evidence remain unchanged.

```powershell
python -B -m unittest discover -s docs/performance-p1/jvm-benchmark -p test_analyze.py -v
python -B docs/performance-p1/jvm-benchmark/analyze.py docs/performance-p1/jvm-benchmark/fixtures/run.json
# Future real manifests, explicitly listed after capture/quality review:
python -B docs/performance-p1/jvm-benchmark/analyze.py <run1.json> <run2.json> --baseline current
```

Use `sampleKind=measured` only for actual game captures; synthetic fixtures are
`synthetic`. Keep private raw captures in test-owned storage, immutable and hashed;
retain sanitized normalized inputs and manifest alongside summaries. Each run has
benchmark/config/block/workload IDs, timezone timestamp, complete/failed/interrupted
status, full conditions/runtime/JVM-only arguments, measurement methods, warm-up,
duration, GC-uptime window, raw relative sources, and separate optional startup times.
Record raw producer hashes, repository HEAD, capture tool/version, mod/world/config
hashes and deviations. The tool hashes the manifest and consumed normalized files;
upstream raw-to-normalized provenance must be recorded by the operator.

The tool does **not** validate that a capture method truly measures display or that
claimed hardware/runtime facts are authentic. Adapter/calibration review remains
mandatory. It refuses incompatible comparison cohorts, duplicate config/block runs
and inconsistent arguments within a config. It excludes failed/interrupted runs from
metrics without deleting them, preserves null unavailable metrics, reports aggregate
frame/GC/memory/CPU/startup changes and never declares a tuning winner.

Before sharing, redact account UUID/name/email, tokens, game arguments, endpoints/
private server names, bridge capabilities, absolute user paths and unrelated process
identities. Never read process command lines to create a manifest. Avoid benchmark
world names containing private text. Raw recordings remain local until reviewed;
automated exact-token redaction is not a complete privacy review.

Once real captures exist, produce workload/hardware-labeled frame percentile plots,
GC pause ECDF/histograms (small samples labeled), heap/working-set and CPU timelines,
and separate startup box/point plots. Include units, n, failures, uncertainty and
linked raw hashes. Do not generate synthetic gameplay comparison charts in this P1.

## Current limits and exit gate

No capture adapter, test-world/route assets, exact optimization-mod lock, validated
instrumentation support, alternate-initial-heap laboratory launch path or real
gameplay measurements are delivered here. Those are explicit execution prerequisites;
the reusable deliverable now is audit, protocol, offline parsers/statistics and tested
synthetic formats. Full preset design remains subject to measurements and owner review;
see [P2_DIRECTION.md](P2_DIRECTION.md). Stop after P1.
