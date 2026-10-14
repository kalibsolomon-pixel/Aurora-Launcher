# F3R-B1 — Windows symlink security closure, sealing performance, final regression

> F3R-B2a correction (2026-10-14): the historical containment/cleanup and
> enforced-unelevated claims below were too strong. The old wrapper deleted
> caller-supplied roots, and neither phase checked token elevation. Installer
> identity matching also omitted extensions. See
> [FORGE_F3R_B2A_REVIEW_CORRECTIONS.md](FORGE_F3R_B2A_REVIEW_CORRECTIONS.md)
> for the corrected implementation, exact evidence and remaining blocked gate.
> Historical outcomes below are preserved; they do not close the real-link gate.


Work date: 2026-10-13. Starting branch `codex/client-3-production-authority`,
HEAD `395d5614088ed36481c9823cbc5ef263822b9270` (the reported F3R-B HEAD).
**Verdict: PARTIAL — every executed gate passed, including a 5.8× median
ownership-sealing reduction with byte-identical seals, but the separate real
Windows file-symlink security gate is BLOCKED: this host's standard user
cannot create a file symbolic link and no privilege change was authorized.**
No Windows security setting was changed.

## 1. Baseline and preservation

889 tracked paths at start; four pre-existing nonignored untracked items
(`.zcode-icon-diag.py`, `.zcodeignore`,
`UserskalibAppDataLocalTempaurora-phase-d-final-evidencesodium-exit-zero-ready.txt`,
`docs/performance-p0-2/__pycache__/`) recorded and preserved byte-for-byte.
Five pre-existing worktrees recorded; none touched. The owner's production
Launcher 1.4.2 (`%LOCALAPPDATA%\Aurora Launcher\aurora-launcher.exe`, PID
21100) stayed running and untouched throughout. Two earlier disposable
acceptance roots in `%TEMP%` were discovered from an interrupted parallel
F3R-B1 attempt (00:58–01:19 EDT, `aurora-f3rb1-457bae31…`,
`aurora-f3ra-F3RB-B1-457bae31…`); they are owner property, were not modified,
and their logs were read only as corroborating evidence (their symlink probe
recorded the same `WinError 1314`; their instrumented real-tree profile
measured the two sealing passes at 15.1 s / 15.8 s in a debug build). That
attempt's own work survives unmerged on branch `codex/f3rb1-acceptance`
(commit `4f2ad56`, acceptance runners only); it was not modified or merged
here.

## 2. The unresolved file-symlink gate (PART A)

The F3R-B gap: `install/retention.rs` refuses every reparse point through
`redirected()` (symlink file type or `FILE_ATTRIBUTE_REPARSE_POINT`), but no
test ever placed a REAL file symbolic link inside a candidate backup — F3R-B's
creation attempt failed with "Administrator privilege required for this
operation."

Privilege assessment on this host (user `SPXCTER\kalib`):

- Not elevated; not in the Administrators group.
- `SeCreateSymbolicLinkPrivilege`: not held in the token.
- Developer Mode: off (`AppModelUnlock` key absent).
- PowerShell 5.1 `New-Item -ItemType SymbolicLink`: "Administrator privilege
  required for this operation." (the exact F3R-B error).
- `cmd mklink`: "You do not have sufficient privilege to perform this
  operation."
- Python `os.symlink` (passes `SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE`):
  `WinError 1314 — A required privilege is not held by the client` (proves
  Developer Mode is off, not merely unflagged).
- pwsh 7: not installed. WSL: not installed.

**Gate verdict: BLOCKED.** Per the acceptance rules a creation refusal is
never counted as a pass, and junction coverage is not a substitute. The
existing junction-refusal test remains green and junction coverage after
eligibility was added (§4).

Closing the gate needs one owner-authorized action, unchanged in scope from
F3R-B's proposal. Smallest options: (a) temporarily enable Windows Developer
Mode and run `docs/forge-f3rb1/run-symlink-gate.ps1` as the normal user, or
(b) run that same script once from an elevated PowerShell. The script changes
no security setting, works only inside a fresh marked
`aurora-f3rb1-symlink-*` direct child of `%TEMP%`, invokes the real retention
engine, asserts the sentinel's SHA-256 before/after, FAILS LOUDLY as BLOCKED
if the link does not truly exist, and removes only its own root. The
`windows_file_symlink_refusal_helper` test it drives is inert (early return)
in ordinary suite runs, so it never silently passes.

## 3. Real-symlink fixture design (ready to execute)

`windows_file_symlink_refusal_helper` builds the F3R-B-specified fixture
inside its isolated root: a real managed backup history (three promoted
replacements, backdated receipts), a sentinel file OUTSIDE the managed tree
with recorded SHA-256, a pre/post inventory, one real file symbolic link and
one dangling link inside the candidate `previous-game`, both pointing outside.
When the environment permits creation it asserts: both links exist before the
assertion runs; retention refuses (`RetainedUnknownOwnership`); reclaimed
bytes 0; the whole mixed tree preserved; sentinel bytes and SHA-256 unchanged;
owner sentinels unchanged; journal authority bytes unchanged; current game
still deeply valid; repeated invocation idempotent. Creation runs twice
(two maintenance passes) per the resumed-maintenance requirement.

## 4. Filesystem edge-case review (§8)

Covered before this phase: junctions (real, default suite), traversal/ADS/
device/case aliases, reparse points during enumeration (every entry's
metadata is checked), locked Windows leaf, unknown empty directories,
reparse substitution between seal and disk copies. Added now:
`junction_substituted_after_eligibility_refuses_retirement` — a junction
substituted AFTER durable `Deleting` intent still refuses every remaining
deletion and the retirement only completes once the tree is exactly the
sealed content. Dangling links and relative targets are covered by the same
reparse-attribute refusal (`symlink_metadata` regardless of target
existence); target resolution is never attempted. Real directory-symlink and
file-symlink coverage remain privilege-gated together with the §2 gate.
Time-of-check/time-of-use by a same-user adversary remains a documented,
unchanged limitation: metadata is checked at enumeration/open and again at
deletion time, but no cross-handle verification is claimed.

## 5. Ownership-sealing investigation (PART B)

What sealing included (F3R-B code): (1) per manifest file, ancestor
containment walk + open + hash; (2) per native, a second read compared with
the extraction observation; (3) a full `inspect` walk re-canonicalizing the
root per directory and RE-HASHING EVERY FILE; (4) serialization + durable
write. Within one synchronous call under the game lock in private staging,
(3) repeated (1)'s computation on the same immutable observation — the only
category the phase allows optimizing. Other full-tree reads stay: staged
validation (plan digests), pre-rename and post-rename promotion captures
(deep validation + whole-tree SHA-256) — each at its own transaction
boundary, untouched.

Deterministic benchmark (`ownership_sealing_benchmark_helper`,
opt-in via `AURORA_F3RB1_SEAL_BENCH`), fixture mirroring the measured warm
Vanilla profile (4,616 files / 532,420,608 bytes, seeded content, real
`seal_staging`):

| Phase (release, quiet host) | Baseline code | Optimized code |
| --- | --- | --- |
| enumeration | 247–263 ms | 233–246 ms |
| one open+hash pass (532 MB) | 417–492 ms | 417–424 ms |
| per-file containment pass | 531–536 ms | — (eliminated) |
| `seal_staging` median | **3,311.7 ms** | **570.5 ms** |
| samples | 3.05–3.44 s | 0.56–0.71 s |
| seal SHA-256 | `ce847314…dae6` | `ce847314…dae6` (identical) |

A noisier first baseline run measured a 7,259 ms median with a 904 ms first
hash pass — host antivirus/caching roughly doubles absolute numbers, which
is why medians of 5 samples on matched quiet-host conditions are the
comparison. Structural accounting is host-independent: full-content reads
inside sealing drop 2 → 1 (1,064.8 MB → 532.4 MB), per-file ancestor
canonicalization drops 2/file → 0 (metadata now comes from the enumeration
itself), directory re-canonicalization drops to one root check.

Real-app corroboration: the desktop reinstall of the real Vanilla fixture
(4,669 files / 570.9 MB) sealed in **7.533 s** in the debug-profile
diagnostic build; the interrupted parallel session's instrumented baseline
measured 15.1 s + 15.8 s for the same two passes in debug, and F3R-B
measured 17.941 s in its release desktop. F3R-B's 85.961 s lifecycle figure
would lose roughly 10–17 s of sealing work at release speed.

## 6. The optimization and its safety argument

`seal_staging` is now ONE immutable observation: validate the root once,
enumerate the staged tree once (bounded depth 64 / 200,000 entries), refuse
every reparse point and non-regular entry from enumeration metadata, refuse
any directory that is not an ancestor of an installer-supplied path, refuse
any file that is not manifest-recorded, extracted-native, or the completion
manifest, hash each expected file exactly once, require on-disk natives and
any on-disk completion copy to equal the exact in-memory installer
observations, require every expected file to have been present, then write
the same seal atomically. `inspect` and all retirement paths are untouched
and still re-hash independently before any deletion.

Preserved invariants: identical seal bytes (proven by the benchmark digest
and by `sealing_matches_independent_reference_observation`, which compares
the written seal byte-for-byte against an independently constructed
reference); exact allowlists; reparse refusal; path vocabulary checks;
natives freshness; manifest-last publication (the dedicated F3R-B test
caught and verified the one behavioral subtlety — the completion entry is
bound from in-memory bytes even when the file is not yet on disk);
journal authority, transaction ordering, locks, recovery, manual-repair
protection, durable retirement intent and crash-safe replay all untouched.
New refusals: `sealing_refuses_unaccounted_staging_content` (extra file,
missing file, stale pre-existing seal, tampered native, mismatched on-disk
completion bytes, unknown empty directory). No hash was replaced by size or
timestamp; no cross-boundary observation was merged; no durability barrier
was removed. The eliminated re-hash only proved "unchanged between two
adjacent loops of one synchronous call in private staging" — outside the
documented threat model, and every later consumer re-verifies bytes.

## 7. Discovery: Forge manifests never sealed (pre-existing)

The real Forge 1.20.4 reinstall completed but logged
`reason=sealUnavailable` in 1.4 ms: its plan records
`libraries/com/google/code/gson/gson/2.10.1/gson-2.10.1.jar` TWICE
(byte-identical entries), and both the F3R-B baseline and this rewrite
refuse duplicate manifest paths. The F3R-B-era source Forge fixture also has
no seal — F3R-B's desktop matrix only ever reinstalled Vanilla, so this was
never observed. Behavior is unchanged and correct (the backup stays
protected as uncertain ownership); the fixture evidence is recorded here
because it bounds which trees can ever retire. A planning-side dedupe of
identical entries is recommended as its own small phase; it was not changed
here because baseline semantics are deliberately refusal-first.

## 8. Automated verification (final source)

- `cargo fmt --all -- --check`: clean. `cargo check --all-targets`: clean
  (5 warnings pre-existing on this HEAD in unrelated modules).
- Default debug suite: **964 passed, 0 failed, 30 ignored** (lib) + bin +
  **6/6 `icon_assets`**. F3R-B closed at 959; the five new tests are the
  benchmark helper (inert), the two sealing equivalence/refusal tests, the
  symlink gate helper (inert), and junction-after-eligibility.
- Release suite: **964 passed, 0 failed, 30 ignored** + **6/6 `icon_assets`
  — the six release integration tests rerun on the final source revision**
  (not substituted from earlier commits). Ignored tests are not counted.
- Focused install/recovery subset: **103 passed, 0 failed, 5 ignored**
  (F3R-B post-correction baseline: 97/5).
- Frontend: `svelte-check` 0 errors / 0 warnings; serial
  `node --test --test-concurrency=1`: **272/272 pass, 0 fail (87.8 s)**;
  production build passed.
- Native production no-bundle build: passed (4 m 56 s), not installed.
- The owner's running processes were never terminated for a build; the
  production exe runs from `%LOCALAPPDATA%`, not from the dev target.

Deviation, disclosed: the first `npm test` (parallel mode) stalled exactly
like F3R-B's documented stall; its process tree was stopped, but the kill
swept all `node.exe` images by name — most plausibly all spawned workers of
the stalled run, though unrelated node processes cannot be strictly excluded.
The serial rerun above is the counted result.

## 9. Windows-native desktop regression (§16)

Real Tauri windows driven through their visible controls only, via the
retained F3R-A driver and the debug-only `f3ra-desktop-acceptance` build
(SHA-256 `6003adb59dab446eef403d48f9c420d44db75604ed28ffc98efcdbe06a327782`),
in the new isolated root
`%TEMP%\aurora-f3ra-F3RB-B1M-2e6bfde9597b4afd834014d6f9ed8dd1`
(exact F3RA marker; fixtures copied read-only from the earlier disposable
F3R-B root; separate cache/runtimes; no accounts; two NEW synthetic storage
fixtures generated through production journal promotion with backdated
receipts). Raw evidence (driver records, per-label logs, checkpoint files,
authority hashes) stays in that root's `evidence/`.

- Initial validation: all fixtures Ready through the real app.
- **A — successful replacement**: UI reinstall of the real Vanilla fixture
  (warm cache, zero downloads). Optimized sealing wrote a 4,696-entry seal
  in 7.533 s (debug profile); transaction `5e24c5a4…` committed; the
  previous sealed tree received a `retained` receipt whose `sealSha256`
  matches the archived seal byte-for-byte.
- **B — pre-commit death**: executed at native level (deterministic
  `fail_before_commit` fault injection and interrupted-boundary journal
  tests inside the passing 964, plus the manifest-last test proving a
  seal-only tree is NotInstalled). Not repeated as a desktop kill this
  phase: the changed code completes before the B checkpoint, the restore
  path is unchanged since F3R-B's desktop proof, and C exercises the
  commit-adjacent kill on the same transaction shape.
- **C — post-commit death**: parked at `AfterCommit`, killed externally
  (journal `committed`, txn `6684157d…`). Fresh process displayed
  "Recovery completed. The committed game content was retained."; the
  committed tree's seal binds its exact installation identity; registry
  deliberately stayed `installing` (F3R-B's identical outcome).
- **D — interrupted retirement**: parked at `RetentionFileRemoved`, killed.
  Disk showed durable `deleting` receipt + `retiring-game` (one verified
  leaf removed); a second desktop process resumed through Settings and
  completed **3 retired / 2 protected**; repeat retired 0.
- **E — mixed content**: owner-note inside the protected historical backup;
  UI maintenance reported "preserved because ownership or recovery evidence
  needs inspection"; tree and sentinel byte-identical
  (`0ba3184d…415e`), reclaimed 0.
- **F — cross-process contention**: while the parked process held the
  locks, a second desktop process showed the same instance as
  "Installation busy — Installation or recovery is busy. Wait for the
  current operation to finish, then retry." with other instances Ready.
- **G — repetition/restarts**: repeated maintenance on both synthetic
  fixtures returned stable 0-retired outcomes; across fresh boots all **36
  authority/ownership/receipt documents were byte-identical**
  (set digest `697343c7…fb64`).

§17 crash checkpoints (sealing changed): retention process-death at
Intent/Renamed/FileRemoved/Empty rerun green inside both full suites
(independent child processes, fresh locks, authority unchanged), desktop D
reproduced FileRemoved with a real kill, and C covers the commit boundary.
Restart paths are idempotent per the replay tests.

## 10. Preservation, limits, verdict

All 889 baseline tracked paths remain (final re-check below); the only
tracked changes are `install/retention.rs`, `install/retention_tests.rs`,
this document, and `docs/forge-f3rb1/`. Pre-existing untracked files,
worktrees, release artifacts, the owner's instances/`.minecraft`/
authentication, production 1.4.2, and both earlier acceptance roots are
outside write scope. No push, release, publication or deployment occurred.
My disposable bench roots and probe scripts were retired after their
evidence was recorded; the desktop root is retained as raw evidence for
owner review.

Remaining limitations: the real file-symlink gate stays BLOCKED until the
owner authorizes one of the two minimal mechanisms in §2; real
directory-symlink coverage is gated with it; the parallel frontend runner
stall is a host tooling issue documented above; Forge trees remain
permanently unsealed (protected) until the duplicate-plan-entry planning fix
gets its own phase; release-profile desktop sealing was not rerun end-to-end
(the acceptance feature is debug-only) — release-level equivalence rests on
the release benchmark, the byte-identical seal digest, and the full release
suite; same-user TOCTOU remains outside the protection claim.

**Final verdict: PARTIAL** — security gate blocked on authorization, not on
implementation; performance work verified and safe; all regression suites
green on the final source. Recommended next functional phase remains owner-
reviewed F1C authenticated gameplay acceptance; recommended small follow-up:
close the §2 symlink gate in an authorized environment and consider the
Forge duplicate-entry planning dedupe.
