# P3-A test matrix

| Check | Result / evidence scope |
| --- | --- |
| New Python tooling | 36 passed on Windows, including actual Windows API/exit-handle tests; synthetic importer/quality fixtures, not Minecraft |
| Existing P1 JVM analyzer | 13 passed, historical files unchanged |
| Full Rust suite | 986 library + 6 icon tests passed; 31 existing ignored; no failures |
| Frontend settings/instance regressions | 40 passed, sequential; performanceSettings, settings, instanceManagement, instanceStatus |
| Frontend type check | 0 errors / 0 warnings |
| Production frontend asset build | PASS, static build only |
| Development native build | PASS, tauri/custom-protocol; no installer/release build |
| Actual native fresh installation | PASS: normal verified A/B/C Fabric installs, exact managed Java, pinned Aurora/API/Sodium/Lithium artifacts, zero accounts |
| Actual native restart | PASS: schema-6 Balanced, 2048 MiB, empty custom text; deep runtime revalidation |
| Native runtime UI check | PASS: real Ready Java row, account absent, game not running |
| Native shutdown | Four exact retained process handles, exit 0; initial automation close-disconnect handled in helper, no process-name termination |
| Real standalone GC ingestion | PASS: actual default-G1 summaries, shrink-capacity and rounded process-CPU records |
| Actual CPU/memory plumbing | PASS: own-process Windows sampling, actual 30 s global ambient observation; game child capture NOT TESTED |
| Raw frame importer | Synthetic pinned PresentMon/QPC and normalized-unit fixtures PASS; actual capture BLOCKED |
| Minecraft / Aurora / Sodium capture | NOT TESTED; no gameplay or authentication attempted |
| UI-1–UI-5 full browser capture regressions | Existing P2 evidence preserved; not rerun in P3-A because no product UI/source change |
| Linux execution | NOT TESTED; shared format design is not cross-platform validation |

Tests cover percentile definitions/depth, known metrics, duplicates/order/nonfinite
timestamps, explicit units, PID/creation/stream mismatch, dropped-row accounting,
missing/empty/hash-tampered artifacts, traversal containment, incomplete/failure
retention, warm-up/count/coverage/gaps, overshooting sampler endpoints, clock offset/
drift/uncertainty, noncausal proximity, GC shrink and invalid end occupancy,
unknown collectors/malformed logs, schema/required/privacy fields, rejected-data
redaction, cohort/runtime/mod/render/workload drift, duplicate runs/blocks, changed
configuration vectors and legitimate per-run clock/preflight differences.

Small windows deliberately mark P99.9 unavailable. Standalone GC plumbing found
P1's invalid before-used <= after-capacity assumption and motivated a separate P3-A
parser fix; the original P1 tools/evidence remain unchanged. Actual process sampling
found millisecond endpoint overshoot; clipping cumulative-counter intervals now
preserves the final interval with a documented uniform-rate approximation. Neither
fix relaxes hash/identity/coverage tests or removes inconvenient observations.

Private command logs: .git/performance-p3a-{tools-tests,p1-tests,rust-tests,
frontend-tests,type-check,frontend-build,native-build,native-verify-log,
native-restart-log,native-runtime-ui-log}.txt. Raw personal paths/session records
stay private. Sanitized evidence and native screenshots are committed separately.
