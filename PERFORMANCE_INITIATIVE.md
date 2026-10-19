# Aurora performance initiative — Launcher completion record

Completed 2026-10-19 under the owner's end-to-end cross-repository authorization.
No release, push, merge, public artifact distribution or system-wide changes.
Unified report: `C:/Dev/Aurora-Client/PERFORMANCE_FINAL_REPORT.md`.

Baseline `4984010d5309e05ec923535bba066c4e7aa90a5b`, original branch
`codex/client-3-production-authority`, local branch `codex/performance-initiative`.
Original 3,268-file/740,990,316-byte identities and backups remain under
`.git/performance-initiative/20261019T032535Z/`. Final checks found zero missing
files and zero changes to those Launcher originals. Original untracked files and
all historical performance evidence remain outside these commits.

A: reviewed prior P0/P1/P2/P3 evidence and ownership; protected originals.
B/C: Client rejected bounded re-parking on repeated deadline-tail regression and
accepted only the profile-attributed stable world-key cache in Client `9513af4`.
D: existing Rust authority audited in `PERFORMANCE_CONFIGURATION_AUDIT.md`.
E: `JVM_PERFORMANCE_INITIATIVE_RESULTS.md` records four valid G1 processes; retain
2048/default G1/no Xms. Interrupted/short-warmup ZGC data remain ineligible. No
extra replacement or parameter sweep; no measured ZGC performance conclusion.
F/G: `PERFORMANCE_POLICY_DECISION.md` retains the existing advice/manual system,
without unsupported speed/efficiency claims. Native UI persistence, conflict and
recovery checks passed against a fresh test-owned instance copy.
H: `LAUNCHER_OVERHEAD_RESULTS.md` records two complete 60s native/WebView-family
idle captures, 0.078/0.026% of one logical core. No material attributable idle CPU
problem, no production rewrite; debug/working-set/startup limits are explicit.
I: `INTEGRATED_PERFORMANCE_RESULTS.md` records native managed Java21/Fabric0.19.5/
Sodium/OpenGL final Client launch, complete fullscreen gameplay capture and normal
exit0. An induced exact test-child exit-1 was reported; windowed relaunch and
normal exit0 succeeded. Original and copied saved-world preservation verified.
J: unified measured report, local commits and final preservation checks complete.

Verification: 986 Rust library + 6 integration pass, 31 deliberately ignored;
frontend check 0 errors/warnings, frontend/embedded native build pass; 282 frontend
tests pass serially. Parallel frontend run had Vite SSR module-loader timeouts;
failed/pass logs remain in `.git/performance-initiative/validation`. Client548 pass.
No new production Launcher source was changed, so these suites were not repeatedly
rerun for documentation/tooling edits. Tools were exercised on real captures.

Workarounds/limits: missing API in copied dev mods was correctly blocked and fixed
in the new test root; first copy attempt hit MAX_PATH and was retained before a
fresh shorter root; native first-Ready latency remains unaccepted/manual refresh;
Computer Use helper failed second fullscreen targeting after reset, so that title
checkpoint was invalidated and later smoke was windowed. Primary fullscreen data
and options were preserved. No Linux/WSL environment, Vulkan or display-latency
acceptance. Details are explicit in the individual reports; none is a hidden gain.

Private integrated evidence copied under `.git/performance-initiative/integration-evidence/`;
raw dev comparisons under Client `run/jvm-initiative/` (1,340-file capture manifest).
No account/browser/world/private log contents enter tracked reports. The new test
root is retained; all test game/Launcher processes are closed. Existing Gradle JVM
was left alone. Original untracked local files plus generated Python caches stay
uncommitted; no `git clean` or cleanup of owner data.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED** — authorized Client work is committed
in its own repository; no additional justified Client fix follows from these data.
