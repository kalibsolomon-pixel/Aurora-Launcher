# PANDORA-4 acceptance

2026-11-01. **PASS — PANDORA-4 LOCALLY ACCEPTED** within the measured Windows fixtures.
The permitted additive server-list fallback is implemented; server edits remain
explicitly unsupported. This is not a real-gameplay or exhaustive security claim.

Starting full SHA: `1d7c30738569b29fd6a32f85352aadce4ea69813`, clean
`codex/pandora-backend`, in
`C:\Users\kalib\.codex\worktrees\pandora-backend\aurora-launcher`.
Final full implementation SHA: `2f4fe1e9c1e7a3a712e5113ca77d9332a787109a`.
Local implementation commit: **Add conflict-safe syncing, instance overrides and
NBT server additions**. The following documentation-only commit, **Record
PANDORA-4 acceptance and measured resource results**, records this code identity.
Original native and WebView worktrees are outside the implementation boundary.

## Implemented behavior

- Ordinary files use acknowledged per-peer SHA-256 baselines. A single changed
  variant replaces an unchanged peer only if their previous hashes agreed.
  Initial differences and concurrent edits stay local and become pending
  conflicts. Missing files copy only from an unambiguous variant. Deletions
  never propagate; unavailable/paused peers retain their acknowledged state.
- Settings/controls keep their existing compatible-version common-key merge.
  Concurrent conflicts retain baselines across cycles. Explicit choices merge
  the selected key, preserving pack order and unrelated options.
- Native Launcher Settings lists pending differences with instance names,
  **Use first**, **Use second**, and **Defer**. Choices bind both current hashes
  and policy, require stopped/writer-locked instances, and save the original
  before replacement. A changed file or policy invalidates the choice. Defer
  writes nothing; manually converging files clears their pending difference.
- Instance Settings adds sparse **Inherit global / Enabled / Disabled** category
  choices, extra path exclusions and temporary pause. Changes save immediately
  through the existing policy writer. Configuration path selection remains
  global and explicit. Removing participation removes its override. Unknown or
  malformed persisted documents remain untouched; older strict readers reject
  new policy fields rather than silently discarding them.
- Optional `servers.dat` syncing parses bounded uncompressed named NBT. It
  preserves local ordering and raw unknown tags, combines disjoint additions,
  and suppresses duplicates. Ambiguous name/address matches, duplicate identity,
  changed entry fields and malformed/truncated input defer/refuse the file.
  **Server edit/conflict resolution is unsupported**: resolve those in Minecraft.
  Different local ordering alone is not a conflict. No deletions propagate.

All commits reuse the original per-file transaction path, Play reservations,
instance/content locks, cancellation-by-policy checks, reparse/containment checks,
backups and staging. Both copied and merged output are rehashed, then committed
with no-clobber creation or atomic replacement. A late destination creation or
changed source/destination aborts safely. Interruption preserves original or
complete output plus backup; completed files are not rolled back as one batch.

The single minute worker remains lazy and nonoverlapping, with no interval when
globally disabled. File metadata hints/cache are per instance/category. Hints
never authorize a write. File history is capped at 16 MiB, 32,768 peer/path
observations and 1,024 pending conflicts; options retain their 512-key bound per
instance. Full state fails deliberately rather than truncating history. There
is no automatic backup garbage collection or deletion/repair policy.

## Verification

- Core regression: 1,083 passed, 0 failed, **31 existing ignored** (not passes).
- Final focused synchronization/NBT suite: 36 passed, 0 failed, 0 ignored,
  including delayed peers/restart, concurrent/one-sided edits, explicit choices,
  interrupted/stale staging, running-game/writer exclusion, junctions, disabled
  categories, preference/global changes, removal and bounded damaged history.
- Native unit/layout/interaction tests: 42 passed; picker integration: 5 passed.
  The real components render at 720×520, 1120×760 and 1600×1000, including minimum
  size at 100/125/150/200% scale. Conflict buttons dispatch once with pointer,
  Tab and Return. Existing controlled-dropdown and checkbox tests also pass.
- Frontend: 301 passed; type checking: 0 errors / 0 warnings; production web build
  passed. Core/default-host all-target check and both Rust formatting checks pass.
- Optimized core and native acceptance builds passed, with `--locked`, release
  optimization and the explicit `disposable-data-root` native feature. Actual
  native boots, binary identities and resource measurements are recorded below.

The full core suite preceded the last scoped sync fixes; those were subsequently
covered by the final focused suite. No ignored test was enabled or counted as a
pass. Existing unrelated Rust warnings were preserved.

UI scope is policy choices and conflict actions. Existing components, colors and
scrolling are reused; no decorative treatment, framework or dependency was added.
[Before](screenshots/sync-disabled.png) is the prior accepted native
surface. After: [minimum conflicts](pandora4-screenshots/pandora4-conflicts-min-720x520-100.png),
[keyboard focus](pandora4-screenshots/pandora4-conflict-keyboard-focus.png),
[instance settings](pandora4-screenshots/pandora4-instance-overrides-typical-1120x760-100.png).
These after images are headless renders of the real Slint window, not desktop
screenshots. Physical visibility, screen-reader operation and other themes are
NOT MEASURED; no blanket accessibility certification is claimed.

## Performance evidence

Windows 11 build 26300, i7-14700F (28 logical processors), 31.76 GiB RAM,
rustc 1.98.1. Optimized/locked software-renderer builds; one sequential process
per configuration/build. The fixture has 32 identical 1 KiB files per instance,
two common options, one edited file, a concurrent edit, then a new **2 MiB**
synthetic pack (126 MiB total committed at 64). No installed Minecraft game or
accounts are present. Native roots copy the clean fixture before mutations.

Final Engine timings below are **wall ms / process CPU ms**. CPU counters are
quantized; zero is below counter resolution, not proof of zero instructions.

| Selected | Converged | One-sided edit | Conflict | Pack copy |
|---|---:|---:|---:|---:|
| Off | 0.31 / 0.00 | 0.27 / 0.00 | 0.19 / 0.00 | 0.22 / 0.00 |
| 2 | 13.66 / 0.00 | 24.60 / 15.62 | 23.05 / 0.00 | 39.71 / 31.25 |
| 8 | 25.47 / 15.62 | 102.96 / 62.50 | 51.90 / 46.88 | 183.72 / 171.88 |
| 32 | 77.67 / 62.50 | 447.25 / 359.38 | 189.57 / 203.12 | 753.18 / 531.25 |
| 64 | 158.87 / 125.00 | 972.92 / 765.62 | 384.84 / 375.00 | 1537.56 / 1187.50 |

At 64, matched pre-change converged/copy wall times were **750.34 / 4070.84 ms**,
versus **158.87 / 1537.56 ms** after. The 2-instance converged cycle increased
from 9.80 to 13.66 ms. Initial Engine state establishment at 64 was 67.17 s before
and 1.36 s after; at 2 it increased from 70.92 to 146.55 ms. These are measured
synthetic synchronization results, **not application-startup or real-world
throughput claims**. The old edit path keeps differing files; its edit timing
is not equivalent work and is not presented as a speedup. The copy comparison
follows those differing edit states and is not an isolated throughput benchmark.

[Measurement data](pandora4-results.json) records every workload’s
RAM/private memory, CPU, files scanned, full-file hash calls, committed bytes,
logical transfers and duration, plus all six repeated cycles. Raw process I/O
is retained; separate Engine file-transfer counters subtract known command/result
pipe bytes. Native I/O remains **logical process I/O**, not disk-only or physical
disk traffic. Disk-only native counters are **NOT MEASURED**. Pre-change scan/hash
operation counts were not instrumented and are **NOT MEASURED**.

The Engine was blocked between commands, with zero measured idle CPU and I/O
over approximately two seconds in every configuration. This alone does not
establish the automatic worker’s whole-app idle behavior. Native results follow.

Whole-app final samples below use seconds **8–55**, before the first real
minute tick. Every run continues to 72 seconds. CPU is percent of **one logical
core**, not percent of all 28. Working set includes mapped/shared pages.

| Selected | Idle working set MiB | Idle private MiB | Idle CPU/core | Private after tick MiB |
|---|---:|---:|---:|---:|
| Off | 46.95 | 10.62 | 0.233% | 10.53 |
| 2 | 32.27 | 10.26 | 0.133% | 10.22 |
| 8 | 33.11 | 10.66 | 0.133% | 10.77 |
| 32 | 32.64 | 10.78 | 0.067% | 11.50 |
| 64 | 32.90 | 10.94 | 0.333% | 13.30 |

Matched baseline samples at Off / 64 were **45.78 / 33.67 MiB working set**,
**10.44 / 11.29 MiB idle private**, and **10.34 / 14.39 MiB private after sampling**.
The final 64-instance post-tick private sample was **13.30 MiB**. Baseline 64 ran
145 seconds to include its long first cycle; final runs lasted 72 seconds.
Their idle comparisons use the same 8–55-second window. The baseline's last
6.93 seconds consumed 0.015625 CPU seconds, confirming a quiescent tail rather
than a memory sample taken during its expensive initial scan.

The initial final-build disabled working set (**46.95 MiB**) is retained. Its
single justified repeat was **32.25 MiB working set / 10.27 MiB private**.
The first baseline disabled run was also higher than its selected-pool sample;
**the cause is unproven**. Neither first-run value was discarded. Final disabled
CPU measured 0.233% of one core in both runs versus baseline 0.067%; the small
absolute difference is recorded, not attributed or described as zero overhead.
Enabled final idle CPU ranged 0.067–0.333% of one core. There was no busy spin,
additional worker or material private-memory regression in these fixtures.

Acceptance is limited to these controlled runs. No statistical performance or
long-duration leak guarantee is inferred. All native runs booted the actual
optimized app into marked data roots; only their exact owned handles were
terminated after sampling. The baseline fixture omits the new empty `overrides`
field so its older strict reader can load the same effective policy.

The 64-instance Engine process retained 2.47 MiB private memory after initial
convergence; its six post-copy cycles ranged 3.32–4.88 MiB and ended at 3.50 MiB.
The larger samples were retained, not discarded. No unbounded growth was
demonstrated in this short run; this is **not a long-duration leak proof**.

These whole-app runs confirm the explicit disposable root and restored Home
in the native log. The first enabled tick is included; Windows process I/O and
the 58-second-to-end CPU bracket are retained in the data. That bracket includes
UI activity and possibly more than one tick, so exclusive CPU per cycle comes
from the separate Engine probe. Native disk-only I/O, physical disk I/O, network
throughput, physical visibility, gameplay/frame time, real pack-library scale
and long-duration soak behavior are **NOT MEASURED**.

Raw evidence root:
`C:\Users\kalib\AppData\Local\Temp\aurora-pandora4-ee18e646-ee9d-429f-8357-2a08e4c936da`.
Protocol: [PANDORA-4 follow-up](BENCHMARK_PROTOCOL.md#pandora-4-follow-up).
Executable hashes and raw-file hashes are recorded with the measurements.

## Provenance, preservation and limits

Pandora reference: `2d83d6f22c43e804d9b544e8dc98d49d4874e3af`, existing read-only
clone. Inspected [syncing](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/syncing.rs),
[duplication](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/duplicate.rs),
[filesystem](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/fs.rs)
and [NBT decoding](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/nbt/src/decode.rs).
No source was copied, translated or linked; **files adapted from Pandora: none**.
Aurora retains private copies and hash baselines rather than timestamp winners
or shared mutable directories. No dependency or lockfile change was needed.

World syncing remains disabled. A future phase needs consistent snapshots,
exclusive game ownership, backup/restore, region-file validation and crash
recovery; simultaneous shared writable directories remain prohibited.

Game exclusion is process-local: games launched elsewhere or orphaned from an
older session must be closed. Checks defend against reparse points and ordinary
filesystem races, not an actively malicious same-user directory swap. Multi-file
atomicity, deletion propagation and automatic backup cleanup remain out of scope.
The tracked-file baseline has no missing files or deletions. Both original
worktrees retain identical status, tracked-file sets and recorded file hashes;
all three lockfiles remain unchanged. No generated game/runtime/build data is
staged. No installed game, account, world, protected release/signing material or
Aurora Client source was modified. No push, merge, tag, release or deployment occurred.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**. This phase changes Launcher syncing;
no Client-side implementation recommendation is justified by these fixtures.
