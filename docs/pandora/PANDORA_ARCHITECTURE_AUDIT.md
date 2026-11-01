# PANDORA source investigation

2026-11-01. This is a source-level engineering comparison, not an exhaustive
security certification or a benchmark of Pandora's runtime.

## Pinned inputs and preservation

- Aurora: `0e2c8dd9e13ea1b1d548505d575674d9114ee386`, clean
  `codex/native-ui-migration`, `C:\Dev\aurora-launcher-native`.
- Investigation/implementation: `codex/pandora-backend`, isolated managed worktree
  `C:\Users\kalib\.codex\worktrees\pandora-backend\aurora-launcher`.
- [Pandora upstream](https://github.com/Moulberry/PandoraLauncher/tree/2d83d6f22c43e804d9b544e8dc98d49d4874e3af):
  `2d83d6f22c43e804d9b544e8dc98d49d4874e3af`, fetched `origin/master`,
  commit subject `Add instance groups`. Study checkout is detached and unmodified
  at `C:\Dev\aurora-pandora-evidence-20261101\PandoraLauncher`.
- Root `AGENTS.md`, current architecture sections, native architecture, NATIVE-10
  acceptance and updater authority were inspected. Old architecture paragraphs
  are historical: NATIVE-10 custom Java and local pack recognition supersede
  older managed-only / unsupported-local-pack statements.
- Before editing: all worktrees, full HEAD/history, index/working status,
  tracked/untracked lists, main checkout binary diff, and SHA-256 of 13 critical
  source/configuration/lock/authority files were recorded in the external evidence
  directory. The primary native worktree and dirty WebView checkout are preserved.
  No release authority, signing key, credential or Client source was changed.

## Source inventory and architecture

The following implementation paths were inspected, with focused reads of their
relevant functions and call sites. This inventory describes actual review scope;
it does not assert every line of each crate was independently audited.

| Domain | Pandora source | Aurora source |
|---|---|---|
| Composition/runtime | root and backend/auth Cargo manifests; backend `lib.rs`, `backend.rs`, `backend_handler.rs` | `native/Cargo.toml`, core manifest, `host.rs`, `application.rs`, native `main.rs`, `api.rs`, `ui.rs` |
| Instances/launch | `backend.rs::prelaunch`, `instance.rs`, `launch/mod.rs`, `launch_wrapper.rs` | `instances/{mod,lifecycle,settings}.rs`, `launch/{state,boundary,process,resolve}.rs` |
| Network/metadata | `metadata/{manager,items}.rs`, `reqwest_client/src/reqwest_client.rs`, `install_content.rs` | `transfer.rs`, `downloads.rs`, `cache.rs`, `install/{mod,assets}.rs` |
| Packs/storage | `mod_metadata.rs`, `install_content.rs`, `fs.rs`, `duplicate.rs` | `mrpack.rs`, `modpacks.rs`, `pack_merge.rs`, `pack_update.rs`, `local_pack.rs` |
| Sync | complete `syncing.rs`, backend prelaunch call sites, `schema/src/backend_config.rs`, `bridge/src/safe_path.rs` | new `instance_sync.rs`, existing managed paths, kernel coordination, content and launch boundaries |
| Persistence/recovery | `persistent.rs`, `fs.rs`, duplication/content failure paths | registry/configuration loaders; `install/{coordination,journal}.rs`, pack transactions |
| Auth/logs/platform | `auth/src/{credentials,secret}.rs`, `account.rs`, `log_reader.rs`, launch sandbox integration | auth domain contracts; process redaction/capture, managed/external runtime validation, platform path and lock helpers |
| Large libraries | `ftree/src/lib.rs`, crate inventory | native bounded Discover models, artwork limits, event-driven readiness |

Pandora has eleven crate directories: auth, backend, bridge, command, frontend,
ftree, nbt, pandora_launcher, reqwest_client, schema and t. The root manifest's
explicit member list is eight entries; additional path crates participate through
dependencies. It separates schemas, UI bridge, platform commands and GPUI client
adaptation. Aurora has a host-neutral domain core and two hosts; Slint uses mirror
DTOs and two Tokio workers with up to eight blocking workers. Tauri remains
optional. More crates alone are neither a speed nor a maintainability proof.

Pandora's backend creates a debounced filesystem watcher and a 64-message event
channel at startup, loads all instances, and handles messages, filesystem events
and a one-second tick. It also carries metadata state maps, content metadata,
server pinging and persistent instance state. These support broader live-library
features. Aurora's readiness is explicitly triggered, coalesced to one in-flight
deep check and invalidated by input changes. Following the owner's correction,
sync has one Rust-owned 60-second interval only while enabled with at least two
selected instances and a category. It performs no network polling, permanent
thread, watcher or background content index. Disabled sync drops its timer.

## Download, cache and scheduling findings

Pandora's launch preparation joins Java, assets, libraries and logging tasks.
Runtime/assets/library code uses independent semaphores (8 network / 32 disk in
the inspected functions), schedules task vectors, and several paths retain an
entire `response.bytes()` payload before hashing/writing. This can overlap useful
work but a task-count limit alone does not bound total buffered payload bytes.
No relative performance claim follows without matched measurements.

Its content installer is different: an eight-permit content semaphore, per-file
coordination, cache rehashing and streamed HTTP chunks into a SHA-1 content-library
path. Size and SHA-1 are checked, and a bad object is removed. That inspected path
creates the final library file before complete verification. Aurora's separate
untrusted staging, verified promotion, rehash-on-hit and explicit digest versus
transport-observed provenance are retained. Neither launcher simply trusts every
cache filename; Pandora's metadata cache also validates an expected SHA-1 when
one is supplied.

Aurora already has global 24 / per-host 16 permits, digest single flight, four
bounded retry attempts, backoff, streaming verification, cancellation checkpoints
and settle-before-return acquisition. The ordered sliding window can hold
completed items behind a slow first item, delaying refill. This investigation
adds a completion-order window only to a provider-pack verification pass whose
callback returns no ordered output. Classpaths, returned artifact vectors and
manifest construction retain deterministic input order.

Independent discovery: Aurora's host map retained every historical host, and a
cancelled digest waiter could leave an otherwise idle slot behind if its owner
had already dropped. A lease now covers owners and waiting futures. Resources
drop before their final lease, and idle entries are removed under the same map
lock. No expiry worker, LRU, dependency or readiness-cache weakening is needed.

Pandora's metadata manager has typed in-flight states, five-minute success
keepalives, error retry lifetimes of 1/3/9/27 seconds, per-host semaphores and a
file fallback when metadata has no expected hash. Its source itself notes that
expired keepalives do not necessarily remove state-map entries. Shared metadata
resolution is promising, but copying the whole manager would introduce a much
larger persistence/expiry contract. Aurora metadata single-flight is deferred
until request counts identify a specific redundant path.

## Packs and extraction

Pandora recognizes Modrinth and CurseForge packs and keeps override payloads in
an `IndexMap<SafePath, Arc<[u8]>>` in the inspected pack-recognition path. Its
broader provider/import support is valuable. Aurora's normalized pack plans,
optional-file review, origin/ownership tracking, existing-instance merge rules,
transactional update/rollback and bounded unsafe-archive rejection remain useful
boundaries. Local `.mrpack` recognition deliberately needs exact published
Modrinth identity; no private provenance is invented here.

Aurora had its own avoidable allocation: planning expanded each accepted
override into a `Vec` (up to 64 MiB) solely to compute SHA-256. The implementation
now hashes with a 64 KiB buffer, retaining exact length, expansion limit, CRC/read
error and digest semantics. Actual override materialization still uses the
existing bounded reader and transactional writers. That remaining allocation is
reported explicitly; this change does not claim fully streaming extraction or
faster network downloads.

## Duplication and filesystem strategies

Pandora's `duplicate.rs` inventories directories/files/internal and external
links, checks cancellation, tries reflinks, recognizes content-library hardlinks
by file identity and SHA-1, then uses a reusable 128 KiB copy buffer. It retains
permissions/timestamps and has Windows junction handling. History includes
`Use reflinking where possible` and a later junction duplication fix. This is a
real capability Aurora currently lacks. The inspected failure path recursively
removes its newly allocated destination.

Aurora should eventually add explicit duplication with an opaque new instance
ID, completion/recovery marker, a user-content ownership policy and private
mutable copies. Reflink availability/fallback must be measured on the actual
filesystem. Hardlinking mutable instance content or recreating external links is
not adopted. Neither a synthetic tiny-file test nor a README establishes that
duplication is fast on this host. No duplication implementation is included.

## Sync: useful design and failure modes

Pandora applies sync at prelaunch. File targets choose the latest observed
created/modified time. `options.txt` combines key/value maps in timestamp order,
preserving the current instance's resource-pack activation keys. Directory
targets become Unix symlinks or Windows junctions to a central directory; only
absent/empty instance directories are linked. Instances can opt out. The target
catalog includes saves, config, screenshots, packs and several mod data roots.
**The current `SyncTargets::default()` is empty**: the catalog is not evidence
that world syncing is enabled by default.

Strengths: simple per-instance exclusion, category awareness, reusable content,
platform links and pack-order preservation. Limits of the inspected design:
timestamps cannot establish causal ordering, map merging can spread settings
across versions, a shared mutable directory exposes simultaneous writers, and
many best-effort filesystem errors are discarded. An empty-directory link policy
also cannot merge existing nonempty directories. These are design concerns,
not an exploit validation or a claim that every deployment loses data.

Aurora implements an independently authored, deliberately smaller first slice:

- Off by default, persisted in strict schema-2 `launcher/instance-sync.json`.
  The owner selects any participating instances, categories and exclusions;
  they sync every minute while the launcher is open. Native Review/Apply and
  source/destination selectors were removed. A known schema-1 manual preference
  loads as unselected/off without rewriting it; it cannot authorize automatic
  participation. Unknown/malformed documents fail untouched.
- Settings and controls are separate. Installed Minecraft and loader pins must
  match. Only keys already present in both option files transfer. Settings use
  a narrow allowlist for mouse/accessibility/chat/audio; controls use common
  `key_` keys. Data version, unknown mod settings, pack activation/order and
  missing/version-specific keys remain local. Duplicate/malformed input fails.
  A hash-only, bounded `launcher/sync-observed.json` baseline detects new edits.
  Initial differences and simultaneous different edits are preserved. A unique
  subsequent edit propagates to unchanged compatible peers; acknowledged
  baselines retain pending edits while a previously observed peer is busy or
  running. The first minute establishes the initial options baseline.
- Resource packs, shaders and screenshots: private copies of missing files;
  byte-identical files are skipped; existing different files are explicit
  keep-destination conflicts. There is no delete propagation or shared folder.
- Configuration uses the same additive policy, with explicit `config/...` file
  or directory selection and matching installed Minecraft/loader versions.
  Unselected configuration is not enumerated. Existing differing files are kept.
- Exact path/prefix exclusions, bounded traversal (4,096 entries, depth 16,
  256 MiB/file, 1 GiB scanned source bytes), reparse/special-name rejection,
  and 64 KiB file hashing. No content values enter the review DTO. A cycle has
  a 1 GiB copy budget; excess work continues next minute. Path/size/mtime hints
  avoid rehashing unchanged folder pairs after convergence, but every actual
  copy still uses the full hash-bound plan. Hints never authorize launch/cache
  integrity. Unchanged options observations are not rewritten each minute.
- A SHA-256 review fingerprint binds request, registry records, source and
  destination identities, full selected policy and planned output. The automatic
  worker applies this internal plan after re-planning and rehashing.
  These local consistency hashes do not create publisher trust.
- Both Play preparations and source/destination writer operations are excluded
  during work. Registered IDs are checked before creating lock roots. The policy
  cycle lock excludes overlapping automatic work across launcher processes.
  Policy writes have a separate lock. Policy is checked before each commit;
  disabling or changing participants stops subsequent commits. An already
  entered atomic per-file commit may finish. Process supervision remains local.
- A review receipt precedes mutations. Options backup is durably written before
  atomic replacement. Missing-file commit uses a unique completed private temp
  and no-clobber link/unlink within that destination; it never links two
  instances or a cache object. Per-file commits can survive interruption; there
  is no claim of one atomic multi-file transaction or automatic rollback.

Remaining sync work is significant: multiple independent pools,
per-instance category overrides, NBT-aware
server-list merging, explicit replacement/conflict resolution, world snapshots
and recovery UI, and explicit cancellation/progress for large sync jobs. Worlds
and `servers.dat` are unsupported/off, not shared blindly. No safety assertion is
made about orphaned games or older/external launchers that do not participate in
the current process's Play reservation. Users must stop such games before sync.
The path rechecks are defense in depth, not a handle-relative filesystem proof
against an actively malicious same-user directory replacement.

## Preparation and incremental readiness

Pandora's joined preparation is a useful user-experience reference. Aurora now
distinguishes **can prepare** from **ready** in its Rust decision. Play may fill
only a missing exact managed Java requirement, after account eligibility, with
the same cache, runtime transaction, diagnostic and supervisor. A second check
under the runtime writer lock permits reuse but refuses a newly appearing
damaged/unknown tree. No custom-Java fallback, repair of foreign files, version
substitution, alternate process path or session bypass is introduced. Final
deep instance/runtime/session checks remain authoritative.

Broader missing-game repair is deferred: registry state alone cannot authorize
overwriting an unknown or user-modified tree. General readiness-result reuse
is also deferred; unchanged timestamps are not byte-integrity proof.

## Comparison and decisions

A = Aurora stronger for the stated boundary; B = Pandora has a demonstrated
capability absent here; C = complementary; D = redesign needed for Aurora;
E = insufficient comparative measurement. These are scoped judgments.

| Capability | Aurora implementation | Pandora implementation | Evidence/class | Recommended action |
|---|---|---|---|---|
| Instance preparation | Deep validation, exact plans; missing-Java Play preparation added | Joined Java/assets/libraries/logging prelaunch | `launch/mod.rs`, application readiness; C | Adapt UX; preserve exact plans/guards |
| Shared assets | Shared immutable SHA-1 store plus private installation state | Shared launcher asset objects | cache/assets and load-assets source; C/E speed | Preserve; no unsupported speed ranking |
| Download scheduling | Process-wide permits, retry and digest single flight | Content semaphore/per-file lock; separate launch task pools | transfer / metadata / install-content source; A scoped budget | Retain; add narrowly used completion window |
| Modpack extraction | Bounded normalized plan; streaming review hash added | In-memory override maps, broader formats/providers | mrpack / mod-metadata source; C | Reduce Aurora allocation; preserve parser policy |
| Instance duplication | No implemented user operation | Reflink, identity-aware hardlink, copy fallback | complete duplicate source and history; B capability | Separate ownership/recovery design, then implement |
| File syncing | Selected pool, automatic minute cycle, hash-based options edits, additive private files | Prelaunch files/options and directory links | complete syncing source, backend call sites; C/D | Independent model; wider mutable-file/world semantics deferred |
| Caching | Verified staging/promotion; explicit digest provenance | Rehashed library objects; typed metadata cache/fallback | cache / metadata-manager / install-content; A/C | Preserve trust, investigate bounded metadata reuse |
| Recovery | Durable installed-state journals, ownership-based rollback | Best-effort writes and scoped failure cleanup in reviewed paths | install journal / persistent / duplicate source; A scoped | Keep durable core; do not port persistence fallback |
| Security | Rust-only sessions/spec, narrow secret persistence, bounded archives | OS secret stores, log regex redaction, sandbox support | auth / log-reader / wrapper / core contracts; C/E overall | No blanket winner; preserve Aurora's boundaries |
| Metadata expiry | On-demand domain resolution | Typed shared states, keepalives, file fallback | metadata-manager source; C | Measure request duplication before adapting |
| Large-library indexing | Bounded visible models, on-demand inventory | Fenwick-tree crate/live filesystem updates | ftree/backend source; E speed | No new index or watcher without workload evidence |
| Idle resource use | Slint software renderer, lazy sync | GPUI plus backend watcher/tick | architecture only; E measured comparison | Measure Aurora before/after; Pandora NOT MEASURED |
| Provider breadth | Modrinth; deliberately bounded Forge/NeoForge/pack support | CurseForge, additional imports and sandboxing | install-content / launcher-import / auth manifests; B breadth | Separate provider policy work, not wholesale adoption |

## Provenance and deliberate non-adoptions

No Pandora source was copied, translated or linked. The implementation is
independent, informed by source-level concepts and Aurora-specific defects.
No new production dependency or lockfile change is required. Tokio's existing
dev dependency gains `test-util` for deterministic virtual-clock tests.
Pandora's root license is MIT,
copyright 2025 Moulberry. It is not a blanket license for every vendored crate:
`reqwest_client/LICENSE-APACHE` names Zed Industries (Apache-2.0), and ftree's MIT
notice names Bruno Rucy Carneiro Alves de Lima. Any later direct reuse must track
the exact file/commit and carry its applicable notices. No distribution notice
for unincorporated code is claimed necessary or added.

Not adopted: GPUI/framework replacement, whole-backend import, permanent sync
watchers, timestamp-winner mutable sync, world junctions, unbounded metadata
maps, wholesale archive payload caching, content-library hardlinks for mutable
files, permissive damaged-config defaults, arbitrary external-link duplication,
borrowed OAuth identity, offline authentication bypass, updater-authority changes,
and Client modifications. Broader sandboxing is useful research, not a verified
reason to change Aurora's subprocess boundary in this slice.

Benchmark results, regression outcomes and remaining acceptance work are in
`../../PANDORA_BACKEND_INTEGRATION_ACCEPTANCE.md`. No Client-side defect or change
was established: **AURORA CLIENT HANDOFF: NOT YET REQUIRED**.
