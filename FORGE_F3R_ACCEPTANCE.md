# F3R-A — desktop, Linux and recovery acceptance

## Verdict and revision identity

**PARTIAL — Linux execution is unavailable.** Windows desktop A–D and closing
regression passed; the required Linux gates remain unexecuted. Deep installation validation is not authenticated
gameplay acceptance. No release, push, publication or production deployment is
part of this work.

Starting branch: `codex/client-3-production-authority`.
Starting HEAD: `c461eff2baddb099db52f5d56aa728e9f2f4afab`.
Implementation commit: `a506f5296966ade4714537dca1143ef5ce984819` — expose durable
recovery state and add isolated desktop acceptance diagnostics. The closing
native/UI correction is `adbcc869873e23cbce15a96406bd7ffbbb0ad23e` — retain
completed recovery until instance readiness is restored. The documentation/
evidence commit is identified in the final handoff.
The exact final delivery HEAD is written after the documentation commit to
`.zcode-diag/phase-f3ra-acceptance/final-head.txt`; this avoids a self-referential
commit hash inside its own report. The final response also records that hash.

## Scope, environment and isolation

Executed on Windows 11 Home, build 26300, x86_64, local NTFS, Rust 1.98.1
(`48a229cea`), Node 24.21.0 and Python 3.12.10; Intel Core i7-14700F (20 cores,
28 logical processors), 34,104,127,488 bytes physical RAM. These are real Tauri windows
with the compiled Svelte frontend, native commands/events and WebView2, operated
through their visible controls. Installs and retries were initiated in the UI.
The external controller only starts, observes, snapshots, pauses or terminates
its own retained child process handles. It never invokes an installation API.

Run roots, all direct children of the OS temporary directory:

- Initial investigation: `aurora-f3ra-20261012-061027`.
- Corrected seven-instance desktop matrix: `aurora-f3ra-D2`.
- Malformed-journal clone: `aurora-f3ra-manual`.

Each contains the exact `F3RA-ISOLATED.txt` marker. Managed configuration,
registry, cache, runtimes and instances are under that run's `managed/`;
diagnostics and transaction snapshots are under `evidence/`; every executable
copy has its own `webview-<label>/` profile. No owner configuration or account
document was copied. The applications remained signed out. Production
`%LOCALAPPDATA%/Aurora Launcher/aurora-launcher.exe` was never replaced.

The debug-only `f3ra-desktop-acceptance` feature authorizes only marked direct
temporary children and an exact validated instance identity. It parks at existing
transaction checkpoints for at most 300 seconds. There is no frontend crash
command, IPC trigger or release-build hook. An external retained handle performs
the actual force kill. Release compilation excludes the hook even if the Cargo
feature is selected. No transaction ordering or artifact verification was relaxed.

Executable identities, exact PIDs and wall-clock observations are preserved in
`evidence/desktop-processes.json`. The original F3R behavior plus observation
hook used SHA-256 `7ef37d81491b980fabe317f750ebdb30074cbeda590ae88cc665366d6702c1bf`.
The main replacement/recovery matrix executable used SHA-256
`cb56d5e28e8f3e2627418f4d2fa1851cf3c519a88d6a51e386eaf4f197e34b53`.
The final correction, retry and restart executable used SHA-256
`ab2822ce173f81ac8014e3e19249efe16a3a8966d473282154b8feafb65a8773`.
Earlier UI builds and their evidence are retained and identified in the index.

## Transaction authority

The durable sequence remains Prepared → Replacing (before old-tree rename) →
Promoting (before candidate rename) → Promoted (before post-promotion deep
verification) → Committed (before archive/staging cleanup). A valid installed
manifest alone does not supersede an uncommitted operation. Protected previous
trees live outside disposable staging. Recovery validates identities and trees,
records Restoring, quarantines the recognized candidate, restores the recognized
previous tree, verifies it, and records Restored. Prepared-only attempts become
Abandoned. Valid Committed candidates retain authority. Unknown, malformed,
redirected or conflicting evidence fails closed and preserves trees.

Game recovery does not promote the instance registry to Ready. A restored or
committed game may still have an Installing registry record; explicit retry must
complete the configured lifecycle and its deep validation. The lifecycle lock
precedes the game lock. Startup and focus now observe lifecycle ownership even
before a journal exists, and reload the target's registry record under that lock.

## Windows desktop A — independent processes

**PASS.** `A2-owner` PID 25372 held Forge 1.20.1 at Prepared while a separate
actual desktop (`profile2`, PID 19348) attempted the same target. The contender
displayed Installation busy, and native retry failed with
`installation_already_in_progress`. It did not publish Ready or replace the
owner's journal. Killing the retained owner released kernel ownership; the
remaining application replayed Prepared → Abandoned and retained the original
valid game manifest.

With that owner still parked, the second desktop installed Forge 1.20.4,
transaction `80e55df516b54aa8b7087a1cd4a93d70`, through real processors and
verified staging to completion. Thus different targets progressed concurrently;
this is not a dual-download throughput benchmark. Snapshots 015/016 and native
logs correlate the two independent targets. No global install mutex was added.

Closing-build repeat: contender PID 25268 attempted the same target while PID
27904 held BeforeCommit. The final UI displayed Installation busy, native retry
returned the same busy error, and no Ready was published for that target.

## Windows desktop B — replacement interruption

The deeply validated Forge 1.20.1 original has installation identity
`install-1791788666616-27580`, manifest SHA-256
`06044ce2f07f1bde73f6b2a7c1b042fb3a1cff47534240ee098060df3ad49f60`,
and journal tree SHA-256
`149040bcf1cf0642ab79be3c7bc7667536186dbe4ad67b69421823b39f7b21ba`.
The same verified previous identity was preserved between cases; no damaged
fixture was recycled.

| External kill checkpoint | Evidence and restart result |
| --- | --- |
| Prepared | A2-owner; original game retained, operation Abandoned, UI Installation interrupted. |
| AfterBackup / Replacing | PID 26236, transaction `c17949c4ddf0490084e5f6148c129350`, snapshot 024: game absent, original in previous-game, verified candidate in staging. Restart restored the exact original and displayed Previous installation restored. |
| AfterPromotion / Promoting | PID 2572, transaction `a768d49abdbf4ed0b00ca1a4d3323a2d`, snapshot 032: candidate active, original protected. Restart followed the recovery path below and restored the original. |
| BeforeCommit / Promoted | PID 27904, transaction `22726d094e964b00bc10c7b65cb033ba`, snapshot 045: verified candidate active but uncommitted. Restart restored the original (snapshot 050) and displayed Previous installation restored. |
| AfterCommit / Committed | PID 19372, transaction `4ba62538025d47cdad94625a63cf8785`, snapshot 051. Restart PID 20464 retained candidate manifest `db0fe77640b14dff2fb8032efa259c0d4b350ceef5aea4f39e119196c4a403b5`, completed archive/staging cleanup, and kept registry Installing (snapshot 054). |

The separate malformed-journal clone began with a valid Vanilla tree and a
stored Ready record. Real desktop PID 22464 displayed Recovery required with
manual-inspection guidance. It neither exposed Ready nor overwrote the journal.
All 4,697 instance files, including the deliberately malformed journal, matched
their before hashes; no instance files appeared or disappeared.
The final corrected executable repeated this refusal in PID 26612; the same
4,697 files and malformed journal again matched their original hashes.

Final corrected desktop PID 6496 retained Recovery completed through discovery
reloads, then its explicit UI retry completed transaction
`b3f62b0c270946dd88ebe5c794ec5794` and deeply validated the instance Ready.
Snapshot 057 records final candidate manifest
`0179b684fa1b1024f990580ac472544177a025dc4adaef8df905f2256fe3489f`.
Three fresh boots (PIDs 25896/28292/27096) retained identical journal, registry
record and all game/history manifest identities (snapshots 060/063/066).
The last two desktop lists settled to seven Ready labels; the first list was
captured during its focus check. Read-only restart did not create another
transaction, remove history or change commit authority.

## Windows desktop C — interruption of recovery itself

**PASS.** The same interrupted transaction `a768d49...` was replayed across
three separately killed real desktop processes:

| Checkpoint | PID | Snapshot | Filesystem identity |
| --- | --- | --- | --- |
| Restoring | 5224 | 035 | Recognized candidate still active; original previous-game retained. |
| AfterQuarantine | 22820 | 038 | game absent; candidate in rejected-game; original previous-game retained. |
| AfterRestore | 11280 | 041 | Exact original active; rejected candidate retained; journal still Restoring. |

The next desktop completed Restored (snapshot 044). The previous manifest and
tree digest matched the original; the candidate's manifest stayed
`ed1d1d69e80405bf79da5624b89aae5841f886d81c03143c4ccce272e9c6b9bf` in
quarantine. No duplicate destructive rename, lost backup or authority reversal
occurred. While recovery was paused, the rendered frontend remained responsive
and said it was checking saved state/recovering interrupted installations; no
Ready label or Play action was presented for an unresolved target.

## Windows desktop D — real processor parent death

**PASS.** UI retry in PID 19348 entered a real Forge 1.20.4 processor with receipt
transaction `c6da8adfb7fd421cb7288a24f8a95b5b` at processing. Java PID 6740 was
identified as that launcher's child and opened through a retained kernel handle.
Two reads 200 ms apart returned STILL_ACTIVE (259), before the launcher kill.
After force-killing the retained parent, the Java handle signaled within the
10-second bound; `WaitForSingleObject=0`, exit code 0, childTerminated=true.
This is independent child observation, not inference from parent disappearance.

Snapshot 022 retained Prepared journal/processing receipt, no staged completion
manifest, and the unchanged valid previous tree. Restart PID 17280 replayed the
attempt as interrupted/Abandoned. Its explicit UI retry completed transaction
`df1cce497dba4948a819bf1c60dcb1b6`, registry Ready, then read-only deep validation
returned Ready with no problems (6,064 ms). The final desktop screenshot also
shows Forge 1.20.4 Ready. This proves that observed child terminated; it does not
claim observation of arbitrary hypothetical grandchildren.

## Desktop lifecycle corrections

The optional typed recovery summary conveys busy, interrupted, previous restored,
completed or manual repair without exposing raw journals or launch arguments.
Home, list and workspace share one presentation decision; recovery outranks
stored/cached Ready. Initial load and focus checks say what is being checked.
Installation immediately invalidates displayed Ready while metadata is resolving.
Focus refreshes coalesce; generation checks reject older state responses; failure
ends Checking with a retryable error. Native state/recovery work moved off the
Tauri UI thread. State readers serialize within a process so they do not mistake
each other's temporary lifecycle lock for a competing installer. There is no
background poll or automatic second install. Narrow list styling keeps long
recovery explanations readable beside retry controls at the real desktop size.
The final commit-boundary run exposed a read-only discovery reload dropping the
Completed explanation while correctly retaining Installing. Terminal journal
presentation now retains Completed for an incomplete instance across reloads,
and suppresses it after Ready. This does not alter journal or registry authority.

## Linux stage — not executed

**BLOCKER: no Linux execution environment.** `wsl --list --verbose` reported
Windows Subsystem for Linux not installed; Docker was unavailable; no dedicated
Linux machine, VM or authorized remote host was supplied. No operating system,
host component, boot configuration or disk layout was installed or changed.

Linux cross-process contention, SIGKILL lock release, aliases/symlink refusal,
descriptor lifetime, cancellation, native-filesystem replacement/replay,
permission/cross-device errors, directory fsync, processor parent death and the
graphical desktop are all **UNEXECUTED**, not passed by Windows tests or source
inspection. No distribution/kernel/filesystem/mount measurements exist. In
particular, a Unix process group alone is not evidence that SIGKILL of its parent
terminates Java; the required real process-tree test remains open. Any correction
must be evidence-driven and preserve Windows Job Object behavior.

## Automated and loader regression

Baseline reproduced: 941 Rust unit tests plus 6 integration tests passed,
30 ignored; 266 frontend tests passed. Closing source checks:

- `npm run check`: zero errors/warnings.
- Complete frontend suite: 270 passed, zero failures.
- `cargo fmt --all -- --check` and `cargo check --all-targets`: passed.
- Default-feature Rust suite with `--release`: 942 unit and 6 integration passed, 30 ignored.
- Final debug acceptance-feature Rust suite: 943 passed, 30 ignored.
- Frontend production build and final native production build: passed.
- Actual final-source Tauri boots and frontend/native communication: observed.

The additional tests cover recovery precedence over cached Ready, completed-but-
Installing state, focus coalescing, failed refresh, obsolete responses, native
lifecycle contention/manual repair isolation and the temporary-root authorization
guard. Existing journal replay tests additionally assert terminal presentation.
Ignored tests remain ignored; no ignored benchmark is counted as a passing test.
Existing build warnings are recorded in logs. No dependency/lockfile changed.

All seven copied fixtures deeply validated Ready before fault testing: Forge
1.20.1/47.4.10, Forge 1.20.4/49.2.0, Forge 1.21.11/61.2.0, Vanilla 1.21.11,
Fabric 1.21.11/0.19.5, NeoForge 26.2/26.2.0.88, and Aurora-on-Fabric 1.21.11.
Closing quiet validation again returned Ready/no problems for all seven, in
7,782 ms summed target-validation time (release validator). Individual times:
Forge 1.20.1 1,149 ms; Forge 1.20.4 1,162; Forge 1.21.11 1,122; Vanilla 1,066;
Fabric 1,044; NeoForge 1,175; Aurora/Fabric 1,064. This closing check is not the
quiet replacement performance comparison. Its first attempt overlapped a desktop
startup validation and correctly reported busy for Forge 1.20.4; that attempt is
retained, and the passing run was made after all run-owned test windows closed.

## Performance — reproducible warm comparison

One quiet A–B–A desktop series used the same copied Forge 1.20.4 target, concrete
pins, verified cache, managed Java and NTFS volume. No bulk hashing or compilation
ran concurrently with these measured installs. Every run had 3,915 cache hits,
zero downloads. A is starting F3R behavior with the observation hook; B adds the
diagnostics and UI/native presentation corrections. This is not a pre-F3R versus
F3R comparison and does not isolate every change independently.

| Run | Installer ms | Lifecycle ms | Assets ms | Staged validation ms | Commit ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Baseline first warm | 114,128 | 121,458 | 16,644 | 30,123 | 46,626 |
| Instrumented | 98,894 | 104,769 | 4,095 | 28,764 | 47,674 |
| Baseline warmed again | 97,288 | 103,265 | 3,742 | 28,339 | 46,739 |

The instrumented run was 1.65% above the fully warmed baseline installer and
1.46% above its lifecycle in this single pairing. The first baseline's slower
asset phase demonstrates warm-cache variance; these samples establish neither
a statistically significant regression nor a speedup.

For B, each of four tree captures covered 3,936 files / 835,886,854 bytes:

| Component | Measured cost |
| --- | --- |
| Manifest parsing per capture | 18.385–25.282 ms |
| Tree enumeration per capture | 86.525–93.675 ms |
| Deep manifest/file validation per capture | 4.905–5.148 s |
| SHA-256 whole-tree hashing per capture | 10.328–10.598 s |
| Installed-state write | 24.091 ms |
| Staged tree synchronization | 468.413 ms |
| Journal durable writes | 0.933–2.223 ms |
| Checked rename preparation | 85.502 / 86.067 ms |
| Old-tree / new-tree rename | 3.554 / 3.524 ms |
| Post-promotion verification (includes fourth capture) | 15.360 s |
| Archive / cleanup bookkeeping | 1.491 / 1.690 ms |
| Game lock acquisition | 538 microseconds |

Capture costs must not be added again to overlapping commit/post-promotion
aggregates. Four captures re-observe old authority at preparation and commit,
the candidate before promotion, and the active promoted tree. Repeated hashes
are the dominant measured durability cost. No hash, sync, lock, installed-marker
ordering or post-promotion validation was removed. A future bounded experiment
could evaluate one traversal computing required manifest checks and tree digest
together, while preserving every path/identity/trust check and all separate
observation windows. Reusing trust across copies or skipping final validation is
not an accepted optimization.

## Retention proposal — no garbage collection implemented

A retained Forge 1.20.4 game is about 835.9 MB (797.2 MiB); Forge 1.20.1 about
826.0 MB (787.7 MiB). Repeated successful replacements and quarantined candidates
therefore accumulate materially. The closing evidence inventory quantifies this
run: ten trees / 38,520 files / 8,309,325,955 bytes (including fault candidates
and the excluded timeout's quarantine). The last successful retry added one
826.0 MB previous-game tree. All diagnostic trees are retained.

Proposed follow-up: an explicit owner-reviewed maintenance action, holding the
same lifecycle/game locks, may retire only an exact manifest-proven managed
history tree after terminal journal validation and deep validation of the active
game. Protect every unresolved transaction, manual-repair evidence, only valid
previous installation, and at least the most recent proven valid previous pin.
Persist cleanup intent before an atomic rename into an exact cleanup quarantine;
make restart replay idempotent and test every deletion boundary. Reject unknown,
malformed, linked, redirected or conflicting paths. Never inspect/delete instance
user mods, config, worlds, logs or other user state as part of retirement. Age or
disk quotas alone cannot authorize deletion. Add explicit space reservation and
user choice in a dedicated phase; no automatic pruning, cache eviction or hidden
retention preference ships here.

## Durability limits, preservation and next decisions

This exercises Windows local-NTFS process death, not sudden power failure,
controller write-cache loss, actual host disk exhaustion, every antivirus/file-
sharing interaction, network filesystems, FAT or mounted cross-device faults.
Windows directory persistence has the same documented limitations as F3R;
Unix directory-fsync guarantees remain unexecuted. Cross-process install exclusion
is not cross-process launch exclusion. No coordinated rollback of registry,
desired configuration, Aurora or user content is claimed.

Preservation verification passed: the initial 842 tracked paths, all 15,257
pre-existing untracked hashes, original F1C source fixtures including long paths,
copied instance user/Aurora files, and protected production/auth documents.
The exact result is retained in `preservation-closing.json` and the curated
evidence index: 2,640,127,634 pre-existing untracked bytes, 6,871,641,577 original
F1C managed bytes (42,654 files), five copied user/Aurora files (5,504,125 bytes),
and four protected production/auth documents all matched. No existing diagnostic
evidence was cleaned or rewritten;
no owner cache, Minecraft root, world, credential or production install was a
fault target. `.minecraft` remained off-limits. No deletion/rename is staged.

Excluded attempts remain visible: the initial copy omitted ten Windows long-path
files, making that *copy* damaged. The original fixture was intact. A new root
was copied with extended paths and all seven validated before acceptance. An
initial processor observer read the wrong receipt member and missed the active
processor; that install completed normally and was not counted as a kill. One
AfterPromotion pause expired and safely restored the old tree; the matrix uses
the later externally killed repeat. Early UI builds exposed self-contention and
stale Checking responses; corrected source was checked and booted. These are not
silently relabeled successful acceptance cases.

F1C authenticated Forge 1.20.1/1.20.4 gameplay remains the separate owner
requirement. No password, login, token or gameplay was used in F3R-A. Owner
decisions: provide an authorized Linux native-filesystem/desktop environment;
review the UI/durability changes and backup-retention proposal; complete F1C
gameplay separately. Recommended next phase is Linux A–D equivalent execution,
including real SIGKILL child observation, followed by reliability review. Do not
substitute new loaders, providers or release publication for that missing gate.

## Changed-file inventory

Native implementation and acceptance: `src-tauri/Cargo.toml`,
`src-tauri/src/{application,desktop_acceptance,lib}.rs`,
`src-tauri/src/install/{journal,mod}.rs`, and
`src-tauri/examples/{f3ra_desktop_driver.py,f3ra_validate.rs}`.
Frontend: `src/lib/backend.ts`, `src/lib/instances/InstanceWorkspace.svelte`,
`src/lib/launcher/{instanceStatus.ts,instanceStatus.test.ts,startupReadiness.test.ts,store.svelte.ts}`,
`src/lib/pages/{HomePage,InstancesPage}.svelte`, and `src/routes/+page.svelte`.
Documentation: this report, the current F3R-A architecture contract, and an appended acceptance addendum to
`FORGE_F3R_DURABLE_OPERATIONS.md`. Curated screenshots, observations, summaries
and preservation evidence are new under `.zcode-diag/phase-f3ra-acceptance/`.
The exact delivery inventory is retained in that directory's
`changed-files.txt`. No pre-existing tracked file was deleted or renamed.

The final default debug `cargo test` build encountered the owner's running
development executable and Windows refused removal. The owner process was left
open; the complete default suite was rerun with `--release`, and debug feature
unit tests use `--lib`. That build refusal is retained separately, not counted
as a completed test run or an application fault acceptance case.
