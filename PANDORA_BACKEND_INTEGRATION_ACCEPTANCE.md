# PANDORA backend integration acceptance

2026-11-01 — **PARTIAL — IMPLEMENTATION IN PROGRESS**.

Historical phase record. PANDORA-4 extends file updates, sparse overrides and
server-list additions; see [current acceptance](PANDORA_4_ACCEPTANCE.md).

The source investigation and the selective improvements below are implemented.
Important parts of the larger initiative remain unfinished. This is not a full
feature-parity, security-certification, gameplay or owner visual-acceptance claim.

## Source identities and working boundary

- Aurora starting source: `0e2c8dd9e13ea1b1d548505d575674d9114ee386`.
- Original native checkout: `C:\Dev\aurora-launcher-native`, branch
  `codex/native-ui-migration`, initially clean and preserved.
- Implementation: `C:\Users\kalib\.codex\worktrees\pandora-backend\aurora-launcher`,
  branch `codex/pandora-backend`.
- Pandora studied: `2d83d6f22c43e804d9b544e8dc98d49d4874e3af`, upstream
  `origin/master`, subject `Add instance groups`; detached read-only clone at
  `C:\Dev\aurora-pandora-evidence-20261101\PandoraLauncher`.
- Initial selective integration commit:
  `884525370e1a14673adda30639149130dba50cf0`.
  Its manual sync interface was subsequently superseded by the owner's explicit
  selection-and-minute-sync correction. It is retained in history as evidence,
  not presented as the accepted current workflow.
- Ending implementation commit:
  `65ab258a69c74a865f38c140546b3fbca2b923eb` — automatic selected-instance syncing.
  The documentation-only completion commit follows this implementation identity.

Full source inventory, implementation-level comparison, scoped security
considerations and provenance: [architecture audit](docs/pandora/PANDORA_ARCHITECTURE_AUDIT.md).

## What changed for users

### Automatic selected-instance syncing

Settings now offers **Automatic syncing**, participating-instance checkboxes,
category checkboxes, optional exclusions and explicit configuration paths.
Select at least two instances. Their selected content syncs **once every minute
while the launcher is open**, without Review or Apply. Selection persists across
launches. Disabled syncing has no interval worker. Changing selection resets
the countdown; missed ticks are skipped rather than accumulated and cycles never
overlap. There is one selected pool, bounded to 64 instances.

Games supervised by this launcher, Play preparations and busy instance writers
are excluded. Stopped peers may continue syncing, and an options edit stays
pending for a previously observed compatible peer that is temporarily unavailable.
The launcher does not identify orphaned games from prior sessions or games
started by other launchers; those games must be closed before syncing.

| Content | Implemented automatic behavior |
|---|---|
| Game settings | Common mouse/accessibility/chat/audio keys, matching exact installed Minecraft and loader pins |
| Controls | Common `key_` bindings, with the same compatibility check |
| Resource/shader packs, screenshots | Private copies of missing files; identical files skipped; differing existing files preserved |
| Selected configuration | Explicit `config/...` files/directories only; additive copies, exact compatible pins; existing differences kept |
| Pack enablement/order, unknown/version-specific option keys | Remain local |
| Worlds, server lists, deletion/replacement propagation | Unsupported/off |

The first options cycle establishes a baseline and preserves initial differences.
Later unique edits transfer to unchanged peers. Simultaneous different edits
remain local and appear in the status count. This is conservative synchronization,
not latest-timestamp overwrite. Ordinary file updates that would replace an
existing differing file deliberately remain unresolved; no conflict-resolution
or world-recovery interface is claimed.

The Rust worker reuses internal hash-bound plans and per-file commits. Strict
schema-2 preferences live in `launcher/instance-sync.json`; a bounded hash-only
baseline lives in `launcher/sync-observed.json`. The known manual schema-1 file
loads off/unselected until configured; malformed or unknown documents remain
untouched. Settings values are not persisted in the hash-only baseline. Original
options and a reviewed inventory are saved under `launcher/sync-backups` before
replacement. Multi-file work is not one atomic transaction; completed files and
backups remain available after interruption.

Traversal, path lengths/depth, file counts/sizes and total source bytes are
bounded. A minute cycle copies at most 1 GiB. Unchanged folder metadata hints
avoid repeat hashing after convergence; actual copies still rehash full plans.
Hints never substitute for artifact/launch integrity. There are no directory
junctions shared between instances, watchers, dedicated threads or network
polling. Disabling checks policy before subsequent commits; an entered atomic
file commit can finish. Path checks are defense in depth, not a proof against
an actively malicious same-user directory swap.

### Preparation, transfer and archive work

- Native Home offers **Prepare & play** when the sole blocker is the missing
  exact managed Java requirement and account/instance/process checks permit it.
  The existing verified installer fills that requirement, then the existing
  final launch validation and supervisor remain authoritative. A writer-locked
  missing-only policy refuses damaged/unknown runtime replacement. Custom Java
  selection never silently falls back.
- Host/digest slot leases now release idle coordination entries, including
  cancelled digest waiters. This fixes lifetime retention without an expiry
  task or another cache.
- One order-independent modpack provider-verification pass uses a bounded
  completion-order window to refill behind completed jobs. Ordered artifact
  vectors, manifests and classpaths keep their existing order.
- `.mrpack` review hashes overrides with a 64 KiB buffer instead of allocating
  each complete expanded override. Size/expansion/read/CRC failures remain
  enforced. Actual extraction still uses its existing bounded allocation and
  transactional writers; fully streaming extraction is unfinished.

## What the comparison established

Pandora has an implemented reflink/content-library-aware duplication path,
broader provider/import support, OS secret-store coverage and optional sandboxing.
Its joined launch preparation and category-aware sync are useful references.
Aurora retains stronger boundaries in the inspected verified-artifact promotion,
digest provenance, normalized plans and durable ownership/recovery paths.
These are scoped architectural findings, not an overall security ranking.

No Pandora code was copied, translated or linked. The changes are independently
authored. Pandora's root MIT notice names Moulberry (2025); vendored
`reqwest_client` carries Zed's Apache license and `ftree` a separate MIT author.
No incorporated upstream code requires a new distribution notice in this slice.
No production dependency was added; existing Tokio test utilities were enabled
for virtual-clock tests. Lockfiles and release authority are unchanged.

Not adopted: GPUI/framework replacement, timestamp-winner sync, mutable shared
world folders, arbitrary external links, mutable-file hardlinks, unbounded
metadata/archive buffers, permissive damaged-config defaults, borrowed OAuth
identity, authentication bypass or Client changes. A brief temporary link used
to commit a completed private copy never links two instances or a cache object.

## Measurements

Host: Windows 11 Home build 26300, i7-14700F, 28 logical processors,
approximately 31.76 GiB visible RAM; rustc 1.98.1. Release/locked builds use
the software renderer and explicit disposable roots. Raw samples, exact
executable hashes, all repetitions and outliers are retained in the external
evidence directory. [Protocol](docs/pandora/BENCHMARK_PROTOCOL.md) and
[initial results](docs/pandora/benchmark-results.json) describe the workloads.

| Controlled probe | Before → selective implementation median | Interpretation |
|---|---|---|
| Five reviews of one Stored ZIP with a 32 MiB override, three process runs | 220.90 → 93.30 ms; peak working set 69.23 → 4.30 MiB; sampled peak private 65.84 → 0.75 MiB | Planning/hash allocation reduction; not installation/download throughput |
| Same archive CPU and logical reads | 0.21875 → 0.09375 CPU seconds; 160.01 MiB logical reads in both | Less allocation/copy work; no disk-read reduction claim |
| 50,000 distinct host permits, no HTTP, three runs | Peak working set 11.02 → 4.07 MiB; sampled peak private 7.81 → 0.66 MiB | Coordination-retention stress test; not typical idle use |
| Same host probe wall time | 12.05 → 12.12 ms | No speedup established |
| 64 delay-only jobs, width 8, one 400 ms head job, six alternating pairs | 624.90 → 406.03 ms | About 35% less synthetic head-of-line delay; not internet throughput |
| Initial manual-era native idle, five fresh-root runs/build | Working set 31.41 → 31.35 MiB; private 10.16 → 10.09 MiB | Historical selective-build measurement; final automatic build measured separately |
| Same initial idle CPU | 0.046875 CPU seconds over about 12.02 seconds in both | About 0.39% of one core; no idle CPU reduction established |

The initial after-build 45.33 MiB working-set outlier is retained, not discarded.
Its cause is unproven. Internal `home-state-loaded` medians were 28 → 31 ms;
this is not physical visibility or Ready and establishes no startup improvement.
Sampled private peaks can miss short peaks; logical I/O is not physical disk I/O.
Archive/host/scheduler probes perform no network by construction.

Final automatic-build idle and enabled-minute resource measurements are
**NOT MEASURED**. The owner asked to move on after the optimized build completed;
additional resource runs were deferred. The historical archive/host/scheduler
implementation is unchanged by the automatic UI correction. No negligible-
overhead or final idle-regression pass is inferred from the earlier manual build.

Pandora runtime/RAM comparison: **NOT MEASURED**. A substantially lower footprint
than Pandora is not established here. Actual cold/warm Minecraft installation,
second-instance install, live modpack throughput, real duplication, populated
and Borealis idle, GPU/frame-time consistency and public-CDN network throughput:
**NOT MEASURED**. Deterministic loopback tests demonstrate behavior, not these
performance deltas.

## Verification, UI and security evidence

The completion record contains final command outcomes and executable identity.
The regression suites cover installers/downloads, verified caches, filesystem
recovery, accounts, launch assembly/readiness and existing frontend behavior.
Sync-specific cases exercise default-off/schema rejection, common-key merging,
pack-order preservation, backups/interruption, no-clobber copies, exclusions,
malformed options, unknown IDs, incompatible pins, running/starting games,
content writers, stale policies and source/destination/backup junctions.
Automatic cases cover three selected instances and an unselected peer, unique
and simultaneous edits, independent edits from both sides, delayed peers,
hash-only history, legacy policy authority, real cycle scheduling under a
virtual clock, repeated minute ticks and timer removal on disable.

Missing-Java tests use local loopback metadata/artifacts: cold acquisition needs
two requests, warm reuse zero, and damaged installed state is refused rather
than repaired. Secret/session/launch DTO boundaries and updater authority are
preserved. This was not an exhaustive Codex Security scan.

The native UI reuses Group, SettingRow, Toggle, Checkbox, TextField and theme
tokens. It adds no decorative system or new renderer. Directional selectors,
Review and Apply were removed. Instance rows use a bounded scrolling list;
checkbox models retain identity while saved values update. Current screenshots
and actual UI-run outcomes appear in the completion record. Older `sync-*`
screenshots preserve the superseded manual design and are historical only.
No supported light theme exists in this checkout; no light-theme or comprehensive
screen-reader/high-DPI acceptance is claimed. Owner visual acceptance remains
pending review.

Actual authenticated Minecraft gameplay was **NOT PERFORMED**. The automated
sync fixture started with two synthetic registry entries and options, with no
installed game, account or world. No sign-in or credential UI was automated.
The owner subsequently interacted with that disposable window, including account
state; its retained data is not copied into benchmark roots. Later keyboard and
resource checks use a fresh synthetic fixture, seeding only registry/options.
These checks cannot establish real gameplay acceptance.

## Preservation and remaining work

All 3,213 initially tracked files remain present. The dirty main checkout's
status and binary diff match the initial record. The original native checkout
remains clean, and its 13 critical-file hashes match. Name-status and deletion
diffs are inspected before commits. Generated games/runtimes, build caches,
credentials and evidence executables are not staged. No `.minecraft`, user
world, production release authority, key or Aurora Client tree was modified.
No push, merge, tag, release or deployment was performed.

Recommended subsequent phases, not authorized experiments or implementations:

1. Explicit conflict-resolution and per-instance category overrides, followed
   by existing-file update semantics with documented recovery.
2. Separate NBT-aware server-list and world snapshot ownership/recovery design.
3. Opaque-ID duplication with completion markers and measured reflink fallback,
   retaining private mutable content.
4. Measure real cold/warm install and metadata request duplication before adding
   more caching, indexing or concurrency. Extend missing-only preparation only
   when ownership permits it.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**.

## Completion record

### Regression results

| Check | Current outcome |
|---|---|
| Core `cargo test --no-default-features` | 1,068 passed, 31 deliberately ignored; 6 integration tests passed; doc tests empty |
| Sync-focused tests | 20 passed, including repeated 60-second ticks and pending edits across a running third peer |
| Native `cargo test --all-targets` | 42 main + 5 picker + 24 example tests passed (71 total) |
| Core default-feature and native `cargo check --all-targets` | Passed |
| Both Rust format checks and `git diff --check` | Passed |
| `npm run check` | 0 errors, 0 warnings |
| `npm run build` | Passed |
| Serial frontend regression suite | 301 passed |

Logs are named `automatic-core-accepted.log`, `automatic-tests-accepted.log`,
`automatic-native-accepted.log`, `automatic-*-check-accepted.log`,
`automatic-frontend-{check,build,tests}.log` in the external evidence directory.
The native test count includes existing example harnesses, not 71 independent
production UI tests. Existing Rust test-only warnings remain.

Failures encountered and resolved are retained in the logs: an early native
API used a private error field; the new Slint list initially lacked its standard
widget import; both were corrected and rebuilt. Earlier frontend default-parallel
tests had nine Vite optimized-dependency race failures; serial reruns passed all
301 before and after the automatic correction. A baseline executable-copy race
with Cargo caused a Windows sharing error; serial build/copy fixed the harness.
No failure is hidden by deleting its log or weakening a check.

### Actual native behavior

At 1100×760, native Settings enabled automatic syncing and saved both participant
checkboxes. A newly created synthetic screenshot fixture was absent in the
destination at `09:03:42Z` and present by `09:04:49Z`, hash
`91b3637e817592391f731caa1d1569cf093b9e06b9b005001284975eb30d6fcd`.
A sensitivity edit made at `09:04:49Z` appeared in the peer by `09:06:14Z`, with
pack activation/order preserved. Observation times bound completion; they are
not precise per-cycle latency measurements. No manual Review/Apply was used.
The window closed cleanly and its saved data was retained.

At 720×520 in a fresh fixture, Tab reached the instance checkbox, Space selected
it, focus remained after the atomic save, and Tab/Space reached and selected
the next instance. The compact page scrolls; long helper text wraps. Focus is
visible, although the list viewport clips the outer left edge of its ring.
That minor visual deviation is recorded for owner review. No full accessibility
certification or owner acceptance is inferred from this smoke test.

Current evidence: [disabled](docs/pandora/screenshots/automatic-disabled.png),
[selected participants](docs/pandora/screenshots/automatic-selected.png),
[compact keyboard focus](docs/pandora/screenshots/automatic-small-focus.png).
Screenshots that captured unrelated foreground content during user interaction
were excluded; no such image is presented as sync evidence.

### Optimized artifact and closeout

- Implementation SHA: `65ab258a69c74a865f38c140546b3fbca2b923eb`.
- Optimized native build: passed with `--release --locked --features
  disposable-data-root`; the recorded 13m48s includes waiting for an earlier
  Cargo build. A subsequent current-source identity build completed in 0.47s.
- Retained executable:
  `C:\Dev\aurora-pandora-evidence-20261101\automatic-native.exe`.
- Executable SHA-256:
  `ee2b9bac485ee77103cf1037f88adc5764458d9d06e42dece70b53772008d3a7`.
- Debug native build and actual native boot passed. The legacy Tauri host was
  type/build checked through the default core target; no Tauri UI boot is claimed.
- The keyboard fixture's disable action persisted `enabled: false` while keeping
  both selected IDs. The owner subsequently continued interacting with that
  disposable window; it remains available, and its saved state is retained.
- Final automatic resource runs were deferred at the owner's request to move on.
  Real minute propagation and the deterministic timer tests passed; CPU/memory
  overhead for enabled automatic pools and large libraries remains unmeasured.
- Preservation evidence: `automatic-preservation.json` records all original
  tracked files present, main status/diff unchanged, original native checkout
  clean, all 13 critical hashes matching, and no lockfile/authority changes.

The larger initiative remains **PARTIAL — IMPLEMENTATION IN PROGRESS** for the
explicit functional and measurement gaps above. The requested correction from
manual syncing to selected-instance automatic minute syncing is implemented.
