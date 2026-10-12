# F3R — Durable installation operations

Work date: 2026-10-12, Windows x86_64. Starting branch
`codex/client-3-production-authority`, HEAD `db2d18e`.
**Verdict: PARTIAL — implementation and acceptance in progress.**

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

## Acceptance and limitations

Pending implementation, deterministic fault matrix, independent-process tests,
native forced-termination acceptance, regression/builds and performance evidence.
No broad schema migration, additional loader/provider, UI redesign, production
installation, owner-data fault injection, push or publication is authorized.
