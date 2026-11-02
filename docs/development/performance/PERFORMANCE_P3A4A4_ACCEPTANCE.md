# P3-A.4a.4 acceptance — PARTIAL

The bounded fresh-process sequence is finished. The planned A–B–C–C–B–A matrix
has five eligible primary resource windows and one retained mixed-state C2r
resource window, with one procedural retry each for A and C. No further C retry
is authorized by this phase's budget. Four coherent frame exports remain usable.
The normal-terrain stationary workload is locked. Overlay and JFR produce readable
evidence; metric readiness is partial. No JVM candidate or production change was
executed. **P3-B remains NOT AUTHORIZED.**

## Repository baseline and preservation

Repository `C:\Dev\aurora-launcher`, branch `codex/client-3-production-authority`.
Starting full HEAD: `bcda4b16f23480b4bd1d4c407ec9e2d66dcd9b3a`.
The ending full HEAD and scoped commit are recorded in the completion message and
private `.git/performance-p3a4a4-final.json`; a commit cannot contain its own hash.
The initial tracked tree and index were clean, with 2568 tracked files and the four
known developer-local untracked files. Baseline inventory, index, history, critical
hashes, canonical identity, previous evidence and worktree state were recorded
before changes in `.git/performance-p3a4a4-baseline.json`.

Raw CSV/JFR/environment series, commands, logs, frozen/restored worlds and failed
attempts are retained under the isolated `p3a4a4-artifacts` directory and private
`.git/p3a4a4`. The [anonymous byte/hash inventory](../../performance-p3a/evidence/p3a4a4/raw-inventory.json)
has an exact local path map; account-bearing filenames/raw logs/worlds are not
staged. File hashes detect later changes, not owner-enforced immutability.

[Preservation audit](../../performance-p3a/evidence/p3a4a4/preservation.json): PASS.
All original tracked files remain; no deletion or rename. All four protected local
files retain their exact bytes/hashes. Seven worktrees remain and the other six
HEADs are unchanged. All 1063 inventoried previous private evidence files and 11
critical files match. Canonical A and prior reports/raw captures remain unchanged.
Separate Aurora Client HEAD `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d` and its 422
tracked files/index/status/worktrees are unchanged. The complete changed-file list
is [here](../../performance-p3a/P3A4A4_CHANGED_FILES.txt).

Production Launcher 1.4.2 executable/configuration/registry hashes match. This is a
bounded production identity audit, not a full owner-world/credential/runtime hash
audit. All game/world/config/runtime/capture actions target the established
disposable managed root and separate WebView profile. No owner instance, normal
`.minecraft`, Client source, release artifact, workflow or production default was
targeted. Normal native refresh, entitlement, readiness and supervised Play were
used. No password, secret copy, token inspection/export or live command-line
export. The previously accepted same-user UUID-key Credential Manager refresh
rotation risk still applies; token-byte preservation is not claimed.

## Representative workload and material verification

[Exact workload/settings/inventory](../../performance-p3a/evidence/p3a4a4/workload.json).
Minecraft 1.21.11 / Fabric 0.19.5, Microsoft OpenJDK 21.0.7+6-LTS; normal noise Overworld,
seed 123456789, structures on, bonus chest off. Forest/plains/water view at
(128.5,96,128.5), yaw 135 / pitch 15, creative flying, normal difficulty. Time 6000
and clear weather frozen; natural mob spawning off, random tick speed 3. No server,
movement or intended generation during captures. Required 1024 full chunks cover
[-8,-8] through [23,23]; all exist. Persisted recorded chunks remain 3772 after runs,
with no observed new recorded chunks. This supports pre-generation, not a live
loaded-chunk count or proof against every transient generation task.

The frozen world contains 209 persisted entities and 446 block entities across its
whole recorded area. Those are not loaded counts. Actual live boundary F3 entity
counts and post-save inventories are in resources.json; loaded chunks and loaded
block-entity counts remain UNKNOWN. F3 renderer section counts are not chunk
counts. Active entities settle differently after equivalent restores, an explicit
ordinary-workload variability source. No entity cleanup was used to hide it.

Archive SHA-256 `de973b21dc49c5ea2a71d60f452b1c8f28e8383264892f021b38f5feb07892f6`
(23992320 bytes); inventory SHA-256
`48ec9f18fdc6cde92d047e979d7c77b402b692fc959e164733959874a8107c95` (42 files / 12 directories).
Options SHA-256 `64cec45e976849b23c95fc3c0b7652d72c535167903b3716fef7e93820b6cf65`.
Every process starts from byte-equal world/config/options inventories, at the same
fixed disposable save path. Previous phase-created worlds/configs are retained by
exact moves to new phase-owned paths, without deletion. Live camera is reset and
F3-verified after loading, before warm-up, then checked after capture and save.

Physical client 1920×1080, windowed, 120 DPI; Custom graphics, clouds off, particles
All, AO on, mipmap 4, render/simulation 12/8, FOV 70 / GUI 2, no resource/shader packs.
Native Unlimited (stored 260 sentinel), VSync off, inactivity Minimized, Aurora
custom cap 0 / adaptive false/PARK. Same Existing configuration: memory 2048 MiB,
default G1, empty custom JVM text, null window override. No 1536 run.

| Material | Bytes | SHA-256 |
| --- | --- | --- |
| aurora-3.0.0.jar | 3077377 | `43f918207a86f91b01b35045309e89d2b040951722e5ac71d01d886beec9811c` |
| cloth-config-21.11.153-fabric.jar | 1148427 | `8a40c84e5ecde525acfb6c4e13b8002dacfca3ef9534d7faf6e8049656f423c8` |
| fabric-api-0.141.6+1.21.11.jar | 2426039 | `bdff7fd7e220085cfad2ff9b1f40dde6534ae0b96cf378f97a374bc54cb9ed0f` |
| lithium-fabric-0.21.4+mc1.21.11.jar | 900462 | `5135c41da5b43cbdcb29424bde65195143ac4084e23834c8eac065942201c78b` |
| performanceoverlay-1.0.2+mc1.21.9-1.21.11-fabric.jar | 188784 | `46c9500f67824996cd81b00a4d3da2dc09d5c6d56495cff98276c961d74804c2` |
| sodium-fabric-0.8.14+mc1.21.11.jar | 1909150 | `fd2619eff5da6b9ba6304b8b72ac3fe132c30e0bd5d675e4581eda417a272f5d` |

Overlay/Cloth are diagnostic dependencies only. Managed runtime identity is
`java-runtime-delta/windows-x64-cb4394a27089d19f65d5baa6cf0482c27c3c7865`,
402 native-verified files, 99041788 bytes; Java executable SHA-256
`1ba6a4fb32a6a3d5c925fe837ae6717e2f75743a71e013e668e6dadb909245ba`.
This is a controlled normal-terrain **stationary subset**, not traversal, loading,
server or memory-pressure coverage. It is not automatically promoted to strict
canonical Workload B. Canonical A is preserved.

## Trial matrix, identity and warm-up equivalence

The [predeclared plan and transparent amendments](../../performance-p3a/P3A4A4_TRIAL_PLAN.md)
use two observations per condition: A Overlay installed/inactive/noJFR;
B Overlay recording/noJFR; C Overlay+JFR, all with the same external sampler.
Six unique matrix-slot creation identities and exact executable checks prevent PID reuse or
process-name ambiguity. Each normal native launch refreshes/validates the same
isolated configuration and session. All games exited normally with 0. The isolated
launcher PID 5796 (creation FILETIME 134366909230654288, binary SHA-256
`05b97f5be5517972863e2460738e36c1a92257853ec9f72809c610c81114e09a`)
was closed normally after the final game, with retained-handle exit 0.

| Run / condition | PID / creation FILETIME | Warm-up UTC (seconds) | Primary UTC (seconds) | Setup gap s |
| --- | --- | --- | --- | --- |
| A1R / A | 12480 / 134366919215745703 | 06:26:19.334723Z–06:31:19.340335Z (300.000001) | 06:31:27.703467Z–06:31:57.705232Z (29.999985) | 8.363 |
| B1 / B | 23168 / 134366923935211236 | 06:34:43.724745Z–06:39:43.730995Z (300.000198) | 06:39:55.959301Z–06:40:25.961065Z (30.000147) | 12.228 |
| C1 / C | 22536 / 134366929146717781 | 06:43:01.467323Z–06:48:01.473021Z (300.000040) | 06:48:12.880692Z–06:48:42.882175Z (30.000069) | 11.408 |
| C2R / C | 22756 / 134366939637498234 | 07:00:12.778129Z–07:05:12.783137Z (300.000272) | 07:05:27.016122Z–07:05:57.018532Z (30.000586) | 14.233 |
| B2 / B | 16692 / 134366944289725209 | 07:07:58.261086Z–07:12:58.266937Z (300.000009) | 07:13:16.007087Z–07:13:46.008160Z (30.000050) | 17.740 |
| A2 / A | 18192 / 134366948812796838 | 07:15:33.522358Z–07:20:33.529678Z (300.000158) | 07:20:38.636954Z–07:21:08.638705Z (30.000360) | 5.107 |

All times UTC 2026-10-17. **C2r is context only for resources**, excluded from C
resource repeatability and condition mean comparisons. Warm-ups have 601 samples
and resource windows 61 at 0.5s;
every sampled foreground identity matches the game. No input during stationary
warm-up/capture. Each uninterrupted sampled warm-up is at least 300s; the explicit
setup gap covers heap-info/JFR/key orchestration and differs as recorded. These
are equal-minimum warmed processes, not identical JVM/JIT/GC state or cold starts.
Heap-info attach occurs outside primary windows in every condition. No forced GC.
No active JFR in A/B was verified before capture. No inactive CSV was produced.
The B1/C1/B2 resource-start brackets follow the Overlay key by less than one 0.5s
sample period; the exact differences are retained, and the last edge can include
a fraction of a second after auto-stop. These resource summaries approximate the
active window, not a precisely aligned observer-cost cohort. C2r's about 5s gap is
materially different and excluded; no arbitrary performance tolerance is inferred.

[Attempt register](../../performance-p3a/evidence/p3a4a4/attempts.json): one short
construction pilot (PID 19028; creation FILETIME not retained; no accepted window),
excluded A1 (PID 21920, creation 134366913918792009, camera mismatch) and excluded C2
(PID 22680, creation 134366934723764700, rejected F10 and 10.656s warm-up overlap).
All raw outputs/failures are retained and neither invalid window enters means.
A1r/C2r are the single allowed procedural retries, not favorable-result reruns.
C2r successfully records frames/JFR, but its resource sampler starts about 5s after
the Overlay key and ends after auto-stop; this mixed-state window is INVALID for
the intended full 30s combined resource comparison. No trimming or second C retry
is used to make it pass. No further retry or candidate capture is performed.

## CPU attribution, RSS and within-window trends

[Full resource series projection](../../performance-p3a/evidence/p3a4a4/resources.json).
CPU is endpoint process/system time divided by total machine capacity (28 logical
processors on hybrid i7-14700F), not percent of one core. Residual subtracts game,
launcher and sampler from system CPU: an estimated unattributed remainder that
includes WebView/attach/OS work and sequential-read uncertainty, **not certified
background-only CPU**. Launcher/sampler 0 in these counters reflects quantization,
not a zero observer cost. Raw half-second intervals, overhead and identities remain
available; RSS is sampled working set, not private bytes/commit or a true peak.

| Run | Game / system / residual CPU % | Mean RSS MiB | RSS start→end MiB | End-warm heap committed / used MiB |
| --- | --- | --- | --- | --- |
| A1R | 4.349768 / 7.320342 / 2.970573 | 1583.373 | 1582.688→1583.594 | 750.000 / 652.288 |
| B1 | 4.559084 / 8.516622 / 3.957538 | 1550.005 | 1548.461→1551.574 | 709.000 / 670.523 |
| C1 | 5.266587 / 8.840941 / 3.574354 | 1629.237 | 1632.844→1633.453 | 742.000 / 585.138 |
| C2R | 4.908523 / 8.138108 / 3.229585 | 1579.584 | 1579.023→1579.469 | 730.000 / 604.139 |
| B2 | 4.434875 / 7.723282 / 3.288407 | 1580.579 | 1577.305→1582.219 | 745.000 / 645.508 |
| A2 | 4.654406 / 8.062927 / 3.408520 | 1594.923 | 1589.004→1599.766 | 714.000 / 542.365 |

| Run | Three 10-second game CPU % blocks | Warm-up end RSS MiB | Minimum available physical GiB |
| --- | --- | --- | --- |
| A1R | 4.515 / 4.206 / 4.329 | 1582.402 | 17.415 |
| B1 | 4.926 / 4.187 / 4.565 | 1529.406 | 17.156 |
| C1 | 5.643 / 5.120 / 5.037 | 1613.824 | 17.242 |
| C2R | 5.206 / 4.495 / 5.026 | 1559.238 | 17.627 |
| B2 | 5.005 / 3.883 / 4.426 | 1573.719 | 17.584 |
| A2 | 4.738 / 4.581 / 4.644 | 1586.332 | 17.605 |

The three blocks expose within-window variation; do not infer a stationary CPU
plateau from a single 30s average. RSS is time-weighted from left endpoint samples.
Heap use is one GC-phase-dependent observation outside capture. Heap commitment
is different from RSS, maximum reservation and post-GC live set. Warm-up series
retain growth and end state rather than assuming fresh process implies no drift.

| Condition | Game CPU observed range (pp; % of median) | Mean RSS observed range (MiB; % of median) | Committed heap range MiB |
| --- | --- | --- | --- |
| A | 0.304638; 6.767% | 11.549; 0.727% | 714.0–750.0 |
| B | 0.124209; 2.762% | 30.574; 1.953% | 709.0–745.0 |

| State minus mean control | Game CPU pp | Mean RSS MiB |
| --- | --- | --- |
| B | -0.005108 | -23.856 |

The C2r row is retained context, not an eligible C resource repeat. C has only
one eligible primary resource observation, so its repeatability/mean overhead
comparison remains INCOMPLETE. C1 is +0.764500pp game CPU and +40.089 MiB RSS
relative to the control mean, descriptively; one observation cannot establish a
repeatable combined-recorder increase, even where it exceeds the observed controls.
These A/B state means are not subtraction-based isolated overhead.
Control A2−A1r is +0.304638 machine CPU percentage points and
+11.549 MiB mean RSS. Compare the full overlapping/non-overlapping observed
ranges in resources.json; two observations do not form a confidence interval or
guarantee a detection threshold. The earlier same-process A-scene CPU drift was
+1.007153pp (5.057570→6.064723%) and RSS approximately 1.48→1.61GB. Both process
restart and scene changed here; any smaller observed drift cannot establish the
cause or a general improvement from restarts.

The Overlay CPU difference (−0.005108pp) is much smaller than control variation
(0.304638pp). Its RSS difference (−23.856 MiB) is comparable to its own 30.574 MiB
repeat range and cannot be called negative overhead. Practical small-effect claims
below a few tenths of a machine CPU percentage point or tens of MiB are unsupported
here; even larger effects require matched repeats, not a guaranteed noise cutoff.
End-warm-up RSS spans 1529.406–1613.824 MiB across eligible runs and committed heap
709–750 MiB. Control heap commitment falls 36 MiB while RSS rises 11.549 MiB;
these counters do not move together. A2 RSS grows 10.762 MiB during its 30s window,
so fresh processes do not establish a perfectly flat warmed resource state.

AC stayed connected, the Ultimate Performance scheme was unchanged, desktop
2560×1440/240Hz and RX9070XT driver 32.0.31041.1004 were recorded. Available memory
passed the existing reserve check. Continuous GPU utilization, thermals, effective
frequency, VRR and paging rate remain UNKNOWN. No universal CPU acceptance
percentage or strict research threshold was invented.

## Overlay quality and fresh-process pacing variability

All four accepted recordings pass existing raw hash/schema/row/footer/numeric/
producer-summary checks, AUTO_DURATION completion and writer-failure log review.
No reported overflow or writer failure. Eligible callbacks begin/end near 30s;
interval sums differ slightly from elapsed spans due producer semantics/rounding.

| Run | Rows | Sum intervals / elapsed span ms | Median / P95 / P99 ms | Callback rate Hz | Max ms / >50 ms |
| --- | --- | --- | --- | --- | --- |
| B1 | 43452 | 30036.829 / 30033 | 0.62300 / 1.12000 / 1.41398 | 1446.624 | 8.380 / 0 |
| C1 | 45031 | 30038.108 / 30034 | 0.60900 / 1.12050 / 1.42700 | 1499.129 | 6.979 / 0 |
| C2R | 43581 | 30038.799 / 30035 | 0.62900 / 1.11100 / 1.44840 | 1450.824 | 7.986 / 0 |
| B2 | 43952 | 30038.059 / 30034 | 0.61800 / 1.11600 / 1.38749 | 1463.210 | 7.067 / 0 |

| Condition | Throughput range / median | P99 range / median |
| --- | --- | --- |
| B | 1.140% | 1.891% |
| C | 3.275% | 1.488% |

[Raw identities and parser results](../../performance-p3a/evidence/p3a4a4/frames.json).
The >50ms definition is strict; adjacent intervals form one burst. Zero observed
events in short captures is not endurance proof. These are **eligible internal
HUD-callback intervals**, not active render time, frame submission, all rendered
frames or physical displayed FPS. Inactive A has no independent interval export;
Overlay's own timing cost cannot be bounded by its active CSV. B/C rate/P99 ranges
are workload-specific two-repeat sensitivity, not the previous A-scene within-session
2.34%/4.96% baseline, and not evidence that either observer improves performance.
The new B throughput range 1.140% is smaller than the old 2.34%; C's 3.275% is larger.
Both new P99 ranges (1.891%/1.488%) are smaller than 4.96%. Different scenes and
two versus three observations prevent attributing those differences to restart.
Large repeatable pacing effects can be screened under identical observer settings;
small changes near these ranges need an INCONCLUSIVE result, not extra favorable runs.

## JFR integrity, retention and interpretation

[JFR evidence](../../performance-p3a/evidence/p3a4a4/jfr.json). Both accepted exports
start/check/stop normally, parse with the exact managed JDK reader and contain GC,
heap/allocation/CPU/settings events. Reported DataLoss 0 in each; privacy-excluded
JVM/system-property/environment/security-property/system-process events 0. Profile
SHA-256 `d48adb550887e5e4e0fd8c3aedbd05e5c234aef17ec5812bb06310a290af5a80`.
Each repository/output is phase-isolated; disk true, maxage 60s / maxsize 32MiB,
45s safety stop. Files are below retention limits; no reported truncation/loss.

| Run | Rounded export s / bytes | Collections / approximate Hz | Top-level pauses / sum ms | Median / P95 / P99 / max pause ms | After-GC used MiB / committed MiB |
| --- | --- | --- | --- | --- | --- |
| C1 | 45 / 5558135 | 41 / 0.911 | 54 / 94.8459 | 1.76125 / 3.50634 / 3.67889 / 3.72590 | 405.059–418.552 / 742.0–742.0 |
| C2R | 39 / 4764387 | 38 / 0.974 | 51 / 98.0792 | 1.96080 / 3.79035 / 4.44840 / 4.52960 | 407.327–422.998 / 730.0–730.0 |

- C1 SHA-256 `4ed2e6f29471ab461abf3d4858b4d6abee28ca1094eb941c56e63744dd3984b4`; 6398 allocation samples, weighted sum 65245969024 bytes.
- C2R SHA-256 `6f9f94fc7d4d4135603c3430e68837a6b99ac2d4849dbcd8d1f1f4a9f88982ae`; 5501 allocation samples, weighted sum 59422563528 bytes.

C1 contains about 14s after the primary window, mostly Overlay-inactive; C2r stops
immediately after its delayed sampler in the same orchestration process and also
contains about 5s after Overlay auto-stop. Whole-export
frequencies/pauses are contextual aggregates with unequal coverage, not matched
pure-combined repeatability or observer costs. Collection counts include concurrent
G1 cycles; top-level pause events avoid summing nested phases. Weighted allocation
samples are indicators, not exhaustive allocation accounting. SafepointBegin is
enabled with 10ms threshold and has zero events in both exports; other safepoint
events are disabled/zero. This does not mean no safepoints. Each export has one
readable JFR chunk. Not every short thread park is captured. No shared bounded frame/JFR
clock is established; no individual GC-to-frame causal attribution.

## Metric-specific readiness and practical sensitivity

| Metric / intended claim | Readiness | Practical rule |
| --- | --- | --- |
| Controlled stationary internal pacing screening | READY WITH LIMITS | Four coherent CSVs; use new B/C observed ranges and identical observers; require repeatable effects clearly beyond that noise |
| Physical display / active-render / per-frame GC causality | UNSUPPORTED | No independent physical source or aligned clock |
| Aggregate heap commitment / post-GC use / pause diagnostics | READY WITH LIMITS | Exact-JDK readable events; optional matched JFR cohort, common coverage and settings in future |
| Small game CPU efficiency improvement | INCONCLUSIVE | Two repeats and state/control variation do not bound a small observer-adjusted effect |
| Small RSS improvement | INCONCLUSIVE | Process/heap/off-heap state varies; working set is not commitment; no small-effect equivalence bound |
| Large CPU/RSS regression screening | LIMITED | Valid counters and empirical ranges can flag large repeated differences; magnitude/goal must be predeclared and exceed ordinary/control variation |
| Exact combined observer overhead / matched GC-frequency repeatability | INCONCLUSIVE | No inactive timing reference, only one eligible C resource window and unequal JFR post-roll; no causal cost bound |
| Traversal/loading/memory-pressure/general gameplay | UNVALIDATED | Stationary subset only |

**RESOURCE COMPARISON LIMITED for A/B**, with combined repeatability,
small-effect/isolated-overhead claims INCONCLUSIVE. Restarts and the completed
control endpoints characterize real variation; the intended C counterbalance is
incomplete. They do not eliminate variation. JFR remains optional diagnostics, not mandatory for every pacing
comparison. No metric family is blocked solely by another family's limitation.
No synthetic test or historical invalid capture is promoted to empirical proof.

## Suitability of 2048→1536 MiB and shortest next action

Only 2048 MiB was executed. Observed baseline committed heap and post-GC live use
are far below 1536 MiB. Reducing a maximum alone may leave commitment, RSS, CPU and
pacing practically unchanged, and could affect G1 ergonomics in either direction.
This scene cannot support a convincing small resource-efficiency claim for that
candidate with the sensitivity established here. No FPS/memory/CPU/GC gain exists.

Keep the candidate and default G1 unchanged as a proposal. Recommend a narrowly
bounded, more memory-demanding **representative workload** that exercises heap
growth before requesting the resource experiment. Use the existing Overlay/JFR/
sampler; do not build another broad instrumentation phase. Lock its baseline
headroom and meaningful memory-use goal, then request an explicit owner scope
for the existing three alternating matched fresh-process pairs, equal >=300s
warm-up, 30s screening, identical observers, and loading/traversal/stability follow-up
only if promising. A future matched JFR cohort must have common coverage; use
Overlay-only primary pacing if combined resource uncertainty is unnecessary.
Classify effects near baseline variation INCONCLUSIVE and retain failures.

Owner authorization may instead name a narrower pacing hypothesis supported by
these measurements; this is not permission to switch or execute an alternative.
Production defaults, presets, launch flags, frame limiter, renderer and Client
source remain unchanged. No candidate, release or deployment follows this phase.

## Tests, code scope and final decision

Baseline 113 benchmark tests and 13 P1 analysis tests passed before edits. Final
116 benchmark tests and 13 P1 tests pass; three focused additions cover the locked
normal-terrain identity, crossed archive rejection and unknown/malformed workload
rejection. The only analysis-code change accepts this exact new archive/workload
pair while retaining diagnostic-only semantics, old identities and strict research
gates. No new dependency, capture framework or product runtime change.
[Test evidence](../../performance-p3a/evidence/p3a4a4/tests.json) records log hashes.
Formatting/diff/deletion/preservation checks pass. Rust/frontend build suites were
not rerun for documentation/offline-parser-only changes; actual existing native
launcher boot, authenticated readiness, supervised gameplay and normal shutdown
were exercised. No push/merge/tag/release/deploy/version bump/workflow dispatch.

**PARTIAL**: controlled practical normal terrain, fresh-process controls and capture
integrity are established, but the second combined resource slot failed timing
coverage and cannot be retried again. Resource-effect sensitivity and the proposed heap
workload remain insufficient for small benefits. Keep the working tools and move
toward a hypothesis-relevant workload rather than pursuing laboratory precision.
P3-B remains **NOT AUTHORIZED** and the prepared heap comparison is deferred.

**PARTIALLY READY — RESOURCE CLAIMS RESTRICTED**
