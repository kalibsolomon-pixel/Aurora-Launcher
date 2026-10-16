# Performance P1 — JVM audit and benchmark foundation acceptance

**Verdict: PASS for the P1 investigation/measurement foundation. Real Minecraft
baselines: NOT MEASURED. No JVM configuration is approved as a performance win.**
This verdict follows the request's explicit allowance for safely blocked real
benchmarks when the absence/blocker is documented. It is not gameplay performance
acceptance or permission to implement P2. Completed UI phases are not reopened.

## Repository and local commits

Starting HEAD: `8983f938c4792fabbd314a854d612d2eb51f7f46` on
`codex/client-3-production-authority`. The expected branch matched. The reported
UI-5 `a1b3fa8` is its parent; the intervening Modpacks card alignment commit was
already present and preserved. Starting tracked working tree/index were clean;
four protected untracked files existed.

| Local commit | Scope |
| --- | --- |
| `a512d7a` | Offline JVM/G1/frame/process/repetition analysis and deterministic synthetic fixtures/tests |
| `bf72d574560c84334fb42a8fa3125d3935d335c9` | JVM launch/hardware audit, benchmark protocol, honest baseline status, proposed P2 direction and raw sanitized host inventory |
| Commit containing this report | Acceptance record only |

Ending implementation HEAD is `bf72d574560c84334fb42a8fa3125d3935d335c9`.
Ending review HEAD is the acceptance commit containing this file, on the same
branch. Its exact SHA is recorded **after commit** in local
`.git/performance-p1-final.json` and the final chat response; a commit cannot embed
its own content-dependent hash. All commits remain local. No push, merge, tag,
publication, version bump, production install, updater application or release
preparation occurred.

## Deliverables and changed files

- [JVM_LAUNCH_AUDIT.md](docs/performance-p1/JVM_LAUNCH_AUDIT.md): source trace,
  final JVM argument order, custom-policy limitations, runtime/hardware audit.
- [BENCHMARK_PLAN.md](docs/performance-p1/BENCHMARK_PLAN.md): workloads, candidate
  hypotheses, metrics, capture semantics, run order/statistics and hardware matrix.
- [BASELINE_RESULTS.md](docs/performance-p1/BASELINE_RESULTS.md): NOT MEASURED
  gameplay status, actual host observations and blockers.
- [P2_DIRECTION.md](docs/performance-p1/P2_DIRECTION.md): review-only profile and
  native-policy direction with evidence/override/fallback gates.
- [Tooling guide](docs/performance-p1/jvm-benchmark/README.md),
  `docs/performance-p1/jvm-benchmark/analyze.py`, `test_analyze.py`, `.gitignore`:
  standard-library-only offline analysis, tests and private-output exclusions.
- `docs/performance-p1/jvm-benchmark/fixtures/{run.json,frames.csv,gc.log,process.csv,.gitignore}`:
  explicitly synthetic retained raw fixtures; gc.log is intentionally admitted
  through its fixture-scoped ignore exception.
- [host-inventory.json](docs/performance-p1/jvm-benchmark/host-inventory.json):
  sanitized raw built-in Windows hardware observations.
- This acceptance report. **15 added files; zero pre-existing file changes.**

The parent directory already contained a different P1 startup-readiness study.
Its README, scripts and evidence remain intact; new JVM tools have their own
`jvm-benchmark/` subfolder. During implementation, an Add operation unexpectedly
replaced the historical `docs/performance-p1/analyze.py`. Work stopped to investigate;
the new content moved into the new subfolder, and the historical file was restored
through Git's working-tree filters. Its hash and every other original-file hash
match the recorded baseline. The final patch contains no change to that file.

## Launch and settings findings

UI-5 global Minecraft & Java links to the selected instance's existing Settings/
Overview. Instance drafts and expected-original atomic Save remain authoritative.
Launcher config is schema 7 with no JVM preference; registry schema 5 owns desired
per-instance memory/text/window settings. Default maximum is 2048 MiB; inclusive
512–32768 MiB bounds are product limits, not a hardware recommendation.

The actual native order is generated Xmx, applicable Mojang JVM groups, any loader
JVM contribution, Mojang logging argument, custom strings, main class, then game
arguments. Fabric adds no tuning bundle. Aurora generates no Xms, GC selection,
pause goal, worker count or compiler tuning by default. The source-derived exact
Windows x64/Fabric 1.21.11 fixture vector is documented without account/game arguments.
Owner saved custom settings and live process command lines were not read.

Native parsing/save and final assembly reject listed direct heap/classpath tokens;
users adjust maximum via memoryMib and cannot set Xms through supported custom text.
The check is lexical, with no general duplicate/collector/support policy. Argument
files/Flags files/equivalent classpath syntax and native/logging property overrides
are not comprehensively rejected. Custom properties emitted last can override
generated properties; incompatible collector switches can fail startup. This is an
audit finding only; no bypass was executed or production policy changed.

Exact game-plan component/major and official platform mapping select managed Java;
no system/custom Java or architecture fallback. Official manifest/files verify via
SHA-1 store and commit-last staging. Play validates full runtime state and bounded
`-version`, then rechecks content/snapshot/session at the locked spawn boundary.
Structured process arguments and exact-child supervision remain unchanged. Ambient
Java injection variables are scrubbed; logs are bounded and known secrets redacted.
Spawn success is not a main-menu marker.

## Hardware, GC, memory and CPU findings

Existing production recommendation inputs are OS/architecture, resolved Java
requirement and explicit memory. No RAM/free-RAM, CPU topology, GPU/shared-memory,
power/thermal or laptop-aware JVM recommendation subsystem was found. No hardware
dependency was introduced.

Actual built-in inventory: Windows 11 Home build 26300, i7-14700F (20 cores/28
logical processors), 32 GiB DIMMs, RX 9070 XT, existing Ultimate Performance scheme.
Transient free memory is recorded, not converted into a heap recommendation. GPU
VRAM capacity and thermal/AC telemetry are not established. Two pre-existing Java
processes and an Aurora launcher remained running. Their binary/release metadata
show Microsoft 21.0.7.0 / JAVA_VERSION=21.0.7, **not** a complete validated benchmark
runtime build/manifest. They were never attached to, signed into, profiled or stopped.

No measured collector/GC-pause, live-set, allocation, gameplay working-set, paging,
GC CPU or process CPU result exists. Default JVM heuristics remain baseline behavior;
no forced worker counts or universal RAM percentage is recommended. Heap is distinct
from total native/resident memory, and GC pause wall time from GC CPU overhead.

## Measurement foundation and limitations

Three disposable workload classes are defined: lightweight Fabric, representative
Aurora/optimization-mod gameplay, and a demanding scene. All require locked exact
mod/world/route/config/runtime/tool identities before execution. A0 current behavior
and its equivalent default A/A control retain ergonomic initial heap. The explicit
Xms512/Xmx2048 reference is a different initial-heap treatment and needs a separately
reviewed laboratory path. Conservative heap and G1 pause-target hypotheses are
conditional experiments; another collector needs a measured hypothesis, exact runtime
support and a compatible parser.

The protocol separates startup/gameplay, warm-up/measurement and per-run/run-level
statistics. It rotates paired blocks, targets >=5 usable repetitions, preserves
failures/outliers/raw hashes and treats bootstrap intervals as exploratory. Primary
P99/stutter evidence must exceed measured A/A noise and satisfy memory/CPU/startup/
failure guardrails. Five repetitions are not proof of significance.

The offline analyzer validates normalized frame intervals, HotSpot G1 summaries and
process samples, preserves null unknowns, separates synthetic/measured cohorts and
reports repeated-run frame/GC/memory/CPU/startup metrics. It does not capture games,
export JFR, authenticate runtime identity or prove a frame field represents displayed
frames. Optional logging remains benchmark-only. No production profiling dependency
or verbose user logging was added. No gameplay chart was generated from fixtures.

**All workloads/configurations are unmeasured**, including Windows on the local
machine and every Linux/8 GiB/16 GiB/iGPU/laptop class. Blockers: owner workload is
active; presentation capture has not been calibrated/approved for installation;
disposable world/mod/route locks and a dedicated authenticated test instance are not
established; exact full Java build/support remains unvalidated. Linux also lacks
the implemented OS credential store for ordinary authenticated launch. Nothing here
claims cross-platform performance equivalence. No owner world/modpack/private server
was used. Real results are absent by design, not replaced with synthetic/browser FPS.

## Verification

| Check | Result |
| --- | --- |
| New Python analysis tests | **13 passed**, synthetic only, Python 3.12.10; includes missing fields, invalid/nonfinite values, malformed/duplicate GC lines, unit conversion, vendor identity variants, timestamp variants, partial/failed runs, empty data, outliers, percentiles, stutter bursts, cohort conflicts and bootstrap determinism |
| Synthetic CLI fixture | Exit 0; output explicitly `sampleKind=synthetic`, hashed normalized inputs; local summary retained at `.git/performance-p1-synthetic-summary.json` |
| Rust `runtime::` | 21 passed, 3 ignored |
| Rust `launch::` | 34 passed, 8 ignored |
| Rust `instances::settings::` | 14 passed |
| Rust `config::` | 24 passed |
| Rust `instances::tests::` | 19 passed |
| Rust `launch_only_configuration_changes_never_invalidate_content` | 1 passed |
| Frontend `settings.test.ts`, `instanceManagement.test.ts`, `instanceStatus.test.ts` | Three focused files passed, process exit 0 |
| Git whitespace / staged name-status / deletion checks | Passed; additions only; no tracked deletion or staged generated application/game/runtime data |
| New documentation local links | Verified |
| Baseline preservation | All 2315 original tracked and four original untracked files hash-identical; seven worktree identities/paths retained, only this branch HEAD advances |

Rust commands used `cargo test --manifest-path src-tauri/Cargo.toml --lib <filter>
--offline`, without enabling ignored live/stress helpers. Total **113 passed, 11
ignored**, zero failures. Existing unused-import/mut/dead-code warnings were not
modified. Temporary native fixtures retain their existing test safety boundaries.
Python uses `-B` and uniquely created disposable fixture directories; no pre-existing
bytecode/cache was cleaned. Frontend tests use existing fixtures, not account/gameplay
operations. Full native build/format/check, production build and application boot were
not required because no production source, startup/native boundary or dependency
changed. No installer or release build was produced.

Raw preservation/tracked inventory/history/process snapshots are local-only in
`.git/performance-p1-baseline.json`; five subsequent native-filter logs are under
`.git/performance-p1-test-*.txt`. Repeated preservation checks used
`.git/performance-p1-check-preservation.py`, which hashes originals and compares
worktrees without deleting/restoring anything. Protected developer files:
`.zcode-icon-diag.py`, `.zcodeignore`,
`UserskalibAppDataLocalTempaurora-phase-d-final-evidencesodium-exit-zero-ready.txt`,
`docs/performance-p0-2/__pycache__/analyze.cpython-312.pyc`. All remain byte-identical.
Root documentation, manifests/lockfiles, frontend metadata and native build config
remain present and unchanged. Original worktrees were not created/archived/removed.

Production Launcher 1.4.2, instances, saves, mods, account settings/credentials and
release artifacts were not mutated by this work. No Microsoft sign-in, credential
read/copy, private-server connection, system setting change, profiling upload or
process-name termination occurred. The already-running owner's app can perform
its normal writes independently; this report does not claim a byte-level snapshot
of all live owner data. Repository preservation is the independently hash-verified
scope. The only owner Java inspection was read-only binary/release version metadata,
without argument lists or attaching to a process.

## P2 direction and outstanding decisions

Propose Efficient/Balanced/Performance/Custom as **future per-instance native
policies**, conditional on workload/hardware evidence. Balanced starts conservative;
Efficient seeks sufficient heap with lower resource use; Performance requires real
frame-tail improvements, not maximum RAM; Custom retains explicit saved control.
No automatic instance migration or custom-text replacement. Show concrete conflicts
and require explicit confirmation before replacing custom settings. Unsupported
flags/runtime/hardware have deliberate failures or conservative unapplied advice,
not silent fallback/relaunch. No implementation is part of P1.

Outstanding: freeze representative mod/world/route locks; choose/calibrate capture
tools and approve any installation; schedule a quiet resource window; authorize
isolated sign-in if needed; review a typed laboratory Xms path; set effect/resource
budgets from A/A data; secure other hardware/Linux coverage; decide custom-argument
compatibility policy. **Stop after P1.** Owner review precedes any P2 implementation.
