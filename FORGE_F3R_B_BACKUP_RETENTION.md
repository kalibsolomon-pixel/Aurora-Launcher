# F3R-B — managed installation backup retention

Work date: 2026-10-12. Starting branch `codex/client-3-production-authority`,
HEAD `c378af02710de61ee32361022e0aa545425cdbbc`. Acceptance is pending.

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
