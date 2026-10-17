# P3-B.1 heap sizing experiment — PARTIAL / INCONCLUSIVE

The preparation gate passed, but the comparison matrix stopped after B2 failed
the camera identity check. B1, C1 and C2 are valid; only Pair 1 is complete.
The candidate is INCONCLUSIVE. No repeatable benefit or candidate-caused regression
is established. Keep 2048 MiB and existing production defaults unchanged.

## Repository and preservation

Branch: `codex/client-3-production-authority`.
Starting full HEAD: `0b1f5ac55825195e8783f519f5ad7c959c7c898b`.
The scoped local commit identity and ending full HEAD are recorded in the
completion response and `.git/performance-p3b1-final.json`; this report belongs to
that commit and cannot contain its own hash. No push or publication occurred.

Before implementation, `.git/performance-p3b1-baseline.json` captured status,
index, history, tracked/untracked byte inventories, seven worktrees/HEADs,
eleven critical files, canonical locks,2,121 prior private evidence files,
three production identities and the separate Aurora Client repository.

[Current preservation audit](docs/performance-p3b1/evidence/preservation.json):
PASS. All 2,580 original tracked files remain, all four protected local files
retain their bytes, the other six worktree HEADs are unchanged, and no prior
evidence or critical file changed. Client HEAD
`5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d`, 422 tracked files, index/status and
worktrees remain unchanged. No deletion/rename. Only the offline Overlay reader
and its focused tests modify existing source; new phase documents/evidence are
separate. Product/native/frontend/preset/schema/release sources are untouched.

Production Launcher 1.4.2 executable/configuration/registry hashes match. This
bounded audit does not claim a complete owner-world/runtime inventory or token-byte
preservation. All game/world/runtime actions target the existing marked temporary
managed root, new disposable instance `dca43ecadb984d0bb088e376c4059fd4`, separate
WebView profile and new`p3b1-artifacts` directory. Native create/provider install,
deep validation, authentication, readiness and supervised Play are used. No owner
instance/world, normal `.minecraft`, Client source or production installation is
targeted. The previously accepted same-user UUID-key Credential Manager refresh
rotation risk remains; no credential/token read/export/copy or auth bypass occurs.

## Workload and preparation

[Exact workload/settings/mod hashes](docs/performance-p3b1/evidence/workload.json).
Option A: normal noise Overworld, seed 123456789, structures on/bonus chest off,
creative flying/normal difficulty, camera (128.5,96,128.5), yaw 135/pitch 15, clear
noon frozen, natural mob spawning off and random tick speed 3. No manual entities,
block machines or artificial Java allocations were added. Render/simulation24/12
loads more normal terrain and existing entity/block-entity content. This is a
stationary representative subset, not traversal/loading/combat/server coverage.

Minecraft 1.21.11, Fabric 0.19.5, Aurora 3.0.0, Fabric API 0.141.6+1.21.11,
Sodium 0.8.14+mc1.21.11, Lithium 0.21.4+mc1.21.11, diagnostic Overlay 1.0.2 and
Cloth Config 21.11.153. Microsoft OpenJDK 21.0.7+6-LTS; exact runtime manifest
SHA-1 `cb4394a27089d19f65d5baa6cf0482c27c3c7865`. Default G1, no explicit Xms,
empty custom JVM text, Existing Configuration and null window override.

Physical game client 1920×1080 windowed, Custom graphics/clouds off/particles All,
AO on/mipmap 4/FOV 70/GUI 2, no resource/shader packs. Native Unlimited 260 sentinel,
VSync off, inactivity Minimized, Aurora cap 0/adaptive false/PARK. Desktop
2560×1440/240 Hz, RX 9070 XT driver 32.0.31041.1004, AC connected and unchanged
Ultimate Performance scheme are recorded, not modified. Continuous GPU utilization,
thermals, effective frequency, VRR and paging rates remain unknown.

Preparation started from preserved normal-terrain archive
`de973b21dc49c5ea2a71d60f452b1c8f28e8383264892f021b38f5feb07892f6`.
Additional ordinary terrain generation occurred during preparation, outside the
matched cohort. After normal save/exit, the new snapshot/archive was exclusively
created and complete copy inventories compared. Comparison archive SHA-256:
`e53e838b0136a03dbdfa8848321063b001e44a33c3dc42d76001b85dfacaf287`
(40,417,280 bytes; 47 files/12 directories). Options SHA-256:
`33695bc0df527fbfe49ec2177cc75bed7b46cadfc581ab542eedaa16ca20fe87`.
All 2,401 required full chunks in [-16,-16]–[32,32] exist; 5,625 total recorded
chunks, 199 persisted entities and 803 persisted block entities are not live counts.
F3 boundary entity denominators were13 then10 in preparation. Loaded chunks and
block entities remain unknown. Dynamic entities may evolve after equal restores.
Canonical A and the earlier normal-terrain source remain byte-identical.

[Preparation/attempt evidence](docs/performance-p3b1/evidence/preparation.json).
Initial PID 23108/creation 134366984825874295 is INVALID for the baseline gate:
foreground 424/601, followed by owner interruption and changed camera/window.
No CSV/JFR candidate data came from it; normal exit 0 and its complete materials
are retained. The one predeclared interrupted-setup retry uses a fresh process
and byte-equal source restoration, with no workload adjustment.

Retry `prep-r`: PID 18180/creation 134366989846857567, warm-up
08:24:16.703533Z–08:29:16.709127Z on 2026-10-17 (300.000107 s), 601/601 foreground.
Capture resource window 08:30:12.799521Z–08:30:42.800865Z,61/61 foreground;
the 55.7 s warm-up/setup gap is retained, not hidden. Normal game exit 0.
Overlay 27,406 intervals,30,035.232ms sum,30,032ms elapsed span, median1.046ms,
P95=1.621 ms/P99=2.1139 ms/max 8.466 ms,912.462 eligible callbacks/s, zero>50ms.
Raw row/footer/numeric/producer-summary/writer checks pass. JFR 3,164,550 bytes,
SHA-256 `8e569c872ff0526d6dbbb11c4f697e7438014db23bc3f166988e1f9303806f3d`;
one chunk, 31 s rounded export, zero reported DataLoss/privacy-excluded events.

Over its approximately 30 s resource window: 1,457 MiB heap commitment,772–796MiB
after GC, maximum before-GC use approximately1,386MiB; ten collections include
concurrent G1 cycles. Thirteen top-level pauses total 36.6236 ms, max 4.6756 ms.
Allocation samples are weighted indicators, not exhaustive accounting. Mean RSS
approximately2,376.45MiB; game/system/residual CPU 3.4243/7.6002/4.1759 machine
percent. Available physical memory exceeds 16,320 MiB. RSS, commitment, used heap
and maximum reservation are distinct. Preparation is not included in paired means.

Relevance passes: ordinary content produces materially more commitment/live use
than the prior stationary scene and observable allocation/GC activity. The
after-GC envelope leaves approximately 740 MiB below 1536 MiB; the frozen comparison
source removes preparation generation. This supports a bounded static candidate,
not a guarantee for loading/traversal. Lowering Xmx can improve, worsen or leave
commitment/RSS/GC unchanged. No forced GC or workload adjustment was used.

## Locked experiment and measurement rules

[Predeclared plan and transparent retry amendment](docs/performance-p3b1/TRIAL_PLAN.md).
[Configuration-to-spawn audit and semantic JVM vector](docs/performance-p3b1/JVM_CONFIGURATION.md).
The only treatment argument difference is `-Xmx2048m`→`-Xmx1536m`, saved through
the normal stale-checked native command. Runtime VM.flags and Overlay maximum
verify effectiveness without exporting command lines/LaunchSpec/session arguments.
Ergonomic VM values may change in response; no explicit Xms or GC flag is added.

| Pair | First process | Second process |
| --- | --- | --- |
| 1 | B1: 2048 MiB | C1: 1536 MiB |
| 2 | C2: 1536 MiB | B2: 2048 MiB |
| 3 | B3: 2048 MiB | C3: 1536 MiB |

Each executed comparison uses a fresh supervised process, byte-equal frozen world/options/configuration,
same mods/runtime/launcher/observers, live camera verification,>=300s stationary
warm-up, 30 s Overlay and 0.5 s exact-creation resource sampling, then normal shutdown.
The existing privacy-filtered JFC SHA-256 is
`d48adb550887e5e4e0fd8c3aedbd05e5c234aef17ec5812bb06310a290af5a80`.
JFR has 45 s safety stop/maxage 60 s/maxsize 32 MiB, explicit isolated repository,
normal named stop immediately after sampling, and exact-JDK reader verification.
Fully contained events in the resource window supply equal approximately 30 s GC
aggregates; crossing events and full export pre/post-roll remain separately retained.
No unequal whole-export rates or nested-phase pause sums are compared.

The practical goal is >=128 MiB repeated saving in commitment or mean RSS across
all three pairs, beyond observed variation, without material pacing/GC/stability
regressions. Three pairs do not establish population confidence bounds. Small
CPU/RSS differences remain inconclusive; no presumed observer cost is subtracted.
P99/P95/median and>50ms tails take precedence over isolated throughput gains.
Internal eligible HUD callbacks are neither all rendered nor physically displayed
frames. Precise GC/frame causality is unavailable. Unknown environment telemetry
and combined-observer effects limit attribution.

## Execution stopped on workload drift

[Valid runs and original metrics](docs/performance-p3b1/evidence/runs.json).
[Excluded and unexecuted attempts](docs/performance-p3b1/evidence/excluded.json).
All timestamps below are UTC on 2026-10-17; durations use monotonic sampling.

| Run | Heap MiB | PID / creation ID | Warm-up UTC / seconds | Resource UTC / seconds | Status |
| --- | --- | --- | --- | --- | --- |
| B1 | 2048 | 13208 / 134366996011148549 | 08:34:55.041305–08:39:55.046925 / 300.000227 | 08:40:19.923423–08:40:49.925639 / 30.000322 | VALID |
| C1 | 1536 | 9100 / 134367001383373955 | 08:43:38.306595–08:48:38.312218 / 300.000252 | 08:48:58.911342–08:49:28.913493 / 30.000450 | VALID |
| C2 | 1536 | 26440 / 134367006537294637 | 08:52:33.308285–08:57:33.313795 / 300.000389 | 08:57:50.550289–08:58:20.551856 / 30.000271 | VALID |
| B2 | 2048 | 20360 / 134367012225187561 | 09:04:06.896366–09:09:06.901569 / 300.000131 | 09:09:16.699735–09:09:46.701344 / 30.000147 | INVALID: camera drift |

Every measured process passed 601/601 foreground warm-up samples and 61/61
foreground capture samples. Fresh-process creation IDs, restored source inventory,
effective Xmx and G1 were checked. Live start/end F3 images and normal-save world
metadata agree on 135°/15° for B1/C1/C2. Post-warm-up setup gaps are retained below;
no forced GC was used. Native deep validation preceded every Play. Extra explicit
installed-state hash snapshots were added outside warm-up from C2 onward; the
common installed-state reference hashes were recorded after Pair 1, not claimed
as additional contemporaneous B1/C1 snapshots.

| Valid run | Warm-up → capture gap s | F10 → sampler gap s | Start committed / used MiB | End warm committed / used MiB |
| --- | --- | --- | --- | --- |
| B1 | 24.876 | 0.449 | 1,484.0 / 948.3 | 1,484.0 / 1,242.5 |
| C1 | 20.599 | 0.406 | 1,512.0 / 1,333.2 | 1,512.0 / 1,165.9 |
| C2 | 17.236 | 0.491 | 1,536.0 / 920.4 | 1,536.0 / 999.9 |

B2 began at (128.5,96,128.5), yaw 135°/pitch 15°. The first post-capture
F3 image shows approximately 137.8°/12.6°; normal-save metadata confirms
137.8499755859375°/12.600001335144043° at unchanged coordinates. The onset and
cause are unknown. Foreground continuity does not prove camera continuity, and
the data cannot establish whether drift occurred during warm-up, capture or
the final check. B2 is therefore INVALID even though its CSV/JFR are readable.
No owner intervention or heap causality is inferred. Its world, options, logs,
configuration, screenshots, CSV, JFR and samplers are retained and hashed.

The earlier interrupted-setup diagnostic retry was already used. Conservatively
stop on renewed camera/setup drift without creating a new retry category or
relaxing the workload check. B3 and C3 were NOT RUN. Pair 2 has a valid C2 but no
valid baseline; Pair 3 has neither member. No cross-pair replacement or pooling
of invalid B2 is permitted. The fixed order was never changed.

## Recording quality and raw identities

All three valid CSVs pass the established row/footer/numeric, duration,
producer-summary and writer-status checks. The pinned reader still labels these
coherent internal diagnostics, without independent proof of rendered-frame
completeness. All JFRs are single chunks below 32 MiB, readable by the exact
managed JDK reader, with zero reported DataLoss and zero enabled privacy-excluded
event occurrences. Full exports are 31 s rounded; the analysis selects fully
contained events in the actual approximately 30 s resource window. No valid run
has a boundary-crossing collection or top-level pause. Exports and pre/post-roll
remain retained. No GC phase nesting is summed.

| Run | CSV SHA-256 | JFR SHA-256 / bytes | JFR analysis s |
| --- | --- | --- | --- |
| B1 | 522a100a086edb5174d0d53566e8c370dbc8ac67ed4a8eaac3e8145492df17a6 | f7e0ca9a562f02040c3c110d2bf7fa14488b928cdc9ef928c7bf48dc0f4d2aae / 2,952,588 | 30.002216 |
| C1 | c36d744a9c1ef1fe8886451a62f62fd559316b954477391bdbf8f67b4364f144 | 279b44067fff2cfaebfdea080e55738edee5d48be9cc2a29a9b0b892d0c3314f / 2,973,362 | 30.002151 |
| C2 | 37ec319712d3b1ec27ea6669201210a47d5328183b5171c7ac797fd1f91f57ca | 461d80031d405cb03ce4258bd73d8b4e9b942100b60df1eeda8105d7277219fb / 3,011,967 | 30.001567 |
| B2 (excluded) | 4ba97de7603e9395a593999154c23e0b3929756b7edf826f07f60e5cd49db864 | 5cae05f49dacb116ce915419c1a92c46bafabfb401dd20637fc479d36726a992 / 2,747,679 | 30.001609 |

[Anonymous raw-byte inventory](docs/performance-p3b1/evidence/raw-inventory.json)
maps SHA-256/size to stable local evidence IDs; `.git/p3b1/raw-inventory-private.json`
retains the exact private path mapping. Raw materials are outside tracked source
under the isolated `p3b1-artifacts` root and `.git/p3b1`. Account-bearing paths,
logs/worlds and screenshots stay private. No live launch command, argument array,
LaunchSpec or token is exported into the report.

## Frame, heap, GC and system results

Only B1/C1/C2 enter the following descriptive tables. Intervals above 50 ms use
a strict >50 ms threshold; all have zero such intervals and zero bursts.

| Run | Intervals / sum ms / span ms | Median / P95 / P99 / max ms | Eligible callbacks/s |
| --- | --- | --- | --- |
| B1 | 27,734 / 30,035.525 / 30,032 | 1.03400 / 1.68500 / 2.19100 / 7.42300 | 923.373 |
| C1 | 26,715 / 30,035.725 / 30,032 | 1.04500 / 1.57700 / 2.00458 / 6.56500 | 889.441 |
| C2 | 27,368 / 30,036.560 / 30,033 | 1.04950 / 1.62165 / 2.16833 / 6.38100 | 911.156 |

| Run | Committed MiB | After-GC used MiB | Before-GC max MiB | Collections (new/old) | Pauses: count / sum / median / P95 / max ms |
| --- | --- | --- | --- | --- | --- |
| B1 | 1,484.0 | 674.527–691.766 | 1,414.972 | 8 (6/2) | 10 / 27.8288 / 3.3285 / 4.5568 / 4.9405 |
| C1 | 1,512.0 | 676.447–691.040 | 1,443.616 | 8 (6/2) | 10 / 30.1732 / 3.4475 / 4.6707 / 5.0429 |
| C2 | 1,536.0 | 677.065–687.449 | 1,461.065 | 6 (5/1) | 7 / 24.2897 / 3.4119 / 5.1804 / 5.2007 |

| Run | Allocation samples / weight bytes | Mean RSS MiB | Game / total / residual CPU machine % | Minimum available MiB |
| --- | --- | --- | --- | --- |
| B1 | 4,316 / 5,492,029,088 | 2,387.285 | 4.037527 / 7.929860 / 3.892333 | 16,370.012 |
| C1 | 4,339 / 5,197,456,608 | 2,366.786 | 3.238173 / 7.012320 / 3.774147 | 16,335.527 |
| C2 | 4,312 / 5,501,255,576 | 2,439.480 | 3.810641 / 7.755315 / 3.944674 | 17,447.109 |

Collection counts include concurrent G1 cycles; top-level pauses are separate.
Allocation weights are sampled indicators, not exhaustive allocation accounting.
After-GC used bytes are phase-dependent observations, not an exact immutable live
set. RSS is time-weighted working set, not commitment or maximum reservation.
CPU is percent of total machine capacity; residual is an unattributed estimate
including OS/WebView/attach work. These observers do not establish CPU efficiency
or precise causal alignment between GC and individual intervals.

## Matched-pair differences and observed variation

[Full absolute/relative comparisons](docs/performance-p3b1/evidence/comparisons.json).
Only Pair 1 is valid. Signs below mean C1 minus B1; percentages use B1 as denominator.

| Metric | B1 | C1 | Absolute delta | Relative delta % |
| --- | --- | --- | --- | --- |
| P99 ms | 2.191000 | 2.004580 | -0.186420 | -8.508 |
| P95 ms | 1.685000 | 1.577000 | -0.108000 | -6.409 |
| Median ms | 1.034000 | 1.045000 | 0.011000 | 1.064 |
| Eligible callbacks/s | 923.373239 | 889.440824 | -33.932415 | -3.675 |
| Maximum interval ms | 7.423000 | 6.565000 | -0.858000 | -11.559 |
| Committed MiB | 1,484.000000 | 1,512.000000 | 28.000000 | 1.887 |
| After-GC maximum used MiB | 691.765709 | 691.039726 | -0.725983 | -0.105 |
| Collections | 8.000000 | 8.000000 | 0.000000 | 0.000 |
| Collections/s | 0.266647 | 0.266648 | 0.000001 | 0.000 |
| Top-level pause count | 10.000000 | 10.000000 | 0.000000 | 0.000 |
| Pause sum ms | 27.828800 | 30.173200 | 2.344400 | 8.424 |
| Pause P95 ms | 4.556785 | 4.670705 | 0.113920 | 2.500 |
| Maximum pause ms | 4.940500 | 5.042900 | 0.102400 | 2.073 |
| Allocation sample count | 4,316.000000 | 4,339.000000 | 23.000000 | 0.533 |
| Sample weight bytes | 5,492,029,088.000000 | 5,197,456,608.000000 | -294,572,480.000000 | -5.364 |
| Mean RSS MiB | 2,387.285491 | 2,366.785609 | -20.499882 | -0.859 |
| Game CPU machine pp | 4.037527 | 3.238173 | -0.799354 | -19.798 |
| System CPU machine pp | 7.929860 | 7.012320 | -0.917540 | -11.571 |
| Residual CPU machine pp | 3.892333 | 3.774147 | -0.118186 | -3.036 |

Pair 1 is mixed: lower P99/P95, slightly higher median and lower callback rate;
commitment increases by 28 MiB while RSS decreases only 20.50 MiB, and pause sum
increases by 2.3444 ms. This does not meet the predeclared 128 MiB memory goal.
It neither establishes a repeatable gain nor a repeatable regression.

Only one valid baseline exists, so its zero numerical range is not a measured
repeatability bound. Candidate C1/C2 supply the following limited observed range;
they are unpaired repeats, not a replacement for the missing baseline runs.

| Candidate metric | C1 | C2 | Observed absolute range |
| --- | --- | --- | --- |
| P99 ms | 2.004580 | 2.168330 | 0.163750 |
| P95 ms | 1.577000 | 1.621650 | 0.044650 |
| Median ms | 1.045000 | 1.049500 | 0.004500 |
| Eligible callbacks/s | 889.440824 | 911.156271 | 21.715447 |
| Committed MiB | 1,512.000000 | 1,536.000000 | 24.000000 |
| Pause sum ms | 30.173200 | 24.289700 | 5.883500 |
| Mean RSS MiB | 2,366.785609 | 2,439.480302 | 72.694692 |
| Game CPU machine pp | 3.238173 | 3.810641 | 0.572468 |
| Residual CPU machine pp | 3.774147 | 3.944674 | 0.170527 |

Candidate RSS varies by approximately 72.69 MiB, exceeding Pair 1's 20.50 MiB
decrease. Candidate CPU and pacing also vary. Earlier-scene variation is context,
not a new-scene noise bound. The missing valid baseline repeats prevent an
adequate sensitivity or equivalence conclusion. Invalid B2 metrics are retained
only in `excluded.json` and never used to estimate baseline variation or effects.

## Safety, decision and next action

| Candidate | Maximum configured MiB | After-GC headroom MiB | 256 MiB screen |
| --- | --- | --- | --- |
| C1 | 1536 | 844.960 | PASS |
| C2 | 1536 | 848.551 | PASS |

No observed OOM, crash or candidate-caused stability failure occurred. Every
executed game process exited normally with code 0, including both excluded
attempts. Candidate after-GC headroom passes the bounded static screen; it does
not certify loading/traversal or longer-session safety. No severe repeated
pacing/GC regression was established. Camera drift is an integrity failure of
one run, not evidence that 2048 or 1536 MiB causes instability.

Acceptance: **PARTIAL**. Candidate classification: **INCONCLUSIVE**.
The preparation workload was relevant, but only one matched pair survived.
There is no demonstrated practically meaningful improvement and insufficient
evidence to claim NO MATERIAL IMPROVEMENT, equivalence, or REGRESSION across the
requested matrix. No promising-result follow-up was conducted. Do not adopt
1536 MiB or change presets/defaults. Retain 2048 MiB and stop P3-B.1 here.
Any future JVM work requires separate authorization; first resolve reproducible
camera/input continuity before repeating a frozen, fully matched experiment.
No unrelated flags, heap sizes, collectors, presets or performance work were added.

## Verification, shutdown and Git completion

[Test logs and hashes](docs/performance-p3b1/evidence/tests.json): baseline 116
benchmark tests and 13 P1 tests PASS; final 119 benchmark tests and 13 P1 tests
PASS. The three additions keep the new exact workload diagnostic and reject
historical/new archive crossovers. No dependency or product/native/frontend code
changed. The existing native executable actually booted, authenticated, deeply
validated content/runtime and supervised gameplay/normal exits. Full Rust and
frontend rebuild suites are not warranted by this documentation/offline-reader
identity change.

[Normal shutdown and final isolated configuration](docs/performance-p3b1/evidence/shutdown.json):
the last game exited 0, the normal stale-checked native Save verified 2048 MiB,
all other configuration fields equal the original, and content remained Ready.
The launcher then closed normally, with its retained process handle reporting
exit 0. No process-name termination or cleanup was used. Disposable evidence
and worlds remain retained. Production defaults, schemas and owner state remain
outside this experiment's mutations, subject to the stated credential/audit limits.

[Changed-file inventory](docs/performance-p3b1/CHANGED_FILES.txt) and
[source/evidence hash inventory](docs/performance-p3b1/evidence/source-inventory.json)
describe the local commit. Before staging, the original tracked set, critical
files, canonical locks, prior evidence, protected files, production identities,
Client repository and seven worktrees were rechecked. Name-status and deletion
diffs contain no deletions or renames. Only two existing files have scoped edits;
all other phase deliverables are new. Build/game/runtime/raw account-bearing data
is not staged. No earlier acceptance report or strict research gate was rewritten.
No push, merge, tag, release, deployment, production installation, version bump
or workflow dispatch occurred. P3-B.1 ends with this partial report.
