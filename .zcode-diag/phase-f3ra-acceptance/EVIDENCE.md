# F3R-A evidence index — 2026-10-12

Overall verdict: **PARTIAL; Linux engine and desktop unexecuted**.
Full report: [docs/compatibility/FORGE_F3R_ACCEPTANCE.md](../../docs/compatibility/FORGE_F3R_ACCEPTANCE.md).
Implementation starts at `c461eff2baddb099db52f5d56aa728e9f2f4afab` on
`codex/client-3-production-authority`. Exact delivery revision is recorded in
`final-head.txt` after the closing documentation commit. No push/publication.

## Retained raw roots

These are new disposable roots, not the owner's existing diagnostic roots:

```text
C:\Users\kalib\AppData\Local\Temp\aurora-f3ra-20261012-061027
C:\Users\kalib\AppData\Local\Temp\aurora-f3ra-D2
C:\Users\kalib\AppData\Local\Temp\aurora-f3ra-manual
```

The first root contains `baseline.json` (842 tracked paths and 15,257
pre-existing untracked size/SHA-256 entries), `protected-baseline.json`, environment
and reproduced baseline check logs. The D2 root contains corrected copied
fixtures, `source-fixture-hashes.json`, complete build/test logs and the actual
desktop's stdout/stderr under `evidence/<label>.log`. Executables, games, caches,
runtimes and WebView profiles remain there and are not committed. No raw evidence
was deleted to make a result look successful.

`evidence/desktop-processes.json` is the controller's append-preserving record:
labels, PIDs, executable SHA-256, retained-handle kills, child observations,
snapshot paths and Unix timestamps. Checkpoints are `checkpoint-<pid>-<point>.json`.
Snapshots include the registry record, journal, receipt and exact installed
manifest hashes. See `matrix-observations.json` for the curated extraction.

## Windows observations

| Scenario | Principal correlation |
| --- | --- |
| A same target | A2-owner PID 25372 / profile2 PID 19348; owner Prepared, contender busy; owner kill releases kernel lock. |
| A independent targets | Snapshots 015/016: owner remains Prepared on Forge 1.20.1 while Forge 1.20.4 reaches verified staging; transaction `80e55df516b54aa8b7087a1cd4a93d70` completes. |
| A final-build contention | PID 27904 BeforeCommit / contender 25268; [A-final-busy.png](A-final-busy.png), matching accessibility text and native error. |
| B AfterBackup | PID 26236, snapshot 024, transaction `c17949c4ddf0490084e5f6148c129350`; [B-after-backup-status.png](B-after-backup-status.png). |
| B AfterPromotion | PID 2572, snapshot 032, transaction `a768d49abdbf4ed0b00ca1a4d3323a2d`; C replay below restores old authority. |
| B BeforeCommit | PID 27904, snapshot 045, transaction `22726d094e964b00bc10c7b65cb033ba`; snapshot 050 restored, [B-before-commit-restored.png](B-before-commit-restored.png). |
| B AfterCommit | PID 19372, snapshot 051, transaction `4ba62538025d47cdad94625a63cf8785`; snapshot 054 retains exact committed candidate, registry Installing. |
| C recovery kills | PIDs 5224/22820/11280, snapshots 035/038/041; [C-recovery-in-progress.png](C-recovery-in-progress.png), [C-restored-after-three-kills.png](C-restored-after-three-kills.png), snapshot 044 Restored. |
| D real Java kill | profile2 PID 19348 / Java 6740, `D-processing-observed.json`, snapshot 022, retained handle STILL_ACTIVE twice then signaled after parent kill. |
| D successful retry | PID 17280, `D-deep-ready.log` Ready/no problems; [D-final-ready.png](D-final-ready.png). |
| Manual repair | Separate clone PID 22464, [manual-repair.png](manual-repair.png), [manual-preservation.json](manual-preservation.json): all 4,697 instance files unchanged. |
| Final completion/retry | Final executable SHA-256 `ab2822ce173f81ac8014e3e19249efe16a3a8966d473282154b8feafb65a8773`, PID 6496; [B-completion-fixed.png](B-completion-fixed.png), [final-seven-desktop-ready.png](final-seven-desktop-ready.png). |
| Repeated restart | Snapshots 057/060/063/066, PIDs 25896/28292/27096; [repeated-recovery.json](repeated-recovery.json), [repeat-3.png](repeat-3.png). Last two lists settled to seven Ready labels. |
| Final manual refusal | PID 26612, [manual-final.png](manual-final.png), [manual-final-preservation.json](manual-final-preservation.json); stored Ready overridden, same 4,697 files unchanged. |

Screenshots show the real application at its tested desktop size; adjacent `.txt`
files retain the accessibility tree. The final D screenshot comes from another
real window observing the now-Ready Forge 1.20.4 target, alongside the restored
Forge 1.20.1 case. An earlier D screenshot was occluded and is retained only in
raw evidence, not used as visual proof.

## Reproduction boundary

The guarded feature is debug-only. Build the frontend, then build the actual
Tauri binary with `tauri/custom-protocol,f3ra-desktop-acceptance` to an isolated
output path. The diagnostic TAURI_CONFIG merge labels its window and clears the
updater key; production repository configuration/identifier remain unchanged.
Create a new direct `aurora-f3ra-*` child of the OS temporary directory with exact
marker bytes `F3RA isolated desktop acceptance\n` and `managed/`. Copy verified
fixtures using Windows extended paths; validate all seven before introducing a
fault. Do not copy accounts or the owner's configuration. Record original hashes.

Start `src-tauri/examples/f3ra_desktop_driver.py <marked-root>`. Its `start`
command copies the isolated executable per label and provides a separate WebView
profile, managed-root override and optional existing transition checkpoint.
Use the real desktop install/retry control, observe the checkpoint, request a
snapshot, then `kill` the label through the controller's retained handle. Restart
the actual application for replay. A 300-second pause expiration is an error
rollback, **not** an accepted externally killed case. The processor watcher uses
retained Windows handles; it never kills Java by process name.

`f3ra_validate` is read-only, network-free, guarded to a marked new temporary
root, and optionally accepts a validated instance ID. It supplements the real
desktop outcomes; it is not a substitute for them.

## Checks and measurements

Raw logs in D2: `closing-ui-check.log`, `closing-ui-tests.log`,
`closing-ui-build.log`, `completed-rust-fmt.log`, `completed-rust-check.log`,
`completed-release-tests.log`, `completed-feature-tests.log`,
`completed-production-build.log`, `desktop-completed-build.log` and the closing
loader/restart files. `checks-summary.json` records final outcomes and log hashes.
The debug all-target test build refusal against the owner's open executable is
retained in `completed-rust-tests.log`; the full suite passed in the release
target and debug unit suite passed with `--lib`. The first closing validator
overlapped desktop startup and reported one busy target (`closing-seven-ready.log`);
the quiet all-seven pass is `closing-seven-quiet.log` and
[loader-regression.txt](loader-regression.txt). No busy result was counted Ready.

Quiet A–B–A performance: `baseline.log`, `profile2.log`, `baseline2.log` under
raw `evidence/`. Use the first successful transaction in profile2, before its
later fault scenarios. `performance.json` preserves those exact aggregate and
component lines. Four capture measurements overlap commit/post-promotion totals;
do not double-count them. The comparison is starting F3R versus instrumented
F3R-A, not pre-F3R versus F3R, and has one sample per position.

Linux: raw `environment.json` records WSL not installed, Docker unavailable and
no supplied Linux host. There are no Linux execution logs or pass claims.

## Preservation and exclusions

[preservation-closing.json](preservation-closing.json): PASS. All 15,257 original
untracked files (2,640,127,634 bytes), all 42,654 original F1C files
(6,871,641,577 bytes), five copied user/Aurora files and four protected
production/auth documents matched. All 842 original tracked paths and critical
documentation/manifests/lockfiles/build configuration remain. No deletion diff.
Source fixtures were read-only; fault targets were marked new roots. Production
1.4.2 SHA-256 remains
`711df646d834588b404c7170db76b6546b409433aecfd882b026299412d13818`.
The later [preservation-final-recheck.json](preservation-final-recheck.json)
again matched all pre-existing untracked/protected files and tracked presence.

Excluded, retained attempts: an initial copy silently skipped ten Windows
long-path files (fixed by a fresh extended-path copy, then seven deep Ready
checks); a processor observer used the wrong receipt member and missed processing
(no kill; normal success); a bounded AfterPromotion pause expired (old tree safely
restored; later kill repeat used); early UI self-contention/stale Checking and
the transient Completed explanation were corrected and retested. Original
fixtures and historical evidence were not rewritten to conceal these attempts.

Backup retention is a proposal only. All old/rejected trees remain in the new
run roots: [retention.json](retention.json) counts ten trees / 38,520 files /
8,309,325,955 bytes. [raw-evidence-hashes.json](raw-evidence-hashes.json) binds the
124 retained D2 evidence/check files to their SHA-256/size. F1C authenticated gameplay remains owner-pending. Windows NTFS process
death is not a universal power-loss, network-filesystem or Linux acceptance claim.
