# P3-B.1a camera/input continuity recovery — PARTIAL

Historical B2 onset and input source remain unestablished. The scoped correction
addresses the missing boundary observations and removes pointer-based setup. It
does not prove a defect in Aurora Client or the computer-use helper. This phase
does not repeat the heap comparison or establish a JVM performance improvement.

**AURORA CLIENT HANDOFF REQUIRED**

- **Subsystem:** Aurora Client PARK frame pacing, spin tail and frame-head scheduling.
- **Evidence:** the installed 3.0.0 artifact audit identifies concrete scheduling
  behavior and unresolved parameter interactions. The heap cohort used cap 0 and
  did not test active global PARK pacing; no defect or gain is established.
- **Proposed improvement:** separately audit positive-cap overshoot, variance and
  CPU cost; implement a change only if that investigation supports it.
- **Expected benefit:** potentially steadier capped intervals or reduced CPU cost
  at equivalent pacing, without a numerical gain claim.
- **Risks:** input/tick scheduling, spin CPU cost, hardware variability, features,
  Sodium/OpenGL compatibility. Preserve manual PARK; Vulkan is a separate concern.
- **Launcher dependency:** isolated authenticated native Play, fixed JVM/workload,
  existing Overlay/JFR/resource pipeline and camera checkpoint procedure.
- **Recommended Client development phase:** **C-PERF.1 PARK scheduling/CPU audit**
  in the separate Aurora Client repository, investigation before implementation.

## Repository baseline and preservation

Branch `codex/client-3-production-authority`; starting full HEAD
`64d8cd8d66596b91882acfe93ba6b41b39caecda`, exactly as requested. Index was clean;
only the four expected developer-local files were untracked. Before implementation,
`.git/p3b1a/baseline.json` recorded branch, history, status/index, tracked/untracked
byte inventories, seven worktrees/HEADs, eleven critical identities, 2,951 retained
prior private evidence identities, production executable/config/registry identities
and the separate Client repository. There were 2,594 original tracked files.

Client HEAD `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d`, 422 tracked files.
[Sanitized baseline and exact critical/protected hashes](docs/performance-p3b1a/evidence/baseline.json).
[Final preservation audit](docs/performance-p3b1a/evidence/preservation.json).
The four protected files are `.zcode-icon-diag.py`, `.zcodeignore`,
`UserskalibAppDataLocalTempaurora-phase-d-final-evidencesodium-exit-zero-ready.txt`
and `docs/performance-p0-2/__pycache__/analyze.cpython-312.pyc`.

Production audit scope is the same bounded executable/config/registry hash check
as P3-B.1; it does not claim a complete owner-world/runtime inventory or token-byte
preservation. Production Launcher 1.4.2, owner content, credentials, system settings
and Client source are outside all test mutations. No cleanup or process-name
termination occurs. All previous benchmark reports/captures remain original.

The ending full HEAD/local commit identity is recorded in the completion response
and `.git/p3b1a/final-git.json` after commit; this report cannot contain the hash of
its own containing commit. Only scoped reviewed local additions are committed.
No push, merge, tag, release, deployment, installation into production, version
bump or workflow dispatch is performed.

## Investigation and correction

[Complete input timeline and hypothesis review](docs/performance-p3b1a/INPUT_INVESTIGATION.md).
[Exact investigation source identities](docs/performance-p3b1a/evidence/investigation-sources.json).
The original action transcript adds information absent from the earlier acceptance:
B2 used coordinate clicks for menu/world entry, then typed its teleport command
at 09:01:31 UTC and submitted it after a recorded-action gap at 09:03:58. The
09:04:01–02 start-camera call still showed 135.0/15.0. No recorded pointer action
occurs between that valid start image and the first changed final image.

F3-hide precedes warm-up. Warm-up was 09:04:06.896366–09:09:06.901569 UTC,
300.000131 s with 601/601 game-foreground samples. No intermediate camera image
exists. F10 was bracketed 09:09:16.237–09:09:16.270; resource capture was
09:09:16.699735–09:09:46.701344, 30.000147 s, 61/61 foreground. The first
changed image follows final F3 in the compound 09:09:52.828–09:09:54.142 call.
Shutdown follows it; normal-save metadata confirms
`137.8499755859375 / 12.600001335144043` at unchanged coordinates.

Persistent drift before the valid initial image and shutdown as the sole onset
are excluded. F3-hide, warm-up, transition/record trigger, recording and final
inspection remain possible intervals. Logs confirm teleport, recording and save
times, not raw mouse deltas. Foreground ownership does not prove camera continuity.
There is no evidence to blame Aurora, the helper, the owner or either heap value.

The supported helper API has absolute window-relative click/drag/scroll targets
and keyboard chords; targeted inputs automatically activate their window. It does
not expose or certify raw/relative deltas. A coordinate click can be unsafe while
mouse-look is captured, but there is no such recorded click in B2's unresolved
interval. Screenshot and key calls are not independently proven input-neutral.
No new input tracer or native hook was introduced to close that uncertainty.

Fresh-process setup also had unobserved menu navigation: S1 was first observed at
Options and S2 at Select World before this agent's first explicit game key in
each process. Both subsequently entered with correct initial camera checkpoints.
This limits claims about startup input provenance; it does not prove a helper or
owner action. See the investigation's specific owner-attended/helper-contract
follow-up rather than treating startup as independently certified input-free.

The [predeclared continuity procedure](docs/performance-p3b1a/CONTINUITY_PROTOCOL.md)
uses keyboard-only world entry, loads the exact frozen orientation without a
teleport, separates UI actions from stationary windows and adds six boundary
images. No drift is reset during warm-up/capture, and no failing window can be
silently retained. Existing F3/screenshot APIs and normal save provide observations.
No Client, Minecraft input, native product, JVM, system or observer implementation
changes are made. The defect corrected is procedural observability and avoidable
pointer exposure; historical root-cause recovery remains PARTIAL.

## Isolation and exact workload

The existing marked same-user temporary managed root and separate WebView profile
are reused with a new artifact directory and new disposable native-created
instance `2ba9a192fc384b30b7c05496778708e4`. Canonical paths/ancestors and descendant
reparse checks precede boot; executable/runtime hashes match. Live WebView parent,
profile and debug-port checks and native-reported managed root agree. Native
provider installs, content/runtime validation, normal Rust-owned authentication,
readiness, supervised Play and normal shutdown remain authoritative. No account
database, browser cookie, token or live game argument is copied/exported. The
previously accepted shared UUID-key Credential Manager refresh risk remains.

Frozen archive SHA-256
`e53e838b0136a03dbdfa8848321063b001e44a33c3dc42d76001b85dfacaf287`;
options `33695bc0df527fbfe49ec2177cc75bed7b46cadfc581ab542eedaa16ca20fe87`.
Every process starts from an equal full world/options/config inventory and exact
six-JAR hashes. Normal terrain/seed 123456789, 24/12 distances, position
128.5/96/128.5 and orientation 135/15 remain frozen. No new world or altered
terrain/entities/graphics/limiter state is prepared; ordinary simulation may evolve
after restoring equal sources. Prior canonical worlds and options stay immutable.

Minecraft 1.21.11, Fabric 0.19.5, Aurora 3.0.0, Fabric API 0.141.6+1.21.11,
Sodium 0.8.14+mc1.21.11, Lithium 0.21.4+mc1.21.11, Overlay 1.0.2 and Cloth Config
21.11.153 are unchanged. Managed Microsoft OpenJDK 21.0.7+6-LTS manifest SHA-1
`cb4394a27089d19f65d5baa6cf0482c27c3c7865`; normal native runtime validation
verified 402 files / 99,041,788 bytes. Java executable SHA-256
`1ba6a4fb32a6a3d5c925fe837ae6717e2f75743a71e013e668e6dadb909245ba`.
Each effective VM.flags confirms 2048 MiB and G1; custom text empty, no Xms,
Existing Configuration, no preset change. Window 1920×1080, native Unlimited 260,
VSync off, inactivity Minimized, Aurora cap 0/adaptive false/PARK stay unchanged.

## Validation results

[Actual session evidence and image hashes](docs/performance-p3b1a/evidence/sessions.json).
Two fresh processes were run; the third allowance was unused. No camera drift
was observed, no camera correction/retry was used, and both saved exact 135/15.

| Session | PID / creation ID | Warm-up UTC / seconds | Capture UTC / seconds | Foreground warm-up / capture | Camera / exit |
| --- | --- | --- | --- | --- | --- |
| S1 | 19388 / 134367056361061500 | 10:15:10.572207–10:20:10.579811 / 300.000383 | 10:20:35.453594–10:21:05.456217 / 30.000269 | 601/601; 61/61 | PASS / 0 |
| S2 | 27228 / 134367061422691092 | 10:23:34.028167–10:28:34.032706 / 300.000339 | 10:29:57.479806–10:30:27.481055 / 30.000447 | 601/601; 61/61 | PASS / 0 |

| Session | Checkpoint UTC (snapshot call bracket) | Phase | Yaw / pitch | Precision |
| --- | --- | --- | --- | --- |
| S1 | 10:14:59.297Z–10:14:59.636Z | after-entry | 135.0 / 15.0 | F3 rounded 0.1° |
| S1 | 10:15:05.004Z–10:15:05.073Z | before-warmup | 135.0 / 15.0 | F3 rounded 0.1° |
| S1 | 10:20:21.831Z–10:20:22.049Z | after-warmup | 135.0 / 15.0 | F3 rounded 0.1° |
| S1 | 10:20:30.057Z–10:20:30.194Z | before-capture | 135.0 / 15.0 | F3 rounded 0.1° |
| S1 | 10:21:12.591Z–10:21:12.771Z | after-capture | 135.0 / 15.0 | F3 rounded 0.1° |
| S1 | 10:21:17.120Z–10:21:17.220Z | after-inspection | 135.0 / 15.0 | F3 rounded 0.1° |
| S2 | 10:23:22.513Z–10:23:22.835Z | after-entry | 135.0 / 15.0 | F3 rounded 0.1° |
| S2 | 10:23:28.285Z–10:23:28.348Z | before-warmup | 135.0 / 15.0 | F3 rounded 0.1° |
| S2 | 10:29:40.152Z–10:29:40.679Z | after-warmup | 135.0 / 15.0 | F3 rounded 0.1° |
| S2 | 10:29:50.005Z–10:29:50.172Z | before-capture | 135.0 / 15.0 | F3 rounded 0.1° |
| S2 | 10:30:30.307Z–10:30:30.518Z | after-capture | 135.0 / 15.0 | F3 rounded 0.1° |
| S2 | 10:30:35.863Z–10:30:35.931Z | after-inspection | 135.0 / 15.0 | F3 rounded 0.1° |

Initial source metadata in both processes was exact `[135.0,15.0]`; normal-save
metadata in both was also exact `[135.0,15.0]`, with unchanged
`[128.5,96.0,128.5]`. Live images have only one-decimal display precision; no
looser tolerance is invented. All six live boundaries were observed per process.
Discrete checks cannot exclude a transient movement and return between them.

| Session | Warm-up→capture gap s | F10→sampler gap s | Prior <0.5 s performance gate | Capture end→checkpoint s | Overlay / JFR loss |
| --- | --- | --- | --- | --- | --- |
| S1 | 24.873783 | 0.625594 | FAIL | 7.134783 | COHERENT INTERNAL DIAGNOSTIC / 0 |
| S2 | 83.447100 | 0.520806 | FAIL | 2.825945 | COHERENT INTERNAL DIAGNOSTIC / 0 |

The historical performance alignment gate is retained, including every failure
shown above. These recordings are camera-continuity evidence, not eligible heap
pairs or proof of a performance benefit. No threshold is relaxed and no failed
window is favorably retried. Both Overlay exports are internally coherent;
privacy-filtered JFR exports are readable with zero reported DataLoss and excluded
privacy event counts zero. Aggregate resource/JFR windows remain separate from
physical display frames and precise per-frame GC causality.

Both samplers cover 300 s of warm-up and 30 s of measurement, but total settling
time was not identical: S2's post-warm-up setup/dispatch gap was 83.447 s versus
24.874 s in S1. Its after-warm-up image was obtained about 66 s after the sampler
endpoint. S1's post-capture image was about 7.1 s after the resource endpoint;
S2's was about 2.8 s after it. These are bounded, delayed observations rather
than instantaneous endpoints. Preserve these procedural limits as additional
reasons not to claim equivalent performance windows or fully localized input.

**Continuity checkpoints: PASS in both sessions. Overall phase: PARTIAL.**
The bounded procedure worked here, but neither B2’s input source nor its onset
was recovered and the original failure did not reproduce. These results cannot
certify a historical root cause or guarantee future input neutrality.



## Roadmap and next phase

[Full evidence-based roadmap, ownership and experiment specifications](docs/performance-p3b1a/PERFORMANCE_ROADMAP.md).
Ranked priorities are:

1. **Aurora Client C-PERF.1:** positive-cap PARK scheduling/CPU audit. Concrete
   installed mechanisms and capped-play relevance make this the best next target;
   benefit remains a hypothesis and cap-0 heap captures do not test it.
2. **Aurora Client C-PERF.2, conditional:** allocation/render/feature attribution.
   Existing JFR establishes activity, not an Aurora culprit. Profile ownership
   first, preserve feature behavior and rendering compatibility.
3. **Launcher P3-B.2, deferred:** explicit constrained-memory use-case gate and at
   most one justified memory/GC experiment. Mixed P3-B.1 effects and ample host
   memory lower its priority; no heap sweep, generic flags or collector tour.

Launcher owns JVM memory ergonomics, GC flags, exact compatible runtime selection,
its own overhead and performance advice. Client owns pacing, render-loop/UI costs,
temporary allocations, feature/event work and rendering compatibility. GC/pacing
interactions require attribution before assigning implementation. The roadmap
contains the mandatory standing handoff format for every future Launcher phase.
No work in the Client repository is performed by this task.

Keep 2048 MiB because no superior candidate has been demonstrated, not because it
is globally optimal. P3-B.1 stays INCONCLUSIVE; B1/C1 remains the only complete
pair, C2 unpaired, B2 excluded, B3/C3 unexecuted. No historical classification is
rewritten. Overlay is an internal eligible HUD-callback diagnostic, not physical
display proof. JFR is aggregate JVM/GC context; precise per-frame GC causality is
unsupported. Small CPU/RSS effects require adequate sensitivity and repeatability.

The smallest remaining continuity action is an owner-attended bounded run if the
issue recurs, using these checkpoints to isolate the first failing interval. Do
not reopen a large benchmark-refinement program or automatically repeat heaps.
Recommend the separately scoped Client C-PERF.1 investigation next, with this
historical uncertainty disclosed. P3-B.1a stops here after its bounded validation.

## Verification and local Git completion

[Test evidence](docs/performance-p3b1a/evidence/tests.json): 119 benchmark tests and
13 P1 analysis tests PASS before implementation and in final verification. No
analysis/quality-gate implementation changed, so no synthetic checkpoint framework
or extra tests were added. Session projection asserts checkpoint association/order,
equal source identity, observer coverage, exact process identity, settings and
normal-save state against actual retained evidence.

The unchanged native application was really booted, authenticated and used for
deep validation and supervised gameplay. This change consists of new procedure,
roadmap, report and sanitized evidence files plus an appended `AGENTS.md` policy,
with no dependency/native/frontend
or startup edits; rebuilding all product suites would not validate the input cause.
[Changed-file inventory](docs/performance-p3b1a/CHANGED_FILES.txt),
[source inventory](docs/performance-p3b1a/evidence/source-inventory.json) and
[anonymous raw identities](docs/performance-p3b1a/evidence/raw-inventory.json).
Private images, worlds, logs, JFR, original action excerpts and full path mapping
remain retained under `.git/p3b1a` and the isolated artifact root. No raw
account-bearing evidence or generated game/runtime/build data is staged.

The sole intended pre-existing-file edit is the owner-required standing Client
handoff policy appended to `AGENTS.md`. Its original bytes remain an exact prefix;
the preservation audit records both hashes and verifies the exact addition. All
other original tracked files and the other ten critical identities retain bytes.

[Shutdown evidence](docs/performance-p3b1a/evidence/shutdown.json) confirms both
game exits 0, final native content Ready, unchanged disposable configuration at
2048 MiB, and the launcher's normal exit 0 through its retained exact handle.
The final saved world was copied byte-equal into retained evidence as well as
remaining in its disposable instance. Deletion/rename diffs are empty; all seven
worktrees remain attached with the other six HEADs unchanged. Four protected
local files, prior evidence, canonical source, production identities and Client
state/hashes pass the final audit. No deletion or cleanup is staged or performed.
