# Performance P3-A.4a acceptance

## 1. Verdict

**PARTIAL. P3-B NOT READY / NOT AUTHORIZED.** Composed-copy semantics and three
capped/uncapped captures explain why application records and available display
estimates differ. More seriously, independent high-rate WGL markers prove continued
swaps across long PresentMon row holes. Neither uncapped submission nor display
tails are an accepted primary performance metric. Observer interference is not
defensibly bounded. No JVM/preset/renderer change or performance improvement claim.

## 2. Repository and local commit

Branch `codex/client-3-production-authority`; starting full HEAD
`aaa21984a3bac564d988733a1786572aee870aa4`, exactly as requested. Initial tracked
tree/index clean; 2481 tracked files, four protected developer files, seven worktrees.
Pre-change status/index/history/full original hashes, evidence inventory, critical
files, canonical identities and limited production state are recorded privately
in `.git/performance-p3a4a-baseline.json`.

One scoped local commit contains this phase: **Validate P3-A.4a timing fidelity and
preserve partial verdict**. Ending review HEAD is that commit; its exact full SHA
is recorded after commit in `.git/performance-p3a4a-final.json` and the final chat
response. A commit cannot contain its own content-dependent SHA. No push, merge,
tag, version bump, release, publication, deployment or production installation.

## 3. Complete changed-file inventory

[P3A4A_CHANGED_FILES.txt](../../performance-p3a/P3A4A_CHANGED_FILES.txt): **28 paths,
19 additions and nine scoped modifications; zero deletions**. New short-only
fidelity review/tests, opt-in high-rate stimulus extension, focused source/protocol/
OpenGL/overhead/quality reports, sanitized evidence, this acceptance and inventories.
The four requested current reports are updated with historical suffixes retained.
Authorization/short-format/tool guides reflect implemented phase behavior.
Rust/Svelte/TypeScript, native commands/startup, manifests, lockfiles, presets,
renderer, full measured importer/schema/thresholds and canonical locks are unchanged.

## 4. Deletion and preservation audit

[Audit](../../performance-p3a/evidence/p3a4a/preservation.json): all **2472 unmodified
original tracked files plus four protected developer files** are byte-identical;
only nine scoped originals change. All 421 prior private P3-A.2/P3-A.3 evidence
files, frozen worlds/archives/failed inspections, 11 critical files, seven worktrees
and six other HEADs are preserved. No generated binaries/game/runtime data staged.
Production Launcher 1.4.2 binary/config/registry hashes match the baseline. This
is not an owner-world/credential inventory or a freeze of independent software writes.
No cleanup/reset, recursive deletion, process-name termination or system-setting change.

## 5. Isolation and execution

[Environment/isolation](../../performance-p3a/evidence/p3a4a/environment.json).
The established marked temp root, ancestors/descendants, native reported root,
separate live WebView profile, exact instance/runtime/tool paths and signatures
were checked. The previously approved startup-refresh ordering exception and
accepted same-user Credential Manager continuity risk remain explicit. Normal
session restoration/entitlement succeeded; no interactive sign-in was needed.
No secret inspection/export/copy/backup/restore or production sign-in probe.
One normal game child and launcher both exited **0**, with native exact-child
supervision and the retained launcher handle. A legacy-shell script-file attempt
was refused before launch; the existing current shell permitted the reviewed
script under its existing policy. No execution policy was changed.

## 6. PresentMon interpretation

[Pinned field/source review](../../performance-p3a/PRESENTMON_FIELD_SEMANTICS.md)
and [source hashes](../../performance-p3a/evidence/p3a4a/sources.json) distinguish
kernel-derived Other timestamps, CPU accounting, GPU attribution, DWM-associated
display estimates and unavailable physical scanout. The actual header has no
Dropped/DisplayedTime/FrameType field. Zero API duration is not measured zero WGL
duration; SyncInterval=-1 is unknown. NA is neither zero nor proof of lost events
or a physically dropped frame. No original parser arithmetic bug was found;
an actual old reader result remains exactly unchanged on reinspection.

## 7. OpenGL findings and controlled comparisons

[Investigation](../../performance-p3a/OPENGL_PRESENTATION_INVESTIGATION.md),
[actual matrix](../../performance-p3a/evidence/p3a4a/minecraft.json).
One stationary restored-A child, Minecraft 1.21.11/Fabric 0.19.5, exact Microsoft
OpenJDK 21.0.7+6-LTS, Balanced/2048 MiB/empty custom text, 1920x1080 windowed, 12/8,
Minimized inactivity limiting, VSync off. First trace follows **300.120 s** corrected
stationary warm-up. Three 16-second captures each have a 10-second inner window.

| Option | Raw rows / display NA | Inner observed submission rows/sec | Available estimates/sec | Max estimate interval |
| --- | --- | --- | --- | --- |
| Unlimited (260 sentinel) | 29605 / 27630 (93.329%) | 1827.332 | 122.354 | 512.497 ms |
| Cap 120 | 1889 / 0 | 118.198 | 118.194 | 14.026 ms |
| Cap 240 | 3695 / 47 (1.272%) | 231.541 | 228.398 | 10.444 ms |

Every row remains Other/0x0/Composed: Copy with GPU GDI, flags 0, tearing 0,
sync interval -1. All exact identities and 36 foreground observations/condition
match; producers exit 0; QPC frequency is 10000000 Hz before/after. Latest-copy/DWM
association is an evidence-backed mechanism for fewer display estimates, not an
individual drop classification. No fullscreen/VSync-on/global-setting test.

## 8. Primary and secondary metric decisions

**No accepted PRIMARY for uncapped JVM comparisons.** Uncapped submission and
display tails are **DIAGNOSTIC ONLY**. Capped submission cadence and exact-child
CPU/RSS are **SECONDARY** with stated limits. GPU attribution is **DIAGNOSTIC ONLY**;
physical displayed frames and exact render/API-entry timing are **UNAVAILABLE**.
[Decision/gates](../../performance-p3a/P3A4A_QUALITY_GATES.md).

[Gap evidence](../../performance-p3a/evidence/p3a4a/gap-review.json) shows a controlled
**1873.352 ms CSV row hole while 3356 independently recorded swaps continue**,
maximum independent interval inside it 1.7741 ms. Minecraft has overlapping roughly
half-second submission/display holes; its uncapped maximum submission interval is
508.617 ms. These CSV tails cannot support application or physical stutter claims.
The exact driver/consumer missing-record mechanism remains unresolved.

## 9. Representative-rate overhead

[Protocol](../../performance-p3a/P3A4A_DIAGNOSTIC_PROTOCOL.md),
[results](../../performance-p3a/P3A4A_OVERHEAD_RESULTS.md).
One 80-second 1800-Hz WGL trial achieved 1789.54–1791.75 Hz, mirrored none /
PresentMon / sampler / both / both / sampler / PresentMon / none. Every block
retains independent markers/endpoint reads, with a 6-second inner window.

| Observer versus control block means | Rate | P99 interval | Machine CPU points |
| --- | --- | --- | --- |
| PresentMon | -0.01438% | +0.003353 ms | +0.03975 |
| Sampler | -0.02112% | -0.002911 ms | +0.11185 |
| Both | -0.10506% | -0.001343 ms | +0.80431 |

The hypothetical 5% rate / 0.25 ms P99 effect budget yielded predeclared 1% /
0.05 ms / 1-point operational screens. Means fit, but one combined block exceeds
its mirrored control by **1.306 CPU points**. Two blocks/state cannot establish
equivalence. All observer/stimulus exits are 0; CPU/RSS, tail depth, startup+identity,
elapsed excess and read latency are reported with quantization/attribution limits.
The initial mean-only analysis and corrected individual-screen revision are retained.
High-rate row completeness also fails; therefore **no Minecraft overhead trials**
were performed. This is a failed calibration prerequisite, not proof of zero cost.

## 10. Capture completeness and uncertainty

Observed ETW counters are zero through inner windows, not through unobserved
shutdown tails or all driver/consumer processing. Long interior row holes can pass
CSV boundary/order checks; independent reference evidence remains mandatory.
QPC tick/format precision is known; high-rate ETW/API error, drift, GPU error and
physical scanout uncertainty remain unbounded/null. New short review preserves
unavailable/signed fields, independent creation identity and unknown versus zero
loss, rejects mismatched identities/invalid units/physical claims, and labels gaps
without calling them stutters. Every full measured-run gate remains unchanged.

## 11. Environment and preserved invalid observations

Controlled preflight: 60 seconds, AC present, 0.679% mean machine CPU, >=20.79 GiB
available RAM. The first game preflight overlapped loading and is retained as
invalid idle evidence; initial warm-up was interrupted before any trace. Correctly
ordered title preflight: 60 seconds, 2.650% CPU, >=18.72 GiB available, AC present;
the 2% CPU screen fails. No favorable retry or unrelated-app termination.
RX 9070 XT / driver 32.0.31041.1004, desktop 2560x1440/240 Hz; one engine sample
does not prove GPU idle. VRR/thermals/scheduling/overrides remain incomplete.

## 12. Real versus synthetic verification

**74 benchmark tests (65 existing + nine focused) and 13 P1 tests pass**, zero
failures. New tests use explicit synthetic mutations of the retained real CSV
specimen: all-NA display, signed/zero/null, interval mismatch, nonfinite values,
PID/creation identity, QPC/order, interruption/lifetime/loss, malformed external
evidence and unsupported claims;
high-rate opt-in remains bounded. Real WGL/Minecraft/environment/world evidence is
separate. Historical results are not claimed rerun. Rust/frontend/build checks
were unnecessary: no product/startup/native contract changed; the actual reviewed
isolated native binary was booted. [Test evidence](../../performance-p3a/evidence/p3a4a/tests.json).

## 13. Canonical Workload A integrity

[World validation](../../performance-p3a/evidence/p3a4a/world.json): frozen archive
`d385120835fd6b0ad0fb8757601de97b066d9e23023e32733f62e6bd56e13f4f`, inventory
`5082795f0ac5987c1eb61412102a2be5bc7a29868842dd317f985f3193258c61`, options
`efd1595619f579723159ed7bb0cbb1e06d7336dc9980662252162abd15f24ae1` all intact.
Independent restoration/full inventory and post-exit exact zero-entity/block-entity
scene pass. Only maxFps changed temporarily; full options bytes were restored after
each cap and final exit. Camera stayed 0.5/66/0.5, yaw/pitch 0/10. Normal saves may
change the live disposable copy; frozen byte identity and scene equivalence are
distinct. No owner world, canonical frozen bytes, B/C installation or lock changed.

## 14. Remaining blockers and P3-A.4b prerequisites

Isolate missing high-rate ETW records and timestamp fidelity against uniquely
matchable independent timing; establish observer bounds with sufficient independent
counterbalanced trials and a reviewed effect/noise budget; improve environment/
VRR/thermal/scheduling visibility. Review any new instrumentation before installing
it. No invasive instrumentation or alternate tool was installed. Game GC alignment,
equal instrumentation, B/C worlds/gameplay and other hardware remain separate gates.
The unrelated native custom-resolution defect remains deferred; existing saved-game
overrides work. No unrelated product fix was attempted.

## 15. Explicit P3-B verdict

**NOT READY and NOT AUTHORIZED regardless of this result.** No presets, heap/Xms,
collectors, renderer/frame pacer or compatibility were tuned. No five-run JVM
comparison or production behavior change. Stop after P3-A.4a.
