# P3-A / P3-A.1 readiness matrix

Overall **PARTIAL** as of 2026-10-16. P3-B **NOT READY / NOT AUTHORIZED**.
Owner decision: PresentMon 2.3.1 console, separate Windows test user, short capture
only. Owner confirms the test user is **not provisioned**; document setup, do not
create an account. [Acceptance](../../PERFORMANCE_P3A1_ACCEPTANCE.md).

| Gate | Status | Evidence / exact remaining condition |
| --- | --- | --- |
| PresentMon selected and acquired | PASS | [Installation](PRESENTMON_INSTALLATION.md): exact official 2.3.1 portable x64 asset, expected SHA-256 match, valid Intel signature, actual help banner; no installer |
| Presentation calibration | BLOCKED | [Calibration](PRESENTMON_CALIBRATION.md): zero actual presentation rows; tracing/start-stop/PID behavior and controlled OpenGL stimulus untested |
| Windows user isolation | BLOCKED | [Isolation](WINDOWS_TEST_USER_VALIDATION.md): owner says not provisioned; current principal is owner; no separate token/profile/vault demonstrated |
| Authenticated Minecraft | BLOCKED | Legitimate isolated sign-in/entitlement NOT TESTED; no credential read/rotation or Play |
| Minecraft capture | BLOCKED | [Capture report](MINECRAFT_CAPTURE_VALIDATION.md): zero real gameplay rows; no validated target/process/producer loss evidence |
| Workload A | PARTIAL | P3-A installed Fabric/Java evidence retained; [world lock](WORLD_SNAPSHOT_LOCK.md) BLOCKED, no authenticated execution |
| Workload B | PARTIAL | P3-A exact Aurora/API/Sodium/Lithium artifacts retained; world/coexistence/execution BLOCKED |
| Workload C | PARTIAL | P3-A B inventory and proposed heavier scene retained; world/intensity/execution BLOCKED; may remain pending |
| Java runtime lock | PASS | [Prior actual lock](JAVA_RUNTIME_LOCK.md): Microsoft 21.0.7+6-LTS and exact official manifest; must revalidate under test user before launch |
| Native filesystem isolation | PARTIAL | P3-A marked managed/WebView/registry/game root and restarts preserved; that owner's-user root is not authenticated credential isolation |
| GC correlation | PARTIAL | Real standalone G1 + synthetic clock/proximity tests retained/rerun; no game G1, anchors or causal evidence |
| CPU/memory capture | PARTIAL | Read-only self-process API/capture tests pass; no authorized game-child telemetry; heap/paging/GPU/thermals unavailable |
| Artifact quality | PARTIAL | Offline checks/tests and hashes PASS; actual capture manifests/raw sources absent. Full real run INCOMPLETE; short-session schema adapter pending, 300/600 s policy unchanged |
| Snapshot validation tooling | PASS | Read-only complete directory/restore inventory and condition checks; eight synthetic world/isolation tests. No actual world lock/provenance PASS implied |
| Quiet-system preflight | BLOCKED | [Current ambient facts](P3A1_QUIET_PREFLIGHT.md): instant CPU/memory only; no test-session 60 s preflight, AC/GPU/thermals/overhead unknown |
| P3-B protocol prepared | PASS | [Review-only plan](P3B_PROTOCOL.md) retained unchanged; it is not execution approval |
| P3-B authorization | BLOCKED | Explicit separate owner decision required; repeated comparison sets and preset tuning remain prohibited |
| Linux native/capture | NOT TESTED | Separate runtime/credential-store/capture/accounting calibration still required |

Next owner action: provision/select a dedicated standard local Windows test user,
sign into its separate interactive profile and make that session available. Then
verify token/vault/paths before legitimate owner sign-in, confirm tracing permission
without elevation, and select a quiet short-validation window. No new privilege,
software installation or credential action is inferred from tool selection. All
captures/world locks must be reviewed before any separate P3-B decision.
