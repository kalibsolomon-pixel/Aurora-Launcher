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
