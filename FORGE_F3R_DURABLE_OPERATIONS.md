# F3R — Durable installation operations

Work date: 2026-10-12, Windows x86_64. Starting branch
`codex/client-3-production-authority`, HEAD `db2d18e`.
**Verdict: PARTIAL — Windows native-engine durability acceptance passed; full
desktop fault acceptance and Linux execution remain unproven.** The game
replacement/recovery implementation is complete for review. Do not promote this
to a cross-platform COMPLETE claim from the Windows evidence alone. F1C gameplay
acceptance remains independently pending.

Final implementation HEAD: `82225f7` (starting HEAD `db2d18e`). The closing
documentation commit follows it; its exact final HEAD is recorded after commit
in `.zcode-diag/phase-f3r-durable/final-head.txt` and in the final handoff.

## Preflight and F1C

Tracked tree initially clean. Tracked/untracked inventories, history and SHA-256
hashes of pre-existing nonignored untracked files were recorded in
`.zcode-diag/phase-f3r-durable/`. Baseline: 923 Rust library tests passed,
30 ignored; 266 frontend tests passed; frontend type check passed.
F1C authenticated gameplay for Forge 1.20.1/47.4.10 and 1.20.4/49.2.0
remains pending: neither this request nor the checked-in acceptance record
contains owner confirmation. F1C remains PARTIAL independently of F3R.

## Baseline architecture audit (before implementation)

Lifecycle resolves exact versions, saves Installing, ensures managed Java for
processors, calls the common game installer, installs optional Aurora, deeply
validates, then saves Ready. All four loader families share game execution;
Aurora is a separate Fabric-only tail. Game installation uses a process-local
mutex keyed by instance ID, fixed `.install-staging/game`, verified cache
acquisition, copied inputs, bounded processor execution, staged rehashing,
manifest-last and promotion. Schema-1 F1A receipts live only inside staging.
Replacement moves `game` to `.install-staging/replaced-game`, promotes the new
tree, then removes staging. Retry recursively discards that staging, including
the only old valid tree after a between-renames crash. Receipt writes use
temporary-file rename without sync. Existing-game ownership checks parse the
manifest but do not deeply validate before reservation. No startup restoration
or independent-process exclusion exists. Game schema 2 (legacy 1 readable),
registry 5, runtime state 1, F1A receipt 1 remain the compatibility baseline.

Runtime installation similarly uses a process-local exact-identity mutex and
two renames with the retired tree inside disposable staging. Expected-digest
cache publication uses unique staging plus verified rename/race validation;
transport-observed first-use pin publication can race across processes.
Metadata is fetched into memory, with no shared mutable metadata file. Natives
and processor scratch are per-instance. There is no cache GC. F1A cancellation
and Job Object/process-group termination must remain intact.

## Failure-state matrix and intended authority

| Interruption / conflicting evidence | Baseline risk | Required F3R decision |
| --- | --- | --- |
| Two processes, same target | Both reset/write fixed staging | OS lock on one stable target-owned file; immediate busy |
| Independent targets | ID-only mutex can needlessly serialize roots | Distinct physical lock objects; no whole-launcher install lock |
| Aliased/junction target | Lexical derivation insufficient | Canonical root, no redirected managed descendants; same file lock |
| Acquisition/processor/staged validation failure | Old tree usually intact | Preserve old; staged receipt remains diagnostic only |
| Before backup rename | Old game intact | Journal binds exact old/new evidence before any rename |
| After backup, before promotion | Retry destroys last old tree | Restore proven old backup; preserve candidate |
| After promotion, before commit | Manifest appears complete prematurely | Journal blocks Ready; restore old until commit authority persists |
| After durable commit | Cleanup can fail | Keep valid new authority; do not roll back committed replacement |
| Cleanup interrupted | Partial retired tree | Never delete the only valid tree; retain evidence on cleanup failure |
| Recovery interrupted during restoration | Another between-renames window | Persist Restoring first; replay by identities and exact locations |
| Malformed/future/missing journal with backup | Ambiguous authority | Manual repair; preserve both; no guessed timestamp ordering |
| Missing staging / backup | Receipt alone misleading | Compare actual old/new candidates; safe restoration or explicit conflict |
| Both trees valid, no authority | Ambiguous | Preserve both; manual repair |
| Both candidates invalid | No ready installation | Repair-required; preserve evidence |
| Disk full / denied write / locked rename | Rollback may fail | Fail before destructive work or leave replayable journal; no success claim |
| Cross-volume move | Copy fallback could partially destroy | Same-parent paths, rename only; fail, never copy/delete fallback |
| Link/reparse/path injection | Arbitrary cleanup possible | Derived fixed paths only; strict bounded journal; refuse links recursively |
| Cancellation during commit/recovery | Async drop can split transitions | Synchronous non-awaiting critical section; durable replay after forced kill |
| Registry/game disagree after restore | False Ready or wrong pin | Deep lifecycle consistency blocks Ready; explicit retry remains available |

## Design decisions before implementation

Preserve installed-game schema 2: commit authority belongs to a separate strict,
versioned journal outside disposable staging, not a breaking manifest migration.
The journal contains identities and hashes, never filesystem paths or secrets.
Old backups stay outside disposable staging. Only OS-locked execution/recovery
may mutate game authority. Read-only validation refuses unresolved evidence;
startup and retry perform recovery under that same lock before Ready/mutation.
Every rename is a separate transition. Recovery restores uncommitted old state,
retains committed deeply valid new state, and preserves ambiguous evidence.

Use standard-library `File::try_lock` (available in the installed Rust toolchain),
which maps to Windows LockFileEx and Unix flock. Lock files persist; ownership
is the kernel-held handle, never a PID or file-existence heuristic. No waiting
queue or stale-file deletion. Windows handles deny delete sharing. Target
paths derive from validated IDs/exact runtime identities; multiple resource
ordering is lifecycle target -> runtime -> cache source, with cache locks never
requesting instance/runtime locks. Registry document critical sections remain
short and must not span network activity.

Journal updates use exclusive unique temporary siblings, write_all, sync_all,
atomic rename and Unix parent-directory sync. Candidate files are synchronized
before replacement. Windows directory/power-loss guarantees and filesystem
behavior remain explicitly limited; process-kill recovery is the acceptance
target, not an unsupported universal power-loss promise.

Primary references: [Rust file locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock),
[Windows LockFileEx](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex),
[Windows MoveFileEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).

## Implemented transaction and recovery model

The journal writer, replacement, recovery and readiness gate ship together: a
writer-only intermediate version would make a promoted manifest falsely Ready,
so those dependent invariants form one reviewable commit. Durable schema 1 is
separate from installed-game schema 2. Legacy game schema 1 remains readable.
F1A receipts remain schema 1; their new Replacing stage does not falsely claim
that the protected backup is the legacy staging-relative `replaced-game`.
Legacy receipts are bounded and classified safely; they never supply paths.

Game operation states: Prepared -> Replacing -> Promoting -> Promoted -> Committed.
Recovery states: Restoring -> Restored, or Abandoned when preparation stopped
before replacement. Each operation has a fresh opaque UUID. The active journal
is outside staging and terminal copies live in `.install-history/<UUID>/journal.json`.
The previous game is retained in `previous-game`; a superseded uncommitted
candidate is retained in `rejected-game`. No backup deletion/GC is implemented.
This deliberately spends disk space to avoid deleting the only recoverable tree.

Parsing is strict and bounded (journal16KiB, installed manifest16MiB, legacy
receipt4MiB); unknown fields/schema and mismatched IDs fail. Manifest SHA-256
and sorted complete-tree SHA-256 observations bind identity and content. These
are recovery consistency evidence, not invented upstream artifact digests.
Production artifacts still use the existing honest SHA-1/SHA-256/observed trust.

Recovery precedes game mutation and startup Ready presentation. Same-target
kernel ownership covers replay. Valid old evidence wins until Committed; after
Committed the new manifest identity remains authority and ordinary deep file
validation determines health. Unknown/missing/conflicting evidence never causes
a tree wipe. Replaying Restoring tolerates a crash before quarantine, after
quarantine and after the old directory is restored. The final validation/Ready
write and final validation/spawn hold the game lock. Validation itself remains
read-only and download-free, acquiring only an existing rendezvous file.

Ordinary lifecycle installs hold `.instance-operation.lock` across their full
operation. Restoring a game does not invent a rollback of desired configuration,
optional Aurora content or a registry pin already persisted as Installing: those
facts must still agree before Ready. Fresh pack component rollback retains its
Installing record and game/journal instead of erasing recovery authority.

Cancellation is checked before acquisition and between settled bounded batches,
copies, native archives and validation batches. An active download settles under
its existing transport bounds; cancellation does not promise immediate I/O abort.
F1A supervises processor trees as before. Synchronous replacement/replay has no
cancellation await between transitions. Forced termination relies on persisted
state and kernel lock release, not Rust destructors.

An explicit retry recovers first, then allocates a new transaction; it never
reuses an unresolved transaction's backup slot. Recovery is network-free. Its
public Rust outcome distinguishes None, Interrupted, PreviousRestored and
Completed. Existing progress surfaces describe restoration/completion; local
diagnostics retain transaction IDs and stages. `installation_already_in_progress`,
`installation_recovery_required` and `installation_cancelled` identify busy,
ambiguous and cancelled work without returning paths, arguments or tokens.
Command DTO shapes stay unchanged. Startup marks affected summaries Installing
when recovery fails or another writer owns the target, while unrelated instances
continue loading. Final launch still requires deep lifecycle/runtime/session
validation; a stored Ready flag never overrides a recovery problem.

## Shared resources and document writes

Game and lifecycle locks are distinct: `.instance-operation.lock` covers ordinary
create/retry/configuration workflows; `.game-install.lock` covers the actual
game mutation/replay and final Ready/spawn validation. Acquisition proceeds from
lifecycle ownership to runtime/cache work, or to game/content/document locks at
finalization. Cache/runtime code never requests lifecycle ownership. Short
registry/config critical sections do not span network operations. Contention
fails explicitly; no installation wait queue exists. Transport-observed source
locks alone wait up to 30 seconds, checking every 25ms.

Runtime identities have separate parent-owned kernel locks. A valid runtime is
reused and never replaced; this phase does not claim that damaged-runtime repair
has a new durable game-style journal. Expected-digest artifacts retain verified
temporary publication and hash-on-reuse. Digest-less source pin publication is
serialized by SHA-256 of the source identity, without upgrading its trust status.
Metadata remains in memory, natives/scratch remain target-scoped, and no cache
cleanup/GC was introduced.

Registry save now merges independent changes against the loaded snapshot under
a short document lock; conflicting edits to the same record fail instead of
clobbering another process. Preferences use stale-snapshot refusal. Both writers
validate the current document and use unique synchronized temporary siblings.
Their snapshots are serde-skipped internal state, with no persisted schema or
equality change. Unknown/malformed documents are never overwritten.

## Security and filesystem boundaries

Journal parsing rejects unknown fields, schemas, malformed transaction IDs,
wrong instance IDs, missing candidate identity and bad digest shapes. All paths
are fixed derivations under a validated instance root. Directory containment is
rechecked before rename and cleanup. Root aliases converge to physical storage;
redirected managed descendants, junctions/reparse points, traversal and link
trees fail closed. Unix managed directories also reject a different device.
Tree inspection is bounded to depth 64 and 200,000 entries. Windows lock handles
deny delete sharing; Linux opens rendezvous files with O_NOFOLLOW; Unix rejects
multiply linked lock files. A lock file's existence or PID never proves ownership.

Only fixed staging is recursively disposed after containment/tree inspection;
legacy backups block that disposal. History and rejected/previous games are not
deleted. Fresh pack component rollback now keeps the unavailable record and
game/journal after exact provider-file rollback, rather than recursively erasing
the instance root. Its regression test asserts those stronger preservation rules
and unchanged sentinels/cache; it was intentionally updated for this behavior.
No existing loader assertions were removed.

F1A processors remain supervised execution with the user's permissions, not an
OS sandbox. Sources, verified-input rules, bounded output, timeout, cancellation
and process-tree termination remain in force. Full regression exposed a retained
output-tail bug when one pipe read exceeded the cap; the fix retains only the
last cap bytes, with oversized-chunk and zero-cap coverage. No authentication,
credential, release-source or artifact-trust policy changed.

These checks do not constitute a handle-relative filesystem sandbox against a
malicious same-user process continuously replacing namespace components. No
cross-process exclusion of already-running Minecraft or all content mutations
is claimed. A same-device bind mount is not proven absent by a device-number
check. Network/removable filesystem semantics need separate qualification.

## Automated verification

Baseline: Rust library 923 passed, 0 failed, 30 ignored (54.17s); frontend
266 passed, 0 failed (83.812s); type check 0 errors/warnings. Existing integration
baseline was six passing tests from F1C; final F3R execution re-runs them.

The final result table and log index are in
[the curated evidence record](.zcode-diag/phase-f3r-durable/EVIDENCE.md).
Final source: format/check passed; release suite 941 unit plus 6 integration
passed, 0 failed, 30 ignored (unit execution 54.25s). Frontend 266/0,
type check 0 errors/warnings, static production build and final native
production build passed. The native build took 7m54s including time queued
behind release-test compilation; it produced a build-tree executable only.
Commands used:

```text
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --release --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --no-bundle
cargo build --manifest-path src-tauri/Cargo.toml --example f3r_acceptance
```

The complete release-profile test run avoids overwriting the owner's running
debug executable. The debug library suite also passed 941/0/30 ignored (54.07s)
before the last regular-leaf read guards; final all-target check and release
suite cover those guards. Frontend checks/builds are included even though no
frontend source changed. Existing unrelated warning output is retained.

Deterministic coverage includes every writer/recovery checkpoint; fresh and
successful replacement; old/new/both invalid; missing staging/backup/journal;
future/malformed/path-injected evidence; conflicting authority; idempotent
recovery and interrupted retry; schema-1 game/receipt fixtures; snapshot races;
real subprocess death and aliases; Windows junction and locked rename/cleanup.
The final journal subset passed 11 tests. Partial journal writes and failures
at temporary creation, file sync and atomic replacement inject StorageFull,
PermissionDenied and CrossesDevices errors. These are deterministic I/O faults,
not a claim that the host disk was filled or a second volume was mounted.
Actual Windows locked-handle failures separately verify safe replay.

## Native acceptance and loader regression

All destructive cases used a newly marked direct Temp child, never the owner's
managed root. Its 42,654 input files (6,871,641,577 bytes) were copied read-only
from the existing F1C isolated root in 98.015s. The runner refuses an unmarked,
non-Temp or incorrectly named root. The original diagnostic GUIs stayed running.
Root discovery: `.zcode-diag/phase-f3r-durable/native-root.txt`.

The checked-in opt-in Rust example links the production engine and official
normalized plans. A Rust-only observer parks at a transition; the Python driver
forcibly terminates that exact independent process. Persisted journals, real
renames, kernel locks, cache objects, managed Java and processors are exercised;
destructors and injected errors do not simulate these deaths. No environment or
frontend control enables transaction faults in the shipped application.

| Scenario | Observed Windows native-engine result |
| --- | --- |
| A: same target | Holder PID 21712 held Prepared; contender 27256 failed busy in 16ms. After killing the holder, recovery PID 22332 succeeded. |
| A: independent target | Vanilla PID 20940 reached Prepared while Forge 21712 still held its target; both recovered after exact-handle termination. |
| B: old moved | PID 9616 killed after backup: game absent, protected backup present, journal Replacing. Recovery 18308 restored old; repeated recovery passed. |
| B: new promoted | PID 20432 killed after promotion: both trees present, journal Promoting. Recovery 25100 quarantined candidate and restored old; repeated recovery passed. |
| B: committed | PID 25704 killed after Committed. Recovery 18100 retained the committed candidate and protected old tree; repeat passed. |
| C: recovery killed | PIDs 26224/3184/19032 killed at Restoring/AfterQuarantine/AfterRestore. Subsequent and repeated replay passed each time. |
| D: real processor | Forge 1.20.4 runner 27152 killed with Java 18416 observed alive through a retained handle. Java exited after parent death; receipt remained processing, journal Prepared, old manifest unchanged, no staged completion marker. Recovery passed. |
| D: explicit retry | Final-source Forge 1.20.4 replacement completed Ready after that interruption; all 3,915 artifact acquisitions were cache hits, zero downloads. |
| E: loader preservation | Vanilla, Fabric, NeoForge 26.2/26.2.0.88, Forge 1.21.11/61.2.0, Forge 1.20.1/47.4.10, Forge 1.20.4/49.2.0 and Aurora/Fabric all deeply Ready, no problems. |

The final matrix result is PASS **for independent native engine processes**.
The matrix binary predates the closing derived-directory/read-leaf hardening;
the final-source retry and final automated suite cover that delta. Full matrix
logs retain PIDs, UUIDs, stages, tree evidence, timings and replay outcomes under
the isolated root's `native-evidence/`. Failed harness attempts were retained:
Java rejected a canonical Windows extended path in its classpath, so the runner
now validates the physical root but supplies ordinary Windows path spelling;
the initial short-lived child PID race was fixed by retaining an OS process
handle and observing the actual running processor. Neither failed attempt
damaged the original game.

A separate real Tauri diagnostic executable booted against an empty isolated
root, logged backend Ready, answered frontend requests and rendered Home with
“No instances yet” and “Not signed in.” Its native UI accessibility observation
is recorded in `desktop-boot.txt`. This proves boot/presentation/IPC, **not two
full desktop GUI instances executing the A–D crash scenarios**. No authenticated
Minecraft launch or gameplay was performed and no auth state was opened.
That full-app acceptance gap and unexecuted Linux behavior keep F3R PARTIAL.
The final-source desktop build was repeated separately (PID 4348, 26.83s build),
with backend Ready and the same Home rendering recorded in `desktop-closing.txt`;
its config/WebView directories are also isolated. A first build invocation used
an unqualified feature name and failed before compilation; the corrected command
uses `--features tauri/custom-protocol`. Production build configuration did not
change.

## Performance and durability limits

The matrix has no cold-install baseline, so it cannot quantify a percentage
regression. Native target-lock acquisition samples are about 0.4–0.5ms; journal
authority writes are approximately 1–2.2ms. Full tree hashing, validation and
synchronization dominate replacement, not lock acquisition. The final-source
post-processor-kill warm retry took 158.597s including plan/runtime/readiness;
installer total 137.298s, staged validation 31.930s and commit 56.466s. All 3,915
artifacts were cache hits with zero downloaded bytes. Compilation ran concurrently
on the host for this sample; it is explicitly not a quiet-host baseline.

With all builds/hash-inventory work finished, an uncontended quiet-host repeat
on the final source completed Ready in 128.348s: installer 111.066s, staged
validation 27.320s, commit 51.379s, lock acquisition 388µs. Journal writes were
0.912–1.545ms. It again reused 3,915 cache artifacts with zero downloads and
reused Java after validating 403 files / 91,160,397 bytes in 1.510s.
This is a measured warm total, not a measured before/after F3R overhead delta.
The final-source seven-loader revalidation also passed (96.530s on a host with
build activity). Logs are `native-warm-quiet.log` and
`native-all-loaders-closing.log`.

After-backup recovery including final deep validation took 39.750s; after-
promotion restoration 55.172s; after-commit replay 5.531s. Repeat invocations
were 22.719s, 21.703s and 5.438s respectively. Recovery interrupted at its three
boundaries subsequently completed in 51.813s, 35.140s and 24.672s. The final
seven-instance validation took 81.516s. Independent ownership was demonstrated
while another target remained parked; this does not measure dual-download
throughput. No global installation mutex was added.

Windows x86_64 local-disk process termination is the exercised platform/failure
model. Unix locking and directory fsync are implemented but were not executed
on Linux/macOS. Files are synced before transitions, Unix directory sync follows
renames, and cross-device renames fail without copy/delete fallback. Windows
directory persistence, controller write caches, sudden power loss, FAT/SMB/NFS,
actual disk exhaustion and mounted cross-volume fault acceptance are not proven.
No universal power-loss durability claim is made. Previous/rejected trees are
retained indefinitely; disk-space cost and retirement policy remain open.

## Preservation and review decisions

Final hash verification, critical-file inventory and deletion checks are recorded
in `preservation-closing.json` and summarized in the evidence index. No cache,
owner instance, production executable, credential state or pre-existing diagnostic
was used as a destructive target. `.minecraft` remained off-limits. No dependency,
lockfile, Tauri identifier, existing schema, loader pin or production endpoint
changed. No push, publication, installer deployment or history rewrite occurred.
Build output, games, runtimes and executable diagnostics are not staged.
Hashes matched all 15,230 pre-existing untracked files and all 42,654 original
F1C managed files. The copied instance user/Aurora files outside game/staging/
history also matched. All baseline tracked and critical files remain present;
the only inventory correction was a verifier assumption of `vite.config.ts`
where the original repository actually tracks `vite.config.js`.

Owner review should decide the retention/GC policy and the desired future UI
for explicit manual repair. A failed version/configuration upgrade may restore
the previous game while remaining Installing because the attempted registry
pin/desired state differs; automatic multi-document/Aurora rollback is not
implemented or claimed. Preserve those records for explicit retry/review.
No approval for a schema-3 migration or new loader/provider is requested.

Recommended next phase: finish full Tauri A–D acceptance and Linux subprocess/
filesystem acceptance; then review this transaction design and the measured
hash/sync cost before broad rollout. Handle authenticated F1C gameplay confirmation
separately, and design any backup retirement or coordinated desired-state rollback
as a bounded follow-up with its own failure matrix. Do not proceed to new loaders
or CurseForge as a substitute for closing reliability acceptance.

## Local commits and changed files

| Commit | Review scope |
| --- | --- |
| `78e4c26` | Pre-implementation audit, preservation authority and failure matrix. |
| `ab994a2` | OS game/runtime/source locks and real subprocess/alias tests. |
| `cf27ae1` | Processor retained-tail bound found by regression. |
| `300ffc9` | Journal, replacement, replay, readiness, cancellation and dependent lifecycle integration. Writer/replay ship together to avoid a false-Ready intermediate implementation. |
| `146e4eb` | Independent registry merges, preference stale-write refusal and synchronized documents. |
| `b5f16ee` | Filesystem I/O faults, substituted-directory refusal and locked recovery/cleanup. |
| `82225f7` | Opt-in native replacement/recovery/processor death acceptance runners. |
| Closing documentation commit | This report, architecture and curated evidence; exact HEAD in final handoff. |

Production changes: `application.rs`, `cache.rs`, `config.rs`,
`install/{coordination,journal,mod,processors,receipt,state}.rs`,
`instances/{deletion,lifecycle,mod}.rs`, `launch/boundary.rs`, `modpacks.rs`,
`runtime/install.rs`. The deletion-module test now blocks the document lock,
instead of an obsolete shared temporary filename, to keep exercising registry-
write failure with the new unique temporary writer; no deletion feature was added.
Acceptance helpers: `examples/f3r_acceptance.rs`, `examples/f3r_acceptance_driver.py`,
`examples/f3r_processor_kill.py`. Documentation: this file, `ARCHITECTURE.md` and
`.zcode-diag/phase-f3r-durable/EVIDENCE.md`. The exact name-status diff is retained
locally; no pre-existing tracked file was deleted or renamed.

## F3R-A acceptance addendum — 2026-10-12

This addendum preserves the historical F3R findings above. The follow-up is
documented in [FORGE_F3R_ACCEPTANCE.md](FORGE_F3R_ACCEPTANCE.md), with a new
curated index at `.zcode-diag/phase-f3ra-acceptance/EVIDENCE.md` and retained raw
evidence in newly marked `aurora-f3ra-*` temporary roots.

Actual Windows Tauri processes exercised same-target contention, independent
targets, externally killed replacement checkpoints, three consecutive recovery
kills, and a retained-handle observation of real Forge Java termination after
parent death. The UI now distinguishes busy, interrupted, previous restored,
completed and manual-repair recovery, suppresses stale Ready during preparation
and focus checks, and rejects obsolete state responses. Native recovery runs off
the UI thread and observes lifecycle ownership in the installer's existing lock
order. Debug checkpoint observation is restricted to marked temporary roots and
excluded from release builds. Integrity checks and commit ordering are unchanged.

The quiet warm A–B–A Forge 1.20.4 series measured 114.128 / 98.894 / 97.288 seconds
for the installer. First-run asset-cache variance prevents a general speedup
claim. Detailed measurements identify repeated deep validation and whole-tree
hashing as the main cost; durable journal writes and actual renames are
milliseconds. No verification/sync reduction was implemented. The new report
proposes a separately tested single-pass validation/hash experiment and an
explicit, locked, ownership-proven retention workflow; it does not implement GC.

The overall verdict remains **PARTIAL** because no Linux installation, VM or
authorized remote was available. Linux locking, filesystem/directory-sync,
SIGKILL processor termination and desktop results remain unexecuted. Windows
NTFS process-death evidence is not universal power-loss or cross-filesystem
durability. F1C authenticated gameplay remains separately pending. Production
Launcher 1.4.2, owner/auth state, original fixtures and existing diagnostics were
preserved; there was no push, publication or production replacement.
