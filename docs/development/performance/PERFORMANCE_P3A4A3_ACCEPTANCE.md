# Performance P3-A.4a.3 acceptance

**PARTIAL. Recording workflow validated; practical resource-comparison readiness
remains inconclusive. P3-B NOT READY / NOT AUTHORIZED.** No JVM candidate, product
recommendation, production installation or performance improvement was implemented.

The new [Practical Performance Protocol](../../performance-p3a/PRACTICAL_PROTOCOL.md)
is distinct from the preserved Strict Research Protocol. Five real Overlay CSVs,
three real JFR exports and three coherent warmed-up stationary baseline windows
replace the previous recording unknowns. The remaining issue is observed CPU/RSS
drift and unresolved observer effects in the metrics the proposed heap experiment
would evaluate. Optical timing, perfect HUD completeness and precise GC/frame
correlation are **not** reimposed as practical prerequisites.

## Repository and preservation

Branch `codex/client-3-production-authority`; starting full HEAD
`79a55680d23e97985913d6b7955f38862c6957ab`, the one local P3-A.4a.2 commit after
`133cdd7e9988c5e8cc70b45afa7535d04b9c750f`. Initial tracked tree/index clean;
2552 tracked paths, four protected developer-local files, seven worktrees. Full
status/index/history/file hashes, critical manifests/build files, prior evidence,
production identities, canonical materials and separate Client state were captured
before mutation in `.git/performance-p3a4a3-baseline.json`.

One scoped local commit contains this report: **Validate practical Overlay and JFR
recordings and prepare first heap experiment**. Ending HEAD is that report commit;
its full content-dependent SHA is recorded after commit in
`.git/performance-p3a4a3-final.json` and the final response. No push, merge, tag,
release, deployment, workflow dispatch, version bump or production installation.

[Complete changed paths](../../performance-p3a/P3A4A3_CHANGED_FILES.txt),
[preservation/deletion audit](../../performance-p3a/evidence/p3a4a3/preservation.json).
Only performance guides, focused offline checks/tests and new reports/sanitized
evidence change: **22 paths, 16 additions and six modifications**. All original
tracked paths remain present; **zero deletions or
renames**. All four protected local files, 932 retained prior private evidence files,
11 critical files, seven worktrees and the six other worktree HEADs are preserved.
Separate Aurora Client remains at `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d`:
all 422 tracked files plus HEAD/index/status/worktree state unchanged.

Production Launcher 1.4.2 executable/config/registry hashes match baseline. This
limited hash scope is not an owner-world, credential or full runtime inventory.
Actions never target owner instances, worlds, mods/packs, Java or release artifacts.
No `.minecraft`, cleanup/reset, recursive removal, process-name termination,
global power/display/security change or secret export. Normal supported same-user
refresh retains the previously accepted shared credential-rotation risk; no vault
inspection, account copying or production sign-in probe.

Canonical A archive `d385120835fd6b0ad0fb8757601de97b066d9e23023e32733f62e6bd56e13f4f`,
inventory `5082795f0ac5987c1eb61412102a2be5bc7a29868842dd317f985f3193258c61` and options
`efd1595619f579723159ed7bb0cbb1e06d7336dc9980662252162abd15f24ae1` remain unchanged.

## Actual tools, workload and isolation

| Tool / component | Exact identity |
| --- | --- |
| Python / Node / Git | 3.12.10 / 24.21.0 / 2.55.0.windows.3 |
| Isolated launcher | Existing reviewed native binary, SHA-256 `05b97f5be5517972863e2460738e36c1a92257853ec9f72809c610c81114e09a` |
| Java / jcmd / jfr | Managed Microsoft-11369940 OpenJDK **21.0.7+6-LTS**, windows-x64, manifest `cb4394a27089d19f65d5baa6cf0482c27c3c7865` |
| jcmd / jfr SHA-256 | `0522de8be9e6928f98aabde45df0e035d947a9394adead00eb3068ba3bff7fba` / `7995363b06961f6049b4d273046d295ea63adb4eb2919c4dd2e783cc1d5e13f4` |
| Performance Overlay | 1.0.2 / qyzs7zWa, SHA-256 `46c9500f67824996cd81b00a4d3da2dc09d5c6d56495cff98276c961d74804c2` |
| Cloth Config | 21.11.153+fabric / xuX40TN5; prior audited expected publisher digests, installed via native verified store |
| Minecraft / Fabric | 1.21.11 / exact Loader 0.19.5 |
| Aurora / API / Sodium / Lithium | 3.0.0 / 0.141.6+1.21.11 / 0.8.14+mc1.21.11 / 0.21.4+mc1.21.11 |
| JFR profile | Same conservative privacy-filtered default; SHA-256 `d48adb550887e5e4e0fd8c3aedbd05e5c234aef17ec5812bb06310a290af5a80` |

Built-in Windows system/process CPU, memory, power, window geometry and GPU counters
provide context; existing bundled Playwright controls the isolated native command
boundary and the installed computer-use helper operates the game UI. No new tool
download, dependency, renderer hook, custom mod or fork. Exact six JAR hashes and
native runtime validation (402 files / 99,041,788 bytes, reused Ready) are in
[identity.json](../../performance-p3a/evidence/p3a4a3/identity.json).

Filesystem ancestor/descendant reparse checks, marked direct-temp root, exact
launcher/Java hashes, native-reported root and separate live WebView profile pass.
The approved startup-refresh sequencing exception remains explicit. Fresh native
instance creation, exact provider previews/verified installs, ordinary Rust session
refresh, entitlement/readiness and Play were used. No direct game launch or live
argument/token export. Both game launches and the retained-handle launcher exit 0.

One setup error is retained: a pre-launch assertion compared old **pre-launch**
configuration hashes with the previous disposable instance's normally saved
post-exit files. The shell then launched despite the assertion. That title-only
launch was excluded, exited normally and its generated files/logs were preserved
separately. Preparation then explicitly verified actual post-exit limiter settings,
recorded their new observed hashes and restored the fresh test copy. Subsequent
dependent operations stop on a failed prerequisite. No source evidence was altered.
A legacy-shell script-policy refusal and an early exit-state query during normal
shutdown were also non-capture failures; policy was not changed, exit later confirmed.

Workload is **B stack on canonical A scene**, `workload-b-a-scene-diagnostic-v1`.
It is not minimal Fabric A and not canonical normal-terrain B. A's source archive/
options were copied into the fresh disposable instance with full pre-launch
inventory equality. A itself was not modified. Post-exit full region inspection
confirms camera `(0.5,66,0.5)`, rotation `(0,10)`, seed123456789, frozen clear noon,
zero entities/block entities, 5329 chunks and all 1024 required chunks full.
[Scene/settings evidence](../../performance-p3a/evidence/p3a4a3/scene-after-exit.json).

Saved native Unlimited 260 sentinel, VSync OFF, inactivity Minimized, Aurora cap
OFF, adaptive=false, PARK/500us spin/2000us park and distances12/8 are preserved.
Actual physical client is 1920x1080 at120DPI, windowed; framebuffer unobserved.
RX9070XT driver32.0.31041.1004, desktop2560x1440/240Hz and Ultimate Performance
scheme unchanged. AC is present; minimum available RAM is approximately17.57 GiB.
A single GPU-engine sample is context, not continuous GPU equivalence or a whole-GPU
percentage (engines can overlap). VRR/thermals/effective frequency/paging unknown.

## Environmental model corrected

The old quiet scripts opened their own sampler PID but gated **global**
GetSystemTimes deltas: `(kernel+user-idle)/(kernel+user)`, summed over all28 logical
processors. This includes Minecraft, launcher, observers and background/OS work;
it measures a share of whole-machine capacity, not per-core load. Half-second
low-cost process counters are quantized/noisy; window means use weighted endpoint
deltas. Logical threads on this hybrid CPU do not have equal performance capacity.
[Microsoft counter contract](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes).

The 2% screen was described as a quiet/background condition but applied to active
title-screen total CPU. Therefore the old **2.979357%** is not proof of unrelated
interference. Preserve its historical strict failure; no retroactive valid capture.

New read-only handles track exact game/launcher/sampler PID+creation identities.
Residual subtracts these measured process deltas from total; it still includes
WebView, attach tools, OS work and sequential-read uncertainty. It is not certified
unrelated CPU. Launcher/sampler zeros at counter resolution do not mean zero cost.
No arbitrary higher total-CPU limit replaces the strict gate. Stable background
activity is tolerable; unexplained resource changes make comparisons inconclusive.

## Overlay recording and three-run baseline

One30-second pilot, then three30-second Overlay-only windows after a verified
>=300-second stationary in-world warm-up; one combined30-second check. All five
recordings started with F10 while the overlay was hidden and stopped automatically
with `AUTO_DURATION`. Manual early-stop behavior was not separately tested.
All exports contain successful footers, matching row/summary counts, valid finite
numeric columns, positive plausible intervals and nondecreasing timestamps.
No reported writer overflow/abort. Producer Avg/Low1/Low0.1/max/stutter summaries
reproduce within propagated 0.001-ms interval and producer display precision.
The 30-second timers export approximately30.040–30.050seconds of intervals because
automatic stop is checked on the client tick; actual durations are retained below.
[Full reviews/hashes](../../performance-p3a/evidence/p3a4a3/frames.json).

| Actual window | Rows | Summed intervals s | Median ms | P95 ms | P99 ms | Max ms | Internal rate Hz | >50 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Pilot; not warmed baseline | 58276 | 30.040202 | 0.486 | 0.958 | 1.346 | 7.114 | 1939.934 | 0 |
| Baseline1 | 54375 | 30.046918 | 0.502 | 1.019 | 1.372 | 6.563 | 1809.670 | 0 |
| Baseline2 | 55661 | 30.049729 | 0.497 | 0.972 | 1.3064 | 5.928 | 1852.296 | 0 |
| Baseline3 | 54763 | 30.050394 | 0.502 | 0.993 | 1.32338 | 5.404 | 1822.372 | 0 |
| Overlay+JFR | 55136 | 30.050333 | 0.500 | 0.992 | 1.31865 | 6.080 | 1834.788 | 0 |

These are **eligible HUD-callback intervals**, useful internal frame-loop
diagnostics. The rate is count/summed intervals, not mean instantaneous FPS or
physical display FPS. There are25,029–26,191 tied millisecond timestamps and9–13
indistinguishable adjacent rows in the baseline exports; quantization permits them.
No duplicate removal, fabricated missing frames, gap-to-stutter conversion or
perfect rendered-frame completeness claim. Zero >50ms samples does not prove
zero physical stuttering or missed HUD callbacks. Warm-up/pilot are not pooled.

Observed baseline range/median: throughput **2.339%**, median **0.996%**, P95 **4.733%**,
P99 **4.957%**, maximum **19.551%**. Three windows in one process characterize
within-session variation only, not restart/day/hardware variation or a confidence
interval. Coarse pacing changes could be screened here; small CPU/heap changes
cannot be accepted from these results. No generalization beyond this stationary scene.
[Repeatability](../../performance-p3a/evidence/p3a4a3/repeatability.json).

## JFR actual recording/export

Named disk recordings used the isolated repository/output, conservative selected
JFC, duration45s safety stop, maxage60s/maxsize32MiB, explicit normal stop after a
30-second sampler window. JFR-only validation preceded combined recording. Installed
`jfr summary` and selected `jfr print --json` both read every exported recording.
Each has one chunk; actual31/34-second rounded recording coverage includes command/
pre-roll time and is not relabeled as an exactly aligned30-second frame window.
ActiveRecording/ActiveSetting and retention/loss were inspected. No reported
DataLoss events; absence does not prove exhaustive event eligibility or sampling.
All five privacy-excluded event types have zero recorded events.

| Recording | Bytes | Rounded s | GC collections | Top-level pauses | Pause sum ms | Pause P95 ms | Max pause ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| JFR-only pilot during warm-up | 3809713 | 31 | 33 | 42 | 90.6961 | 6.7880 | 12.7341 |
| Warmed JFR, Overlay inactive | 3637357 | 31 | 36 | 48 | 72.7948 | 3.528825 | 3.8811 |
| Combined | 4625557 | 34 | 48 | 64 | 96.6645 | 3.508075 | 5.4945 |

GC/heap/allocation/execution events exist. G1New/G1Old collections include concurrent
cycles; collections and stop-the-world pauses are different counts. Pause totals
use only `jdk.GCPhasePause`, excluding nested phases. Heap used after GC is about
333.6–348.8MiB across recordings; committed heap579MiB, reserved2048MiB. Allocation
and method events are samples, not exhaustive counters. The first CPU-load event
has initialization context inconsistent with concurrent OS-window averages;
GetProcessTimes/GetSystemTimes remain the CPU comparison source. No precise
frame-spike/GC causality or shared timebase is claimed.
[Event coverage, commands, hashes and aggregates](../../performance-p3a/evidence/p3a4a3/jfr.json).

## Observer review and remaining practical gates

| Warmed30-second state | Total machine CPU % | Game CPU % | Residual CPU % |
| --- | ---: | ---: | ---: |
| Inactive before | 8.874463 | 5.057570 | 3.816893 |
| Overlay1 | 8.789633 | 5.220532 | 3.569101 |
| Overlay2 | 8.424479 | 5.355283 | 3.069196 |
| Overlay3 | 9.411393 | 5.746549 | 3.664844 |
| JFR with inactive Overlay | 9.324178 | 5.470582 | 3.853597 |
| Combined | 9.785545 | 6.006968 | 3.778577 |
| Inactive after | 9.669315 | 6.064723 | 3.569235 |

Every warmed window has61/61 foreground observations matching the exact game.
The last control includes one computer-use snapshot about10seconds into the
window; other boundary snapshots occur outside/before sampler windows. Sampling/
snapshot/attach tools are part of the uncertainty, not claimed cost-free.
[All environmental series summaries](../../performance-p3a/evidence/p3a4a3/environment.json).

No crashes, export errors or substantial additional pacing disturbance were seen
in the combined check. Combined pacing is inside baseline variation. However,
inactive game CPU rises **1.007153 machine percentage points (~19.9% relative)**;
RSS also grows from roughly1.48GB to1.61GB over the control sequence. That drift
changes even the sign of active-minus-control resource differences. Combined
versus warmed JFR-only GC frequency is about1.412 versus1.161 collections/s;
pause rates2.843 versus2.348ms/s (rounded-duration normalization). These windows
cannot isolate recorder allocation/GC cost from session drift. No overhead is
subtracted, no arbitrary acceptance percentage invented, no negligible-cost claim.
No-mod overhead and independent uninstrumented frame timing remain unknown.

**Practical resource-overhead decision: INCONCLUSIVE**, not a renewed rejection
for failing strict1%/0.05ms/1pp research budgets. Actual resource variation could
obscure the proposed heap experiment's outcome. A representative normal-terrain B
identity and equivalent fresh-process baseline are also still absent. Minimal A
materials stay intact; no new minimal-A run is claimed.

The [first experiment](../../performance-p3a/FIRST_JVM_EXPERIMENT.md) is prepared:
**Xmx2048m→Xmx1536m only**, default G1, empty custom text, same observers/settings,
three counterbalanced pairs, explicit memory-headroom/pacing/stability/regression
checks and native reversal to2048MiB. It tests possible constrained heap growth,
not assumed higher FPS; current579MiB commitment may mean no benefit. No candidate
has run and Efficient/Balanced/Performance advice is unchanged.

GateA export usability passes; GateA resource observer acceptance is inconclusive.
GateB within-session pacing repeatability passes with limits; resource/restart/
representative-workload sensitivity is incomplete. GateC design is prepared;
GateD is not run. **P3-B is not ready or authorized.**

Smallest next action: one frozen representative-workload, counterbalanced
inactive/recording/JFR resource-control set with equal fresh-process warm-up,
using the same existing tools. Resolve the observed drift before interpreting
a heap benefit; if still inconclusive, defer that claim. No new benchmark mod,
instrumentation framework, optical timing project or indefinite retries.

## Verification and evidence retention

Before changes: **97 benchmark +13 P1 tests pass**. After changes: **113 benchmark
+13 P1 tests pass**, zero failures. Sixteen focused synthetic arithmetic/integrity
tests cover CPU subtraction/weighting/negative uncertainty, missing/reversed/
impossible counters, memory pressure, format/source/window/overflow integrity,
nonfinite columns/footer, producer-summary/stutter disagreement and run variation.
Historical tests/strict importer remain unchanged. An initial shallow P1 discovery
found zero tests; the correct documented directory ran all13. Existing unrelated
fixture ResourceWarning remains non-failing. [Test records](../../performance-p3a/evidence/p3a4a3/tests.json).

No product Rust/Svelte/TypeScript, startup/native contract, dependency/lockfile,
schema/preset or renderer/limiter changes; broad product build suites were not
needed. Actual isolated native boot, authenticated Play, recording, save/exit and
managed Java diagnostics were exercised. Real measurements above are separate
from synthetic tests, estimates and the unexecuted candidate hypothesis.

Raw CSVs remain in the new disposable instance's
`config/performanceoverlay/benchmarks`; raw JFR/repository/telemetry/outputs remain
under the new `p3a4a3-artifacts` sibling of the established marked test root.
Private exact paths, operation scripts and logs remain in `.git/p3a4a3`.
[Raw byte inventory](../../performance-p3a/evidence/p3a4a3/raw-inventory.json)
and [committed evidence inventory](../../performance-p3a/P3A4A3_EVIDENCE_INVENTORY.json)
preserve hashes. No raw JFR, account-bearing log, world, game/runtime binary or
secret is staged. Stop after P3-A.4a.3; P3-B requires separate owner authorization.
