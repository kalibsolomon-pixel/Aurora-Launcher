# F3R-B — managed installation backup retention

Work date: 2026-10-12. Starting branch `codex/client-3-production-authority`,
HEAD `c378af02710de61ee32361022e0aa545425cdbbc`.
**Verdict: PARTIAL — implementation and Windows desktop process-death/retention
acceptance completed; a separate Windows file-symlink case could not execute
because the host requires administrator privilege.** Junction refusal passed.
No Windows security setting was changed to enable the missing case.

## Audit and preservation

Tracked files were initially clean. The tracked inventory, full nonignored
untracked inventory and SHA-256 hashes, and worktrees were recorded under the
new temporary `aurora-f3rb-preservation-c2e2b54b-80b4-45c1-9bb6-c0505bc39fa6`
directory. Protected developer files include `.env*`, `.zcode*`, existing
diagnostics, the three root untracked files and performance Python cache.
Existing worktrees, release artifacts, production 1.4.2 and owner data are outside
the mutation scope. No cleanup of development storage is authorized here.

Actual authority: `install/journal.rs`, `install/coordination.rs`,
`install/{mod,state,natives,receipt}.rs`, `instances/lifecycle.rs`,
`application.rs`, `paths.rs`, and the current F3R architecture and acceptance
records. All loader families share the game executor; lifecycle repair is an
explicit reinstall. Game/user data live in different trees.

Prepared precedes staging; Replacing precedes moving the entire old `game/`
to `.install-history/<UUID>/previous-game`; Promoting precedes candidate rename;
Promoted precedes deep verification; Committed establishes authority. Terminal
journals are archived as `journal.json`. Restoring quarantines an identified
uncommitted candidate into `rejected-game`, restores the old tree and records
Restored. Prepared attempts can become Abandoned. Ambiguity is an error/manual
repair, not a persisted extra state. A backup can be the only valid surviving
copy between renames, or after candidate damage. Those backups must survive.

There is no existing retirement, deletion-failure policy, or storage bound.
Whole-tree rename retains extra/owner-modified files as well as managed files.
Deep manifest validity does not reject extra files; the recovery tree digest
records their bytes but does not grant ownership. Extracted natives also lack
per-file manifest records. Existing schema-1 journals alone cannot authorize
retirement. Links/reparse points are rejected by bounded recovery inspection;
unexpected regular files can still be present. Different targets have separate
kernel locks; same physical targets/aliases contend. Kernel ownership ends on
process death; rendezvous files are never removed.

## Design before implementation

Use a separate version-1 ownership seal generated only from known installer
outputs in private staging (manifest files, exact extracted native observations,
and the completion manifest). Include exact directories, paths, sizes and
SHA-256 observations. These observations are local consistency evidence and
never upgrade artifact trust. Unknown/mixed/modified contents cannot be sealed
by later history enumeration. Existing unsealed backups remain protected.

The first release uses explicit instance Maintenance, not startup/after-install
deletion. Retain the newest eligible completed replacement backup, all backups
younger than 30 days from recorded completion, every current transaction's slot,
and all unresolved/manual-repair/rejected/legacy evidence. No disk-pressure
override, byte quota, loader-specific rule, or delete-all control. The bound is
conditional on time and safety: uncertain/user content is deliberately unbounded.

Acquire lifecycle then game kernel locks; classify recovery first and skip
pending/failed recovery rather than repairing implicitly. Require current
Committed authority and deep installed-state validation before eligibility.
Maintenance never commits a game or changes registry readiness.

Persist retirement intent in the exact history slot before same-parent atomic
rename to `retiring-game`; never copy across volumes. Verify complete ownership
before intent/rename and every remaining file before resumed deletion. Delete
exact recorded leaves, then empty recorded directories; preserve journal, seal
receipt, and small terminal retirement evidence. No recursive directory wipe.
An interruption resumes only after fresh locks, authority and validation checks.

| Invariant | Intended enforcement and test |
| --- | --- |
| 1 Active protection | Lifecycle/game locks; protect current slot; pending journal skips all. |
| 2 Recovery protection | Require valid current committed game; keep latest valid backup. |
| 3 Ownership | Forward-only staged seal plus matching terminal journal and exact bytes. |
| 4 Unknown content | Full file/directory allowlist; changed bytes/additions preserve whole tree. |
| 5 Manual repair | Failed/pending recovery and Restored/rejected history preserved. |
| 6 Concurrency | Existing lifecycle -> game order, nonblocking kernel ownership throughout. |
| 7 Failure isolation | Structured storage outcome; no install or Ready authority changes. |
| 8 Restart | Durable intent, atomic rename, missing leaves allowed only under intent. |
| 9 Containment | Validated IDs, fixed tree names, strict relative seal paths, ancestor/reparse checks. |
| 10 Ambiguity | Missing/corrupt/future evidence, unknown history and identity conflicts preserve. |

## Verification and limitations

Implementation, automated results, Windows desktop acceptance and measurements
will be appended after execution. No acceptance claim is made by this design.
Initial frontend baseline could not load dependencies after D1 cleanup; `npm ci`
restored dependencies from the unchanged lockfile before repeating the suite.
Linux, authenticated F1C gameplay, releases, production replacement, Drive and
additional development storage cleanup remain explicitly deferred.

## Implemented behavior

`install/retention.rs` owns the policy, strict schema-1 seals and receipts,
eligibility, verified retirement and structured report. `natives.rs` supplies
exact extraction output observations, including byte-identical duplicates;
arbitrary native-directory enumeration never grants ownership. The executor
writes the seal before the completion manifest, preserving manifest-last
publication. Neither seal failure nor post-commit receipt failure fails a valid
installation. Such a backup remains protected indefinitely.

The original installation's archived Committed candidate must equal the next
replacement's previous evidence, including its complete tree digest. A fresh
snapshot alone cannot establish ownership: even a syntactically valid edited
seal plus added owner file is refused. Eligibility also requires a matching
terminal replacement journal, receipt, previous identity, original source
journal, seal digest and complete backup bytes. Unknown schemas/fields,
noncanonical paths, missing evidence or invalid identities confer no authority.
Older unsealed installations, including readable game schemas 1/2 and existing
F3R journal schema 1 without the new evidence, are not automatically enrolled.

The public operation accepts only ManagedPaths and a validated instance ID.
The Tauri request contains only that instance ID. There is no caller-supplied
path, time, grace override or deletion selector. Kernel lifecycle -> game
ownership spans registry loading, recovery classification, full instance
validation, all history classification and filesystem mutation. Contention
fails with the existing `installation_already_in_progress`. A pending journal
or leftover staging skips maintenance; the action never calls recovery, changes
transaction stage, downloads, repairs or writes Ready. Failed full validation
also skips. Malformed/unarchived/nonterminal history skips the whole instance
before any deletion. Recognized Restored/rejected history stays protected.

The concrete count policy protects the current journal's entire slot **and**
the newest fully verified historical backup. Thus ordinary eligible history
converges to two protected full trees once all others exceed 30 days. Timestamp
ties all survive; UUID order never invents chronology. Future completion times
survive. The latest valid historical tree is chosen after validation, so a newer
damaged tree cannot displace the last good historical one. Uncertain/legacy
content has no hard byte/count bound; recoverability outranks storage reduction.

Retained -> Deleting -> Retired are states of the separate storage receipt,
not changes to the F3R game lifecycle. The exact history slot receives durable
Deleting intent before `previous-game` is atomically renamed to `retiring-game`
in the same parent/volume. No copy/delete fallback or arbitrary recursive
deletion exists. All remaining contents are inspected before deleting a leaf;
every leaf is checked again for contained regular-file identity and exact
size/SHA-256. Directories derive only from recorded file ancestors; unexpected
empty directories also block retirement. Each known directory is removed only
with nonrecursive remove_dir, which refuses nonempty contents. Original journals
and small retirement receipts remain indefinitely as ownership/restart evidence.

Missing leaves are accepted only after durable Deleting intent. A crash before
rename, after rename, during file deletion or after removing the empty tree is
repeatable under fresh locks and fresh installed-state validation. Both source
and quarantine present, unknown survivors, changed bytes and corrupt metadata
refuse further work. Interrupted atomic metadata writes can leave unknown
temporary siblings: preserve that slot for inspection rather than guessing.
Windows file-sharing failures preserve a safe retry; cleanup errors never enter
installation commit/Ready authority. Original F3R process-death/power-loss and
same-user adversarial namespace limitations remain unchanged.

## UI and diagnostics

Instance Settings -> Maintenance -> Recovery backup storage explains the
30-day/default protection and offers Manage recovery backups. The blocking work
runs off the UI thread. Its typed report distinguishes retainedByPolicy,
retainedActiveTransaction, retainedRecovery, retainedManualRepair,
retainedUnknownOwnership, retiredSuccessfully, cleanupInterrupted and
cleanupFailedSafely. The UI distinguishes ordinary retention, safety skip,
inspection required and retryable cleanup failure. No delete-all control,
background poll, startup hash pass or persisted preference was added.

Reports include eligible/protected/retired counts, per-transaction outcomes,
verified historical bytes before/after, current-invocation reclaimed bytes,
validation time and downstream maintenance elapsed time. Verified byte fields
exclude current/protected unknown trees that were not fully measured; they are
not a promise of total disk usage. A resumed invocation counts only its own
deletions; independent before/after inventories measure a whole interrupted run.
In the public operation, full instance validation is separately timed before
the downstream elapsed counter. Tests that invoke the internal engine include
their game-validation check inside that counter. No paths, tokens, arguments,
server information or telemetry enter retention logs.

## Automated verification

Starting native baseline: all 11 existing journal/recovery tests passed. The
initial frontend attempt lacked installed packages after the closed D1 cleanup;
unchanged-lockfile npm ci restored them. The repeated parallel frontend run
stalled and its specifically identified process tree was stopped; its log is
retained and not counted as passing. Serial execution passed all 272 frontend
tests (118.456 s). Type check: zero errors/warnings. Frontend production build,
Rust format and all-target checks passed.

Native closing default library suite: **959 passed, zero failed, 30 ignored**
(54.32 s). Release-profile regression before the final marker-order correction:
958 unit + 6 icon integration tests passed, 30 ignored. The correction then
passed the installation/recovery subset (97 passed, 5 ignored), its dedicated
manifest-last test, the closing full default suite and final all-target checks.
Ignored tests are not counted as executed acceptance. The optional fixture
generator's selected branch was also actually executed against the marked root.

Focused coverage maps the requested cases as follows:

| Cases | Executed coverage |
| --- | --- |
| Newly created, age/count, latest/only-good | Grace boundary, future clocks, tied timestamps, current slot and newer damaged backup. |
| Active/incomplete/manual repair/validation | Every nonterminal game state; Restored/Abandoned; malformed/missing active authority and damaged current game. |
| Multiple instances/concurrency | Same-target lifecycle and game contention, separate targets, inherited real-process/alias tests, and actual desktop competition. |
| Missing, extra, corrupt, legacy | Missing tree/receipt, malformed/future receipt/journal/seal, wrong instance, unknown history, rejected data and no implicit enrollment. |
| User content/path safety | Extra regular file, modified client/native, unknown empty directory/slot file, traversal/ADS/device/case-alias rejection and Windows junction refusal. |
| Interrupted/partial/repeated cleanup | Every intent/rename/file/empty/final boundary, new owner content after partial deletion, locked Windows leaf and repeat invocation. |
| Genuine process death | Independent child processes forcibly killed at Intent, Renamed, FileRemoved and Empty; fresh lock acquisition and unchanged game authority after restart. |
| Ownership authenticity | An edited seal with an added owner file before replacement cannot gain ownership through a new recovery snapshot. |
| Storage accounting | Tiny and 16 MiB payload fixtures with exact verified-byte arithmetic and independent desktop before/after inventory. |
| Completion ordering | A staged seal alone remains NotInstalled; only the final completion manifest makes that tree validate. |

Native Windows junction/reparse behavior was exercised; Linux remains deferred.
Symbolic-link refusal uses the same shared symlink-metadata/reparse checks, but
separate Windows file-symlink creation is not claimed by the junction case.
The actual creation attempt returned `Administrator privilege required for this
operation.` This remains an explicit acceptance gap rather than being counted
as passed from source inspection or a junction test.

The final diagnostic executable SHA-256 is
`92aa6c4f5908ed1282226635a872ef7071b84afc62e19aaaafd64ee4625e2f15`.
This is the manifest-last implementation, with only temporary diagnostic build
configuration. Closing production no-bundle build also passed (5m20s including
normal native build work); it was not installed or deployed.

## Windows desktop acceptance

All application interaction used Computer Use against exact temporary executable
paths; the owner's production window was never selected. The retained-handle
F3R-A driver only started/observed/killed its own children. No install or
maintenance IPC was invoked by the driver. The application stayed signed out.

Root: `%TEMP%/aurora-f3ra-F3RB-839b61924e3a4fb1834f0ff3cd9d5813`, exact F3RA marker.
Copied read-only inputs came from the earlier disposable F3R-A root, with
separate new registry/configuration and no accounts. Native replacement uses
real Vanilla 1.21.11, verified warm cache and managed Java. Two synthetic 16 MiB
storage fixtures use production durable journal promotion and explicit backdated
completion times for age testing; they are **not playable-game acceptance**.
The original legacy backup, rejected candidate and every prior F3R/F3R-A root
remain untouched. Raw logs, snapshots and accessibility evidence are retained
inside the new root; only compact curated evidence is committed.

An initial diagnostic build boot failed because absent updater configuration
deserialized as null. Rebuilding with the same temporary updater-key/configuration
merge used by F3R-A fixed this diagnostic setup. No repository updater,
identifier, release pipeline or production installation was changed. Initial
UI attempts with offscreen accessibility geometry were reobserved and performed
against current screenshots; they are not counted as successful actions.

The initial desktop matrix passed replacement, pre-/post-commit death/restart,
retention death/retry, mixed content and competing install refusal. It reclaimed
33,556,356 bytes across an interrupted invocation and its explicit resume,
then reclaimed zero on repetition. The final manifest-last build repeats the
matrix below; its final outcome is recorded in the closing evidence section.

### Closing final-executable results

| Scenario | Actual desktop evidence |
| --- | --- |
| A Successful replacement | PID 5132's explicit UI retry completed real Vanilla transaction `37109f4ca50249c6b51aa9b6a88c0c65`, deep validation and registry Ready (snapshot 043). Current manifest SHA-256 `b6b44d2ad8a02cf43812e9ed150fac4187af57b5d53d8ea402f04611a7e78dc5`. The previous sealed tree received a matching retention receipt. Legacy/missing-receipt backups remained protected; the synthetic completed backups retired only under the policy. |
| B Crash before commit | PID 27704 killed externally at BeforeCommit, transaction `71f550b4e0c74741b64eb27e8eb000d1`, journal Promoted and previous tree present (snapshot 032). Restart PID 2548 restored installation `97827a041550429e92198633e8889c7f`, manifest `c9be11617d1c14557050d551d9fd2f0808e432c6d74094476b00474a350c4b9e`, retained rejected candidate and displayed Previous installation restored. |
| C Crash after commit | PID 2548 killed externally at AfterCommit, transaction `a805b34e0862477c8cbe376edd552898`, journal Committed (snapshot 040). Restart PID 5132 retained candidate manifest `1ee22ab3f9068e471f45357e4be8ddb161f23998510630ea7b229ab19f1b47b5`, archived authority, cleaned staging and displayed Recovery completed with registry still Installing. |
| D Retention interruption | PID 22136 killed after one verified file deletion at RetentionFileRemoved. Restart PID 26764 explicitly resumed through Settings, completed two retirements, kept three backups and unchanged current game authority; repeated UI maintenance retired/reclaimed zero. |
| E Mixed content | The first final storage backup retained its extra owner-note.txt, unchanged SHA-256, and the app explained that uncertain ownership remained protected. Its complete backup tree survived. |
| F Concurrency | During first-matrix PID 1956's retention pause, actual desktop PID 10892's same-target Retry failed already-in-progress, the target displayed Installation busy and Forge stayed Ready. Vanilla installation and independent storage maintenance progressed together. Final-matrix PID 2548 also observed the other process's storage lock while its Vanilla retry progressed; PID 26764 resumed storage while that independent game operation was active. |
| G Repetition/restarts | Both desktop storage fixtures returned stable three-backup protection and zero further reclamation on repeated explicit maintenance. Fresh desktop processes retain current installation identities and completed retirement receipts; closing state/hash checks are recorded in compact evidence. |

Closing fresh desktop PIDs 29144 and 18684 both displayed all four fixtures
Ready. All 42 compared registry/game authority/ownership/history/retirement
documents remained byte-identical across those boots. The curated machine-readable
record is [docs/forge-f3rb/evidence.json](docs/forge-f3rb/evidence.json). Raw
accessibility/log evidence stays in the marked root. The mistakenly labeled
`final-repeated-retention-ui.txt` captured a different test window and is excluded;
the zero-reclamation repeat is established by the native final-resume log.

The final repeat initially tried Retry while a focus-triggered Restored-state
observation still owned the game/lifecycle locks. It safely returned busy, was
not counted as a successful install, and succeeded when retried after observation
settled. No verification or lock was bypassed. No file-symlink acceptance is
inferred from the successful A–G matrix: its separate privilege gate keeps the
overall F3R-B verdict PARTIAL.

## Measured costs and storage

These are measured disposable-fixture results, not estimates or a quiet-host
regression experiment. Compilation and another desktop installation sometimes
overlapped. The 16 MiB synthetic unit fixture measured 33,556,356 verified
historical bytes before, 16,778,178 after, one retired/two protected, 264 ms
game validation and 3,985 ms downstream engine time in the final install subset.

The first desktop storage fixture had five backup trees totaling **83,890,924
bytes** including its 34-byte owner-like sentinel. After interruption/resume:
three trees, **50,334,568 bytes**. Reclaimed: **33,556,356 bytes** (about 32.002 MiB).
The resumed invocation itself reclaimed 16,779,140 bytes after the killed
process had already removed a 16,777,216-byte leaf; it took 4,366 ms downstream.
Repetition: zero eligible/retired, three protected, zero reclaimed, 2,208 ms.
The owner-like sentinel retained SHA-256
`0548aa6e1aabdb47d5ac5af012bb8e692822ebec5a2c671d095f912a46c3cba7`.

The final-executable storage fixture independently repeated the same complete
before/after totals and 33,556,356-byte reclamation. Its explicit resume took
3,912 ms downstream, and repetition took 1,902 ms with zero reclaimed bytes.
Each fixture has its own journals/receipts and was generated through durable
promotion rather than copying metadata into an arbitrary backup. Combined across
the two desktop fixtures, measured reclamation is **67,112,712 bytes**; these
disposable test savings are not owner-storage savings.

The warm real Vanilla retry installed 4,669 cached artifacts with zero downloads:
97.505 s installer, 103.030 s lifecycle, 5.201 s final instance validation.
Ownership sealing cost 20.854 s in that debug run over a roughly 570.9 MB tree;
game-lock acquisition was 892 microseconds. These figures demonstrate a real
installation cost; they do not establish a percentage overhead against an
unmeasured equivalent baseline. Hashing/safety checks were not weakened for
performance. Maintenance never adds synchronous startup work.

The closing manifest-last executable's successful UI retry measured 81.266 s
installer / 85.961 s lifecycle, 4.423 s final instance validation, 17.941 s
ownership sealing and 478 microseconds game-lock acquisition. It again used
4,669 cache hits and zero downloads. The difference from the earlier debug run
is not a controlled optimization result. Closing read-only validation returned
Ready/no problems for all four copied/generated fixtures: Forge 18,543 ms,
real Vanilla 1,289 ms and each synthetic storage fixture 11 ms. The Forge sample
includes first-use/cache/host effects and is not a sealing measurement.

## Preservation and review handoff

All **883** initially tracked paths remain present; no tracked path was deleted
or renamed. All four nonignored pre-existing untracked file hashes match their
baseline, including the Python cache file. Protected local developer files and
existing worktrees were not altered. Production 1.4.2, credentials/accounts,
owner Minecraft instances/saves, public distribution, client/website sources,
release pipeline and historical acceptance evidence were outside write scope.
The normal production launcher process was left running throughout. No push,
publication, installer invocation or deployment occurred.

Local commits separate the design audit (`7c4ed08`), retention engine/tests
(`52396c4`), typed command/Settings exposure (`2b516d9`), manifest-last ordering
and isolated fixture generator (`e27c912`), and the closing acceptance record.
The last documentation commit's exact HEAD is reported in the final handoff
and a new temporary final-head record, avoiding a self-referential Git hash.
Full changed-file inventory is the branch diff from the starting HEAD; it
contains only installation retention/integration, its small Settings surface,
tests, architecture and compact documentation/evidence. No game, executable,
runtime, cache dump or large raw acceptance artifact is committed.

## Limits and deferred work

Automatic scheduling/startup/after-commit deletion is deliberately absent in
this first release. Owner-reviewed explicit maintenance is appropriate given
legacy ownership ambiguity, potentially substantial verification cost and the
lack of a historical user rollback UI. No storage-pressure policy can override
recovery/ownership protection. Unsealed legacy data, unknown additions, failed
operations and small terminal metadata can remain indefinitely.

Other filesystems, sudden power loss, actual disk exhaustion, malicious same-user
namespace replacement and Linux acceptance are not established by local NTFS
process termination. Windows directory durability remains the documented F3R
limit. No cross-process already-running game exclusion or coordinated registry/
Aurora/configuration rollback was added. A malformed receipt/temp sibling can
need manual inspection. The first release reports verified byte accounting and
counts, not a comprehensive storage browser.

Deferred per owner: Linux, authenticated F1C gameplay, D1-B3/additional Dev
cleanup, Drive changes, client mod/website work, release publication and
production 1.4.2 replacement. Recommended next phase: owner review of this
explicit maintenance policy and measured sealing cost, then a separately gated
performance study preserving ownership/hash/commit guarantees. Close the separate
file-symlink refusal case in an already-authorized environment before upgrading
this PARTIAL verdict. Automatic
maintenance or legacy adoption needs its own owner decision and acceptance.
