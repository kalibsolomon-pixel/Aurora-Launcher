# P3-A.4a.3 practical readiness — PARTIAL

Use the separate [Practical Performance Protocol](PRACTICAL_PROTOCOL.md).
[Actual recordings and acceptance](../../PERFORMANCE_P3A4A3_ACCEPTANCE.md).

| Practical gate | Result | Evidence / limit |
| --- | --- | --- |
| A: recording usability | PASS | Five coherent Overlay CSVs; three readable locked-JDK JFR exports; no reported writer failure/DataLoss |
| A: observer acceptance for heap/CPU comparisons | INCONCLUSIVE | Inactive control game CPU 5.058→6.065%; combined GC frequency differs from JFR-only; no reliable isolated resource-cost estimate |
| B: three comparable stationary intervals | PASS WITH LIMITS | 30 s each after >=300 s warm-up; throughput range 2.339%, P99 range 4.957%; same process, B stack on A scene, no canonical representative B |
| B: resource/restart sensitivity | INCOMPLETE | CPU drift and cumulative RSS growth; no independent JVM restart cohort or general gameplay baseline |
| C: one reversible experiment | DESIGN PREPARED | 2048→1536 MiB only; matched-pair/regression/reversal rules; no candidate executed |
| D: candidate acceptance | NOT RUN | Separate authorization required |
| P3-B | NOT READY / NOT AUTHORIZED | Resolve practical resource-control uncertainty with existing tools before the proposed heap experiment |

This is not a renewed strict-precision requirement: callback diagnostics can screen
coarse pacing effects without optical timing or exact JFR correlation. The remaining
issue is observed resource drift in the very metrics the proposed heap experiment
would evaluate. No failed historical capture is upgraded. Strict reports follow.

# P3-A.4a.2 current readiness (historical)

**PARTIAL. No accepted primary. P3-B NOT READY / NOT AUTHORIZED.**
[Acceptance](../../PERFORMANCE_P3A4A2_ACCEPTANCE.md),
[candidate comparison](P3A4A2_CANDIDATES.md), [next gates](P3A4A2_DECISION.md).

| Gate | Current result | Evidence / remaining condition |
| --- | --- | --- |
| Static released-artifact audit | VERIFIED WITH LIMITS | Exact versions/publisher hashes/JAR metadata/source/bytecode; declarations distinct from runtime |
| Valence primary role | REJECTED | No raw chronological frame export; not installed; runtime UNKNOWN |
| Overlay coexistence | VERIFIED WITH LIMITS | Exact Aurora/API/Sodium/Lithium/Cloth startup/world/HUD/hide key; both game sessions and launcher exit 0 |
| Overlay recording/CSV/completeness | UNTESTED / PROVISIONAL | Failed quiet screen deferred start/stop/export and 240/1000 captures |
| JFR | PROVISIONAL | Exact-JVM attach/check/help/config query; default privacy profile prepared; no actual recording/export |
| Timestamp / GC attribution | UNSUPPORTED precise alignment | Integer-ms CSV/no bounded epoch anchor; aggregate JFR remains a candidate |
| Observer overhead | UNKNOWN | Unchanged 1% / 0.05 ms / 1pp budgets; no independent bound or on/off timing trials |
| Quiet environment | FAILED title screen | Initial 0.887789%; fresh title 2.979357% >2%; further captures deferred without favorable retry |
| Canonical A / production / worktrees | PRESERVED | Frozen archive/options, original evidence, seven worktrees; new disposable B copy only |
| Offline tooling | PASS WITH LIMITS | 97 benchmark tests and 13 P1; 13 new synthetic gates are no empirical collector proof |
| Full-run/representative B/other hardware | INCOMPLETE | Source schema/durations unchanged; no new canonical B lock or portability claim |
| PresentMon role | OPTIONAL DIAGNOSTIC ONLY | Historical overflow/matching/overhead evidence retained; no mandatory prerequisite |
| P3-B | NOT READY / NOT AUTHORIZED | Separate approval plus remaining measurement gates |

Historical matrices follow unchanged.

# P3-A.4a readiness matrix

## P3-A.4a.1 current decision

PARTIAL; P3-B NOT READY / NOT AUTHORIZED. Installed Aurora cap range verified
(OFF 0, integer 1–2000), requested caps supported. No primary operating point;
independent Minecraft completeness and observer bounds remain unresolved. See
[limiter audit](AURORA_LIMITER_AUDIT.md), [matrix](P3A4A1_MATRIX.md),
[completeness correction](P3A4A1_COMPLETENESS.md),
[overhead recovery](P3A4A1_OVERHEAD_RECOVERY.md) and
[metric/prerequisite decision](P3A4A1_METRIC_DECISION.md).

Earlier phase reports below are preserved historical records. Explicit consumer
overflow warnings now make affected P3-A.4a captures INVALID; old permissive
offline quality labels are superseded. Canonical A and production remain protected.


**Overall PARTIAL. P3-B NOT READY / NOT AUTHORIZED.** No accepted primary metric
for uncapped JVM comparisons. [Acceptance](../../PERFORMANCE_P3A4A_ACCEPTANCE.md).

| Gate | Current result | Evidence / remaining condition |
| --- | --- | --- |
| Same-user isolated authenticated execution | PASS WITH APPROVED EXCEPTION | Canonical/reparse/native root/live WebView checks; prior startup-refresh ordering exception and accepted credential-continuity risk |
| Exact tool/runtime | PASS | Pinned PresentMon 2.3.1 hash/Valid Intel signature; exact managed Microsoft 21.0.7+6-LTS; no replacement tools |
| Field semantics | PASS WITH LIMITS | [Pinned source review](PRESENTMON_FIELD_SEMANTICS.md); Other/Composed GPU-copy inference differs from API/optical timing |
| Capped/uncapped diagnostics | COMPLETE WITH LIMITS | [Three bounded captures](OPENGL_PRESENTATION_INVESTIGATION.md); display NA 93.329% / 0% / 1.272%; no physical claim |
| High-rate submission completeness | FAILED CALIBRATION GATE | Independent swaps continue during long ETW row holes despite observed zero session counters |
| Uncapped primary frame metric | REJECTED | Submission and display tails DIAGNOSTIC ONLY; GPU attribution unvalidated; physical scanout unavailable |
| Representative-rate observer bound | PARTIAL | [1800-Hz trial](P3A4A_OVERHEAD_RESULTS.md); one individual CPU screen fails; no equivalence bound; Minecraft overhead prerequisite failed |
| Quiet environment | PARTIAL | Controlled preflight 0.679% CPU/AC; corrected title preflight 2.650% exceeds 2%; GPU idle/VRR/thermals not established |
| Canonical stationary A materials | PASS | Frozen archive/inventory/restoration preserved; saved options restored byte-for-byte after each cap; post-exit zero-entity scene |
| Normal exits | PASS | Exact supervised game and retained-handle launcher both exit 0 |
| Offline capture gates/tests | PASS WITH LIMITS | 74 benchmark and 13 P1 tests; consistency distinct from independent completeness; old actual reader result unchanged |
| Full measured-run quality | INCOMPLETE | Required sources, hashes, 300/600-second durations and existing thresholds unchanged |
| GC/B/C/other hardware | STILL BLOCKED OR UNTESTED | No hidden logging, new worlds, preset/JVM comparisons or portability claims |
| P3-B | NOT READY / NOT AUTHORIZED | [P3-A.4b prerequisites](P3A4A_QUALITY_GATES.md) plus separate owner authorization |

Historical matrices follow unchanged.

# P3-A.3 readiness matrix

**Overall PARTIAL. P3-B NOT READY / NOT AUTHORIZED.** Corrective A-only work is
complete; high-throughput timing/observer confidence remains an essential gate.
[Acceptance](../../PERFORMANCE_P3A3_ACCEPTANCE.md), [authorization](AUTHORIZATION.md).

| Gate | Current result | Evidence / remaining condition |
| --- | --- | --- |
| Same-user/account scope | PASS | Explicit continuation and accepted credential-rotation risk |
| Canonical/reparse/native WebView isolation | PASS | Checks before both starts; immediate live profile checks afterward |
| Startup authentication ordering | APPROVED EXCEPTION | Home widgets may refresh first; owner reviewed and authorized this limitation |
| Exact PresentMon / unelevated operation | PASS | Pinned 2.3.1 hash, Valid Intel signature, unique bounded traces |
| Fixed native 30/60/120 Hz calibration | PASS WITH LIMITS | 20 s inner windows, independent QPC swaps/rows reconciled; physical scanout unproved |
| Loss counters | PASS WITH LIMITS | Read-only ETW queries show zero through measured windows; shutdown tails unknown |
| Instrumentation overhead | PARTIAL | Alternating 120-Hz screens pass; two blocks/state do not bound uncapped Minecraft interference |
| Unintended 30 Hz limiter | PASS | Focused AFK reproduction after >60 s; Minimized controls it without input simulation |
| Canonical A world | PASS | Zero entities/block entities after save/reload, exact scene, complete chunks, archive restoration |
| Final restored-A short capture | VALID WITH WARNINGS | 46170 rows, exact child/focus, 342.978 s warm-up; no AFK cap; high NA fraction/estimate gaps retained |
| A frame-tail measurement fidelity | BLOCKED | Uncapped display estimates/observer effects not established physical gameplay tails |
| Quiet preflight | PASS WITH LIMITS | Fresh 60 s / 121 samples, 1.232% mean machine CPU, AC present, >=19.68 GiB available; GPU/thermals/VRR unknown |
| CPU/memory telemetry | PASS WITH LIMITS | 61 samples, full 20 s counter window; capture machine CPU 14.677% includes game/OS/observer and background |
| Clean final exits | PASS | Exact supervised game and retained-handle launcher both exit 0 |
| Custom-resolution product defect | DEFERRED | Saved-game overrides remain validated workaround; native resolver unchanged |
| Game GC / correlation | NOT AVAILABLE | Balanced/2048 MiB/empty JVM text retained; no hidden logging or validator bypass |
| B/C gameplay/world readiness | NOT TESTED | Installation evidence preserved; no inference from A-only success |
| Full measured-run quality | INCOMPLETE | Existing 300/600 s, three-source and clock/quality policy unchanged |
| Regression tests | PASS | 65 benchmark (52 existing + 13 focused), 13 P1; product source untouched |
| Other hardware / Linux | NOT TESTED | No equivalence or portability claim |
| P3-B decision | BLOCKED | Separate authorization plus calibration/GC/representative-world gates required |

No preset, heap/collector policy, native DTO, production dependency, release or
global setting changed. Original P3-A.2 evidence remains intact. Historical matrix follows.

# P3-A.2 readiness matrix

Current phase: same-user short validation only, 2026-10-16.
P3-B **NOT READY / NOT AUTHORIZED**. [Authorization](AUTHORIZATION.md) supersedes
P3-A.1's mandatory separate-Windows-user gate; earlier reports are historical.

| Gate | Status | Evidence / remaining condition |
| --- | --- | --- |
| Same-user authorization | PASS | Explicit credential-continuity risk acceptance; legitimate owner-operated login only |
| PresentMon identity and current permissions | PASS | Exact staged 2.3.1 SHA-256 and Valid Intel signature; unique bounded ETW sessions exit 0 without elevation/service |
| Controlled OpenGL operational capture | PARTIAL | Actual rows, exact PID/QPC/swap markers and separate default-CSV parser; full cadence and overhead calibration pending |
| Managed native filesystem isolation | PASS | Test-tree/ancestor reparse checks, actual reported native root, live WebView profile and canonical production-root disjointness |
| Pre-authentication observation ordering | PARTIAL | Owner independently completed sign-in during revalidation; final live WebView confirmation followed login |
| Authenticated entitlement/session | PASS | Supported refresh/session/readiness commands return ready, no blockers, no account database copying or token export |
| Prepared A/B/C content and Java | PASS | All three native deep validations Ready; exact shared Microsoft 21.0.7+6-LTS lock, 402 files / 99,041,788 bytes each |
| Native custom-resolution option | BLOCKED | Normal launch with 1920x1080 configuration rejects unresolved resolution_width; retained failure; product source unchanged |
| Workload A authenticated execution | PASS | Normal default-window launch; Minecraft saved overrideWidth/Height supplies verified 1920x1080; exact Java/Fabric 1.21.11 identity |
| Workload A short capture and world | PARTIAL | Real 776-row capture / 598 inner intervals VALID WITH WARNINGS; archive restoration VALID; world has 19 entities rather than zero and pacing is near 30 Hz |
| Workloads B/C gameplay | NOT TESTED | Content/runtime revalidated; B optional, C deferred; no coexistence or demanding-scene execution claim |
| Quiet short-session preflight | PASS WITH LIMITS | 60 s CPU/RAM/AC screen, owner-quieted apps; one GPU idle observation; thermals/VRR unknown |
| CPU/memory instrumentation | PASS WITH LIMITS | 61 real exact-child samples, complete 20 s counter window, no owner process sampling; background activity rose during capture |
| Game GC/correlation | NOT AVAILABLE | A remains Balanced; no GC flag injection or validator change; no separate laboratory instance |
| Full measured-run quality | INCOMPLETE | 300/600 s and three-source policy unchanged; short calibration is a distinct artifact |
| Regression tests | PASS | 52 benchmark tests (original 44 + eight focused regressions); 13 P1 tests |
| P3-B authorization / readiness | BLOCKED | Separate approval, full calibration/overhead/loss review, representative workload locks and instrumented protocol still required |
| Linux / other hardware | NOT TESTED | No portability/performance equivalence claim |

No presets, collector/Xms policy, product dependencies, native command DTOs, releases
or production installation changed. See [P3-A.2 acceptance](../../PERFORMANCE_P3A2_ACCEPTANCE.md)
for final capture/snapshot status and preservation limits. Successful short capture
is not permission to start repeated comparisons.
