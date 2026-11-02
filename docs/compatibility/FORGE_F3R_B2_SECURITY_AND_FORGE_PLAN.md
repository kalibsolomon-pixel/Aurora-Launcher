# F3R-B2 — Windows symlink security closure and Forge duplicate-entry correction

> F3R-B2a correction (2026-10-14): the historical containment/cleanup and
> enforced-unelevated claims below were too strong. The old wrapper deleted
> caller-supplied roots, and neither phase checked token elevation. Installer
> identity matching also omitted extensions. See
> [docs/compatibility/FORGE_F3R_B2A_REVIEW_CORRECTIONS.md](FORGE_F3R_B2A_REVIEW_CORRECTIONS.md)
> for the corrected implementation, exact evidence and remaining blocked gate.
> Historical outcomes below are preserved; they do not close the real-link gate.


Work date: 2026-10-13. Starting branch `codex/client-3-production-authority`,
HEAD `76ba054e7a28b576ffaa2789764ce160dedb49d3` (the reported F3R-B1 HEAD).
**Verdict: PARTIAL — the Forge duplicate-entry defect is corrected and fully
verified end-to-end, every final-source regression suite passes, but the real
Windows file-symlink gate remains BLOCKED: this host's standard user still
cannot create a file symbolic link and this phase grants no elevation.**
No Windows security setting changed; no push, publication, deployment, or
production replacement occurred.

## 1. Symlink driver review and privilege boundary (PART A)

The F3R-B1 driver and helper were re-reviewed against the phase checklist:
all writes stay inside a fresh marked `aurora-f3rb1-symlink-*` direct child
of `%TEMP%`; the sentinel target is created inside that root; no owner file
is ever referenced; no registry or security setting is touched; no implicit
elevation exists; creation failure fails LOUDLY as BLOCKED and is never
counted as a pass; sentinel SHA-256 and transaction-authority bytes are
checked before and after; refusal and idempotence are asserted twice.

The environment was re-probed and is unchanged: standard user
`SPXCTER\kalib`, not elevated, not in Administrators, no
`SeCreateSymbolicLinkPrivilege`, Developer Mode key absent, and live
creation still fails with "Administrator privilege required for this
operation." **The gate is recorded BLOCKED; no elevation was attempted.**

The gate now implements the phase's preferred execution model directly:

- `AURORA_F3RB1_SYMLINK_PHASE=prepare` builds the fixture and creates the
  two real links (live + dangling). This is the only privileged step and is
  suitable for an owner-run elevated helper; it performs no retention
  decision and leaves the fixture in place with a recorded
  `gate-manifest.json` (slot id, sentinel SHA-256, authority SHA-256).
- `AURORA_F3RB1_SYMLINK_PHASE=verify` re-validates the prepared fixture and
  executes the real retention refusal assertions in a fresh, deliberately
  UNELEVATED process — so eventual acceptance proves the ordinary retention
  code, not an elevated copy of it. Unset, the helper runs both phases in
  one process (the Developer-Mode path).

Exact owner procedure (one-time authorization, no settings change):

1. Elevated PowerShell (only privileged step):
   `powershell -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1 -Phase prepare`
2. Ordinary PowerShell, same machine:
   `powershell -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1 -Phase verify -Root <printed-root>`

A single-shot elevated run is also supported and honestly labeled: in that
mode the retention assertions run elevated too, which is recorded as the
limitation it is. The prepare phase was executed during this phase and
failed closed with the BLOCKED panic, as designed.

## 2. Forge duplicate-entry root cause (PART B §7)

Reproduced from the checked-in official F1C fixtures and confirmed on the
live desktop tree:

- Forge's installer profile (`install_profile.json`) carries the installer
  TOOL's own library set. For 1.20.4-49.2.0 exactly one entry overlaps
  Mojang's 1.20.4 client set — `com.google.code.gson:gson:2.10.1` — with
  identical published SHA-1 (`b3add478…`), identical size (283,367), and an
  identical repository path, but a different download URL
  (Forge's maven versus `libraries.minecraft.net`). For 1.20.1-47.4.10 the
  same single overlap exists on `net.sf.jopt-simple:jopt-simple:5.0.4`,
  also byte-identical — so 1.20.1 installations had the identical latent
  defect.
- `version.json` contains neither entry, so `ForgePlan`'s own
  launch-vs-installer dedup could not see the overlap, and the shared
  `merge_library_sets` boundary only merges Mojang's set with the loader's
  LAUNCH libraries. The executor then materialized every installer-tool
  library beside the composed set, planning the same destination twice.
  Duplicate manifest entries with identical verified digests and sizes made
  `seal_staging` refuse the whole tree — F3R-B1's documented discovery.
- A new official fixture trio for Forge 1.21.11-61.2.0 (recommended pin,
  retrieved 2026-10-13, installer SHA-1-verified against its sidecar,
  Mojang document SHA-1-verified against piston-meta) shows the modern
  installer's tool set has NO overlap with Mojang's — the defect is
  data-dependent, not structural, and 1.21.11 never triggers it.
- Vanilla, Fabric, and Aurora composition have no installer-tool library
  set at all; NeoForge has the same executor pattern and therefore the same
  theoretical exposure, fixed here alongside Forge.

## 3. Duplicate semantics and the correction (PART B §8-9)

The shared composition boundary gained `installer_library_satisfied_by`,
encoding the phase's three-case rule at the only layer that sees both
sources:

- **Identical duplicate** — same (group, artifact, classifier) at the same
  version at the same managed destination, with published SHA-1s that
  agree: the composed entry already satisfies the requirement; the
  installer-tool copy is dropped. URL and size are acquisition detail, not
  identity — the official documents deliberately publish the same bytes
  from different repositories.
- **Conflicting duplicate** — same identity and version whose published
  SHA-1s disagree: a corrupt or forked publication claiming one
  destination; a hard `LibraryConflict`, never a silent choice.
- **Distinct artifacts** — same identity at a different version (or just a
  shared basename): different Maven destinations, both kept.

Both `compose_forge_game_plan` and `compose_neoforge_game_plan` apply the
classifier after the existing merge. This is a generic composition rule,
not a Forge special case, because both loader families share the
executor's installer-library materialization. Deterministic ordering,
verified identities, hashes, sizes, platform rules, processor inputs (the
overlap's destination remains planned exactly once, from the Mojang side),
classpath semantics (derived from the untouched merged set), manifests,
recovery, and the ownership sealer itself are all preserved — the sealer
was NOT weakened, and no loader composition was rewritten. The previously
existing dedup layers (within-document, launch-vs-installer, merge) are
unchanged.

## 4. Automated coverage (PART B §10)

- `game::plan`: `installer_libraries_are_classified_by_verified_identity`
  proves identical-satisfied, disagreeing-digest conflict, distinct-version
  kept, and unrelated kept.
- `forge::compatibility_tests`:
  `installer_tool_overlaps_compose_to_one_destination_per_artifact` runs
  against all three official fixture generations and asserts, per version:
  the overlapping installer entry is dropped; the composed set carries the
  Mojang-published copy exactly once; NO duplicate destination exists
  across both materialized sets (the original defect); merged order and
  provenance anchors hold; every non-overlapping installer entry survives
  in order; processor steps and the overlap's processor-visible
  destination are preserved; for 1.21.11 everything is retained.
- Forge 1.20.4, 1.20.1, and the new 1.21.11 fixtures; Vanilla, Fabric,
  NeoForge, and Aurora-on-Fabric planning/composition regressions run in
  the full suites (§6); ownership sealing after deduplication is proven by
  the retained F3R-B1 test set plus the desktop run below; transaction
  recovery and retention eligibility suites are unchanged and green.
- The failing-shapes that would matter — same destination with different
  SHA or different size metadata — are covered by the classifier's
  conflict case (digest disagreement) and by its identity rule (size is
  not identity; a same-digest entry cannot differ in bytes).

## 5. Native Windows acceptance (PART C §11-13)

Real Tauri desktop, fresh diagnostic build of the corrected source
(SHA-256 `415a902779af4b7ebfae79f2f0ced3a3c7cc99b0da60660650dc62f2ac88cbfe`,
debug-only acceptance feature, isolated root reused from F3R-B1), driving
the real UI only:

- **Forge 1.20.4 reinstall #1**: planning succeeded with the overlap
  deduplicated; 139.4 s warm install (3,771 cache hits, 143 metadata
  downloads), processors completed, transaction `1bf4333f…` committed, deep
  validation passed, and **ownership sealing succeeded — 3,914-entry
  manifest with ZERO duplicate paths, exactly one gson entry, seal binding
  the installation identity**. The first Forge tree ever sealed by Aurora.
  The pre-fix tree it replaced rests as an unsealed, protected backup
  (no receipt, by design).
- **Forge 1.20.4 reinstall #2**: transaction `ff61e900…` committed; its
  previous tree — sealed by the corrected code — received a `retained`
  retention receipt whose `sealSha256` matches the archived seal
  byte-for-byte: the first verified Forge historical backup.
- **Explicit maintenance**: engine outcome `eligible=0 protected=3
  retired=0 reclaimed_bytes=0` — the current slot's sealed backup, and the
  two unsealed legacy trees, all correctly protected; nothing reclaimed;
  no owner-created file touched.
- **Cross-loader (§12)**: the correction lives in the loader-neutral
  composition boundary; Vanilla/Fabric/Aurora plans take no
  installer-library path at all, and NeoForge's composer applies the
  identical rule. All loader planning/composition/compatibility suites
  pass in both profiles. Deep validation is not gameplay acceptance.
- **Crash/recovery scope (§13)**: the change is pure plan normalization —
  journal, recovery, transaction ordering, executor, retention, and seal
  code are untouched (the F3R-B1 sealing optimization is intact). The
  committed transaction shape differs only by one fewer materialized
  library copy. All interruption machinery is therefore covered by the
  unchanged checkpoint/fault-injection/process-death suites rerun green
  in §6; the desktop kill matrix was deliberately not repeated.

UI-automation note: some a11y presses on this WebView build silently
no-op; successful dispatches used fresh element bounds, menu reset, or
keyboard activation on a focused button, and every reported action was
verified against on-disk state (journal/transaction ids above), not
against the button state alone.

## 6. Final-source verification (§14)

- `cargo fmt --all -- --check`: clean. `cargo check --all-targets`: clean.
- Debug suite: **966 passed, 0 failed, 30 ignored** + 6/6 `icon_assets`.
- Release suite: **966 passed, 0 failed, 30 ignored** + **6/6
  `icon_assets` — the six release integration tests rerun on final
  source**. Ignored tests are listed as ignored, never counted.
- Focused new tests: `game::plan` 5/5, `forge::compatibility_tests` 7/7.
- Frontend: `svelte-check` 0 errors / 0 warnings; serial node tests
  **272/272** (149.2 s); production build passed.
- Release tooling (unaffected by this change, rerun anyway):
  **115/115 pass**.
- Native production no-bundle build: passed. The owner's processes were
  never terminated; the production launcher was not running during this
  phase and was not started.

## 7. Preservation, commits, limitations

All 892 baseline tracked paths remain plus the three new fixture files;
pre-existing untracked files, all worktrees (including the untouched
incomplete `codex/f3rb1-acceptance` branch at `4f2ad56`), the owner's
Minecraft data, authentication, and production 1.4.2 are outside write
scope. Commits (this phase): Forge planning correction with tests and the
official 1.21.11 fixtures; the two-phase symlink-gate driver update;
this document with evidence. No push.

Remaining limitations: the real file-symlink gate stays BLOCKED until the
owner runs the two-step procedure in §1 (or enables Developer Mode);
already-installed pre-fix Forge trees remain permanently unsealed and
protected (only future replacements seal); the WebView click flakiness
documented above; same-user TOCTOU remains outside the protection claim.

**Recommended next phase:** the owner executes the §1 gate procedure to
close the last F3R-B PARTIAL item, then F1C authenticated gameplay
acceptance per the owner roadmap.
