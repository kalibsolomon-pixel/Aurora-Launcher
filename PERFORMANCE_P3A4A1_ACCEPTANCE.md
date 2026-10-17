# Performance P3-A.4a.1 acceptance

## 1. Verdict

**PARTIAL. P3-B NOT READY / NOT AUTHORIZED.** Installed Aurora 3.0.0 supports all
requested custom caps. Consumer-buffer overflow is now a supported explanation
for high-rate record loss, with a bounded controlled capacity confirmation.
Independent Minecraft completeness and observer-interference bounds remain
unestablished. No primary operating point or JVM recommendation change; no
performance improvement claim.

## 2. Repository and commits

Branch `codex/client-3-production-authority`, starting full HEAD
`fd9f1a7efc8380082d89752fc39432786b89b903`, exactly as requested. Initial tracked
tree/index clean; 2500 tracked files, four protected developer-local artifacts,
seven existing worktrees. Baseline status/history/index/full original hashes,
private evidence and critical/production identities are recorded privately in
`.git/performance-p3a4a1-baseline.json`.

One scoped local commit: **Validate Aurora limiter diagnostics and reject
PresentMon consumer overflow**. Ending review HEAD is the commit containing this
report; full SHA is recorded after commit in `.git/performance-p3a4a1-final.json`
and the final chat response. A commit cannot embed its own content-dependent SHA.
No push, merge, tag, publication, deployment, production installation, release,
workflow or version bump.

## 3–5. Changed files, deletion and preservation

Exact reviewed path inventory: [P3A4A1_CHANGED_FILES.txt](docs/performance-p3a/P3A4A1_CHANGED_FILES.txt):
**33 paths, 25 additions, eight modifications, zero file deletions.**
Eight scoped originals: four requested current reports, short-format/tool guides,
shared short reader and its regression test. New pure limiter review/tests,
protocol, focused reports, sanitized evidence and this acceptance. Zero file
deletions, renames, generated binaries or game/runtime data staged. Product
Rust/Svelte/TypeScript, native commands, startup, dependency files, versioned
configuration schemas, JVM presets, renderer, measured importer/schema and
canonical workload locks are unchanged.

[Preservation audit](docs/performance-p3a/evidence/p3a4a1/preservation.json): 2492
unmodified original tracked files plus all four protected developer files remain
byte-identical; all 518 prior private evidence files and 11 critical files remain
byte-identical. Seven launcher worktrees and six other HEADs preserved. Separate
Aurora Client HEAD `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d`, all 422 tracked
files, index/status and worktrees unchanged. No client build or source mutation.
Production Launcher 1.4.2 binary/config/registry hashes match their baseline;
this limited audit is not an owner-world or credential inventory.

Canonical A archive `d385120835fd6b0ad0fb8757601de97b066d9e23023e32733f62e6bd56e13f4f`,
inventory `5082795f0ac5987c1eb61412102a2be5bc7a29868842dd317f985f3193258c61`,
options `efd1595619f579723159ed7bb0cbb1e06d7336dc9980662252162abd15f24ae1`
preserved, along with the original 1.873352-second/3356-swap discrepancy. New A/B
world copies derive from that frozen restoration; no final canonical B lock.
A live options restored byte-for-byte after controls. B global cap restored OFF
after diagnostics. Post-exit NBT/inventory checks are separate from provenance.

## 6–8. Limiter, range, activation and isolation

[Audit](docs/performance-p3a/AURORA_LIMITER_AUDIT.md): installed bytecode, rather
than source comments, confirms OFF 0, positive integers 1–2000; all requested caps
supported. Source/build class equality did not establish exact build provenance.
PARK parks remaining time minus spin tail then spins; anchored target progression
and late reanchoring exist, but no measured overshoot estimator or CPU-load
adaptation. GLFW monotonic clock, frame-head scheduling before input/render;
simulation tick-rate constant is not changed. Separate native limiter, AFK/
minimize, VSync and GUI constraints can still apply. Adaptive sleeping can impose
the native 260 sentinel, so actual false is a required interaction gate.

B successfully reached Aurora main menu and the disposable world with exact
Minecraft 1.21.11/Fabric 0.19.5/Aurora 3.0.0/Fabric API 0.141.6+1.21.11/
Sodium 0.8.14+mc1.21.11/Lithium 0.21.4+mc1.21.11, managed Microsoft Java 21.0.7+6-LTS.
Jar hashes are in [scene provenance](docs/performance-p3a/evidence/p3a4a1/scene-provenance.json).
No substituted modpack. Actual configuration: PARK, smooth true, spin 500 µs,
park 2000 µs, lowCPU false, lowLatency true, adaptive false, disableVSync false,
GUI 60; native Unlimited, VSync off, Minimized inactivity. B captures A1–A5 were
deferred after the background review. A non-capture UI/persistence check entered
1000 and restored OFF; no Aurora pacing accuracy is inferred from it. Defaults,
observed live configuration and retained settings are separately recorded.

Native path/reparse checks and exact launcher/tool/runtime hashes passed. The
same tool bytes had a prior verified valid Intel signature; the fresh phase
signature check occurred during the first WGL cadence, after its pre-execution
hash check. Subsequent trials and every game attempt followed that fresh Valid
check. This ordering limitation accompanies the first exploratory cadence.
Live WebView command-line evidence matches the isolated root;
as documented previously, this is not independent proof of every WebView file
access. Normal Rust-owned auth refresh/readiness/play used the authorized same
Windows account; accepted credential-continuity risk persists. No secrets
inspected/exported or committed, no entitlement bypass or direct Java launch.

## 9–10. Complete diagnostic matrix and actual intervals

[Matrix and interval analysis](docs/performance-p3a/P3A4A1_MATRIX.md),
[full sanitized measurements](docs/performance-p3a/evidence/p3a4a1/minecraft.json).
Two initial bounded control captures C1/C2; A1–A5 deferred for background
interference. No controlled native-versus-Aurora timing comparison was completed.
Both controls use a 16-second producer,
10-second inner window; stationary ≥300-second initial world warm-up, debug off.
No repetitions/JVM comparisons/heap or collector sweeps. 1920×1080 window,
12/8 distances, seed/time/weather/camera/entities retained. B adds Aurora HUD,
Sodium rendering and Lithium simulation behavior; A/B absolute FPS is not a
same-mod comparison. Camera drift from menu mouse interaction was corrected via
ordinary in-game teleport; no shifted-camera capture was taken. F3 is a spot
configuration check, not an independent continuous frame source.

Mathematical ideals 240/500/750/1000: 4.166667/2/1.333333/1 ms. No actual Aurora
cap intervals are available. Matrix records the controls' raw
ETW rate, median/P95/P99/high tails, variance, signed target errors, long gaps,
display-estimate availability, CPU/RSS and background context. These are observed
channel diagnostics, not uniquely attributed limiter error or physical FPS.

## 11–12. Completeness and independent timing

[Completeness review](docs/performance-p3a/P3A4A1_COMPLETENESS.md): prior review
missed explicit consumer overflow warnings; prior uncapped Minecraft reports
30,711 overflowed events. A failing synthetic regression preceded the shared
reader fix. Affected old captures now classify INVALID; original raw bytes and
reports are retained and their permissive labels explicitly superseded.

New controlled 240/500/750/1000 aggregate counts nearly match, but unique nearest
association fails. Targeted default 1800 reports 8343 overflowed events, inner
29,876 rows versus 35,796 swaps and 775.1731 ms maximum row gap. Reduced tracking
exited 0 but emitted no CSV; failed analysis and UNKNOWN unpersisted ETW snapshots
remain visible. One same-tool full-tracking 8192-buffer confirmation recovers
35,821 rows / 35,821 swaps, maximum 1.8187 ms, no >50 ms gap or overflow warning;
1,556 ambiguous nearest assignments still prevent timing certification. ETW zero
counters cannot exclude consumer ring loss. No independent events inside
Minecraft; WGL evidence never certifies it. No renderer hooks manufactured.

Retained capture records include exact PID/creation identity, UTC/QPC anchors and
frequency, stream/modes/all fields, raw hashes, startup/exit/coverage, NA display
estimates and available ETW counters. Missing fields/lost snapshots/shutdown tail
remain UNKNOWN. No missing display row is automatically a dropped frame or stall.

## 13. Observer interference and quiet conditions

[Recovery report](docs/performance-p3a/P3A4A1_OVERHEAD_RECOVERY.md): unchanged
absolute budgets 1% rate / 0.05 ms P99 / 1 CPU pp. Counterbalanced 500-Hz means pass,
but both2 versus mirrored control is −1.062884 CPU pp; off/off drift +0.812591 pp
limits attribution. Retain failure; no outlier exclusion, zero-cost or equivalence
claim. Independent matching also unresolved. Minecraft observer trials deferred.

60-second quiet observations: CPU 0.7022%, 0.9580%, 1.0277%, AC, minimum available
RAM above 22.1 GB. Unchanged RX9070XT driver 32.0.31041.1004, desktop 2560×1440/240Hz.
Post-capture estimated background activity is 2.259976% at C1 and 4.771219% at C2,
above the 2% quiet screen. Initial idle passage does not clear this later evidence:
both controls are confounded, C1 phase quality INCOMPLETE despite its offline
warning-only result, C2 additionally INVALID with 28,259 consumer-overflowed
events. Further captures were
deferred under the owner's interference stop rule, without favorable retries.
GPU aggregate engine sample during A warm-up is context, not quiet-idle/per-game
measurement. VRR, thermals and complete background stability remain unknown.
No unrelated-process termination, global timer/priority/power/display change.

## 14–17. Metrics, operating point, prerequisites and P3-B

[Metric decision](docs/performance-p3a/P3A4A1_METRIC_DECISION.md) classifies
submission intervals/throughput/tails, available display estimates and GPU timing
DIAGNOSTIC ONLY; CPU/RSS observations SECONDARY context with comparative effect
unvalidated; physical scanout UNAVAILABLE. No PRIMARY accepted. Capped pacing
can conceal maximum throughput/JVM differences; regular output or highest FPS
alone cannot select a benchmark cap. **No reliable primary operating point.**

P3-A.4b prerequisites: target-specific independent completeness, unique matching
and timestamp error, controlled observer bounds, capped sensitivity/headroom,
representative canonical B lock, unresolved environmental/full-run gates and
separate authorization. **P3-B NOT READY / NOT AUTHORIZED. Stop after P3-A.4a.1.**

## Verification and retained evidence

Original 74 benchmark and 13 P1 tests passed before changes. Final 84 benchmark
tests (ten focused new regressions) and 13 P1 tests pass. Synthetic fixture results
are not actual Minecraft/WGL evidence. Coverage includes unsupported/missing
config, native/adaptive confounds, identity/QPC units/order, row loss/NA display,
unknown loss/incomplete captures, interval arithmetic, observer failure,
canonical drift and invalid metric promotion. No product tests required because
no product source changed. One benign existing unclosed-fixture ResourceWarning
does not change test success. An initial final P1 command used a nonexistent
discovery directory and failed before tests; its log was retained, then the
correct documented P1 directory ran all 13 tests successfully.

Reviewed sanitized summaries/raw hashes are committed under
`docs/performance-p3a/evidence/p3a4a1/`; raw captures, logs, worlds, disassembly,
account-bearing material and local absolute paths remain private in the isolated
test root or `.git`. Failed attempts retained; no cleanup or destructive reset.
