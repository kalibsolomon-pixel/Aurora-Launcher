# P3-A.4a.4 predeclared fresh-process validation

Declared before the first Minecraft launch in this phase. This is validation,
not a JVM optimization. P3-B remains NOT AUTHORIZED.

| Window | Condition | Overlay | JFR |
| --- | --- | --- | --- |
| 1 | A1 control | Installed, inactive | None |
| 2 | B1 | Recording, HUD hidden | None |
| 3 | C1 | Recording, HUD hidden | Privacy-filtered default |
| 4 | C2 | Recording, HUD hidden | Privacy-filtered default |
| 5 | B2 | Recording, HUD hidden | None |
| 6 | A2 control | Installed, inactive | None |

The palindrome A–B–C–C–B–A gives each condition the same mean ordinal position.
Two observations per condition describe observed variation, not a confidence bound
or a statistical isolation of nonlinear drift. No result-dependent reorder.

One short setup/compatibility pilot may create and freeze normal-terrain B. Target
the existing B specification: seed123456789, creative/normal, overworld,
(128.5,96,128.5), yaw135/pitch15, frozen clear noon, mob spawning off,
random ticks3, generated 512-block square, render/simulation12/8. Verify actual
terrain, entities, chunks and camera after normal save/exit, then compare full
inventories of fresh disposable restorations. Successful creation alone is not a
canonical lock. If this cannot be established in the bounded setup, use the existing
B-stack/A-scene diagnostic lock for resource controls and report representative B
pending. Never change canonical A or claim its scene represents ordinary gameplay.

All six windows use a fresh launcher-supervised game process, a fresh equal world
copy, the same options/mod configuration and the exact pinned six-JAR diagnostic
stack from P3-A.4a.3 (four product mods plus Overlay1.0.2 and required Cloth Config).
Microsoft OpenJDK21.0.7+6-LTS; Existing Configuration, memory2048MiB, empty custom
arguments, default G1, null launcher window override. Saved game client target
1920x1080 windowed, native Unlimited260, VSyncoff, inactivity Minimized;
Aurora cap0, adaptive sleepingfalse. No tuning/candidate/default change.

Each fresh process receives at least300s uninterrupted stationary in-world warm-up,
then one30s resource window sampled every0.5s with exact PID+creation handles.
Record actual warm-up/capture boundaries, focus observations and physical client
geometry. Recorder startup/pre-roll occurs outside the resource window and is
reported separately; actual Overlay duration is its exported summed intervals.
No screenshot or other UI query during the timed resource window. Controls cannot
provide inactive Overlay frame exports; do not invent an independent timing reference.

Reuse the existing sampler, CSV readers and installed jcmd/jfr. Obtain the same
bounded heap-info diagnostic outside every primary window if available. JFR only
in C: pinned privacy profile, named recording, isolated repository/export,
45s safety stop, maxage60s/maxsize32MiB, explicit stop after resource capture.
Retain commands, summary, selected event JSON, raw hashes and failures privately.
No live command-line/token export, forced GC or system-setting changes.

Compare A1/A2, each condition's first/second observation, condition means/ranges,
within-window CPU/RSS trends and end-warm-up state. Use weighted CPU endpoint
deltas against total machine capacity; residual is unattributed estimated work.
Report sampling quantization/sequential-read uncertainty and available memory.
Do not subtract one unrelated control, infer a culprit, or claim negligible overhead.
If control range overlaps condition effects, resource overhead remains unresolved.

Classify metric families independently. Use observed variation to describe practical
sensitivity; no universal CPU threshold, significance/precision claim or new strict
gate. Separate JFR aggregate timing from HUD intervals without an aligned clock.
Retain failures/exclusions. At most one documented environmental/procedural retry
per failed condition; no favorable reruns, maximum six valid primary windows.
If completion is blocked, report PARTIAL and the smallest next action.

## Procedural amendment after excluded first control, before its single retry

The first A1 process exited normally, but its post-exit orientation was
(146.39984,13.05001), versus the frozen(135,15). Position and chunk coverage
matched. Its window is INVALID for the intended camera and is retained, excluded
from comparisons. This is the one allowed diagnostic retry for condition A, named
A1r, before proceeding to B1; no result-dependent reorder or favorable rerun.

An inventory-equal saved camera did not establish the live camera after UI loading.
For A1r and every remaining process, reset the same camera through the ordinary
in-game teleport command after loading, inspect live F3 coordinates/orientation,
hide F3, then begin the same300s stationary warm-up. Verify saved orientation again
after exit. Do not infer precisely which input/loading interaction caused drift.

C1 JFR stopped normally at approximately45s; its30s resource window completed
earlier and the export contains about14s of post-window context, mostly after
Overlay auto-stop. Retain its full aggregate as diagnostic context, not an exactly
aligned combined30s GC cohort. For C2, invoke normal JFR stop immediately in the
same capture script after the resource sampler completes, avoiding controller
round-trip delay. Do not retry C1 or compare mixed whole-export GC rates as isolated
observer overhead. This does not change the six primary resource windows.

## C2 procedural failure and single C retry

C2 is excluded: the F10 action was rejected before any key was sent, and its
resource window began before the warm-up sampler completed. Its JFR-only export,
resource series, world and zero-CSV outputs are retained as INVALID comparison
evidence. Use the one permitted C retry, C2r, with a fresh restored world/process.
Require completed warm-up evidence before the end-warm-up attach or JFR start;
only start the primary sampler after the successful Overlay key action. C2r
replaces C2 in the declared sequence; B2 and A2 remain next. No second C retry.

C2r export review finds a4.952s delay from successful F10 completion to sampler
start. Its final resource segment therefore extends beyond Overlay auto-stop.
Keep its coherent30s frame export and readable JFR as diagnostics, but exclude the
mixed-state resource window from full combined-condition comparisons. C has only
one eligible primary resource observation. The retry budget is exhausted; finish
the predeclared B2/A2 endpoints and report PARTIAL rather than add a third C attempt.
