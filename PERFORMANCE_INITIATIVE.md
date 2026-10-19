# Aurora performance initiative — Launcher execution record

Owner authorization: end-to-end cross-repository initiative, 2026-10-18
(America/New_York). Status ACTIVE; no performance acceptance or release yet.

Unified roadmap and running record: `C:/Dev/Aurora-Client/PERFORMANCE_INITIATIVE.md`.
The owner now authorizes both repositories; historical per-phase stop instructions
remain historical records rather than restrictions on this new initiative.

Baseline HEAD `4984010d5309e05ec923535bba066c4e7aa90a5b`, original branch
`codex/client-3-production-authority`, current local branch
`codex/performance-initiative`. Origin remains the owner's Aurora-Launcher repository.
Private baseline/backup: `.git/performance-initiative/20261019T032535Z/`, 3,268
file identities, including retained performance evidence and protected local files.
No production files, accounts, runtime, instance, release or system setting changed.

Phase A established that P1 already removes a redundant startup-readiness request;
P2 already implements optional persisted Efficient/Balanced/Performance/Custom
advice modes, explicit application, manual controls and bounded memory facts.
Performance intentionally equals Balanced. Do not duplicate this architecture.

P3-B.1 is inconclusive: only one valid pair, a 20.50 MiB RSS reduction versus
72.69 MiB candidate variation, and no demonstrated repeatable improvement.
P3-B.1a camera checks passed but performance timing alignment failed; those runs
remain ineligible for heap conclusions. Retain 2048 MiB, ergonomic initial heap,
managed game-plan-authoritative Java and default G1 until new evidence justifies
a change. The Efficient 1536 MiB advice is a heuristic, not measured superiority.

Client Phase B is complete: two pairs rejected bounded re-parking after repeated
deadline-tail regression; production PARK/500 is retained. Client commit `9513af4`
adds a profile-attributed single-entry world-key cache; 548 tests pass and a
six-process Java 21 diagnostic removes stable-call allocation (no FPS claim).

Phase D audit is complete in `PERFORMANCE_CONFIGURATION_AUDIT.md`; retain existing
authorities and controls. Phase E criteria were committed before preparation in
`JVM_PERFORMANCE_INITIATIVE_PROTOCOL.md`: A/B/C/C/B/A = G1 2048 / G1 1536 /
generational ZGC 2048 MiB, Java-chosen initial heap. Six fresh inputs are prepared
under Client `run/jvm-initiative`. Run 1 is warming up. No primary results yet.
Continue F–J: validate or retain advice policies, investigate Launcher process-family
startup/idle overhead and test the combined system.
No launch-time limiter belongs here. Existing tools under `tools/benchmark/` are
reused read-only for the Client comparison. Physical presentation and input latency
remain distinct from Overlay's internal HUD callback timing.

Current checks: 986 Rust library + 6 integration tests passed (31 explicitly
ignored), frontend check 0 errors/warnings, frontend build and current embedded
debug native build passed. Parallel frontend run had Vite SSR module-loader
timeouts; all 282 tests passed with `--test-concurrency=1`. Both logs are preserved
under `.git/performance-initiative/validation`. This is a local test workaround,
not an accepted production defect or a product change. No heavy work during captures.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED** — the Client work is already authorized
and executing in its own repository; no additional Client source fix is established.
