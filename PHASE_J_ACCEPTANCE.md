# Phase J acceptance — modpack update & reconciliation

Phase J baseline: `565ca8a` (main). Implementation commits `46e0ee2`
(reconciliation domain and transaction), `a01cf57` (Tauri commands and DTOs),
`592dc35` (Update Modpack in the instance Overview), `c2d077f` (semantics
documentation). A first acceptance pass exercised the feature live (real
Sodium Booster 2.1.3 → 2.2 update through the real UI, plus a controlled
live divergence test) but returned **PARTIAL**: AF and BP cited adjacent
coverage instead of dedicated assertions, and five requested visual evidence
states were missing. This completion pass closed exactly those gaps and is
the authoritative record.

Verdict: **COMPLETE** for Phase J scope. Minecraft launch acceptance remains
**NOT REQUIRED / NOT RUN** by owner direction.

## Completion pass scope

Limited to: a dedicated AF test, a dedicated BP test, the five missing
screenshots through the real UI, and this document. No Phase J architecture
was reopened, no production behavior changed, no diagnostic wiring remains in
the committed tree.

## AF — unsafe fallback URL rejected (dedicated)

`pack_update_rejects_unsafe_external_fallback`
(`src-tauri/src/pack_update.rs`): the newest same-project candidate is
discovered normally (same project, published after the anchor), and its
index is honest in every respect except that the only download source for
its external file is an unapproved HTTPS host (`https://unapproved.example.com`).
The Phase I approved-host policy refuses the candidate archive at the parser
boundary:

- reconciliation/apply refuses the candidate with `pack_invalid_download`
  before any acquisition of the candidate's files is attempted;
- no instance mutation occurs — old exact pack identity (`VERS0001` anchor
  equivalent) stands and revalidates, the registry record stays `ready`,
  pack-owned and local files keep their bytes, the untrusted file never
  lands, and no update receipt exists.

## BP — unsupported loader transition blocked (dedicated)

`unsupported_loader_transition_is_blocked_before_apply`
(`src-tauri/src/pack_update.rs`): an installed supported (fabric) pack whose
newest same-project version (published after the current one, same project)
requires the quilt loader is blocked at every entry point:

- discovery reports `blocked` with the loader named as the reason and no
  candidate offered (old loader: Fabric `0.19.5`; candidate loader: quilt);
  the preview (`build_plan`) refuses with `pack_update_unavailable` — no
  reconciliation is ever built — and the apply path is refused through the
  same gate even with a forged fingerprint;
  the candidate's archive is deliberately absent from the fixture server, so
  any attempted acquisition would fail — proving the gate fires first;
- no mutation occurs: old exact pack identity stands and revalidates, the
  registry stays `ready` with the old identity, no target game/loader tree
  is activated, no pack state is rewritten, local files are unchanged, and
  no receipt exists.

Fail-closed behavior only; no unsupported-loader support was added.

## Deterministic acceptance (A–CK)

Every required matrix cell cites a dedicated passing test in the Phase J
suite (`src-tauri/src/pack_update.rs`); no required cell relies on
adjacent-only coverage. AF and BP now cite their dedicated tests above.

| Cells | Test |
| --- | --- |
| A, B, C, D | `discovery_reports_newest_same_project_and_truthful_no_update` (newest same-project candidate; truthful UpToDate; never a downgrade) |
| E | `unsupported_loader_candidate_is_blocked` (discovery-level reason) |
| **BP** | `unsupported_loader_transition_is_blocked_before_apply` (dedicated; preview and apply refuse before mutation) |
| G | `provider_unavailable_is_a_safe_failure` |
| H, I | `plan_pins_exact_old_and_new_identities` |
| M, N, O, Q, U, V, W, X, AB | `canonical_classification_matrix` (component classification, moves by file identity, same-project distinct files, external identity, unrelated user files invisible) |
| AH, AJ, AL, AN, AP | `override_reconciliation_matrix` |
| P, R, AI, AK, AM, AO, CA | `local_modification_conflict_matrix` (conflict kinds; unresolved conflicts refuse a plan) |
| S, T | `missing_old_files_are_disclosed_not_invented` |
| AW | `preview_is_read_only` |
| BB, BQ, BR, BS, BT, BU, BV, BW, BX | `successful_update_commits_the_exact_new_snapshot` (atomic commit, exact new identity, receipt removed, provider inventory, filesystem truth, no stale ownership or duplicates) |
| AQ, AR, AS, AT, AU, AV | `keep_local_and_adopt_new_resolutions_record_divergence_truthfully` and `kept_divergence_carries_forward_until_the_pack_authors_bytes_again` (divergence records, restart survival, carried forward until re-authored) |
| AX, AY, BA | `stale_plans_are_refused_safe` (fingerprint covers registry/pack/managed/local drift) |
| AZ | `forged_inputs_cannot_authorize_mutation` (frontend cannot forge a plan) |
| BC, BH, BI, BJ | `acquisition_failure_rolls_back_completely` |
| BD, BK | `digest_failure_rolls_back_and_keeps_cache` (substituted bytes refused; verified cache survives a retry) |
| BE | `activation_failure_rolls_back` |
| BE, BF | `validating_after_commit_catches_damage_and_restores` |
| BM | `same_game_versions_skip_transition` |
| BN, BO | `supported_game_transition_installs_and_rolls_back` (verified pipeline; game-tree rollback) |
| F | `unsupported_minecraft_transition_is_blocked_at_preview` |
| J, K, L | `malformed_candidate_pack_is_rejected` (parser boundary before any reconciliation) |
| Y, Z, AA | `shared_ownership_retires_only_the_packs_relationship` |
| **AF** | `pack_update_rejects_unsafe_external_fallback` (dedicated; approved-host policy refuses the candidate before acquisition) |
| (classification companion) | `unsafe_external_sources_are_rejected_before_mutation` (an unchanged external component stays external and is preserved) |
| BY | `ordinary_updates_still_block_pack_owned_content` |
| CC | `unprovable_rollback_leaves_the_instance_unavailable` |
| (crash recovery) | `interrupted_update_with_intact_old_state_recovers`, `interrupted_update_with_committed_new_state_finalizes` |
| CD, CE | `ordinary_mod_updates_continue_to_work` |
| CI | `initial_pack_install_still_works_alongside_updates` |
| (live, executed in the first pass) | `live_modpack_update_acceptance`, `live_modpack_update_local_modification_acceptance` (ignored-by-flag live tests; results below) |

## Live acceptance (first pass; not repeated)

A real Modrinth update of Sodium Booster (`tRDjDmhJ`), 2.1.3 (`Vl3xy4P9`) →
2.2 (`w0mkw17z`), Minecraft 26.2, Fabric Loader 0.19.3, through the real UI
and command path against a disposable managed root: old pack installed via
the normal Phase I flow, ready state, restart-shaped disk reload, same-project
update discovery, exact 2.2 candidate with changelog, reconciliation preview
(36 rows: 3 added, 4 updated, 2 removed, 27 preserved), real content changes,
atomic update, exact new identity, UpToDate afterwards, restart persistence.
A controlled live local modification of a pack-owned config override
classified `changedModified`, was resolved Keep local, and survived reload as
exactly one recorded divergence with the pack base at 2.2. Nothing in this
pass reinstalled or replayed that flow.

## Visual evidence

All thirteen requested slots exist under `.zcode-diag/phase-j-modpack-updates/`.
The ten live-capture states from the first pass are preserved byte-for-byte;
the five states added by this pass were rendered by the real debug Aurora UI
(vite + Tauri dev build) driven end-to-end through the real command path
(check → preview → resolve → apply) against disposable diagnostic roots
(`AURORA_DIAGNOSTIC_DATA_ROOT`), with live Modrinth discovery and
reconciliation of honestly-derived local state, and are labeled
FIXTURE-DERIVED REAL UI accordingly.

| Slot | State | Kind |
| --- | --- | --- |
| 00-boot.png | Launcher boot on the disposable root | LIVE (first pass) |
| 00-pack-install-progress.png | Phase I pack install progress | LIVE (first pass) |
| 01-old-pack-installed.png | Sodium Booster 2.1.3 installed | LIVE (first pass) |
| 02-pack-update-available.png | Update available 2.1.3 → 2.2 | LIVE (first pass) |
| 03-reconciliation-preview.png | Reconciliation preview | LIVE (first pass) |
| 04-added-updated-removed-summary.png | Change summary counts | LIVE (first pass) |
| 05-conflict-detected.png | Reconciliation preview with one real production conflict row (`config/sodium-options.json` changed by the pack and locally), "1 path needs your choice", Keep my file / Use new pack version visible, action disabled until resolved | FIXTURE-DERIVED REAL UI |
| 06-conflict-resolution.png | Same state with Keep my file selected; action enabled as Update Modpack | FIXTURE-DERIVED REAL UI |
| 07-update-progress.png | Real phase-truthful progress during apply — "Verifying…" with the Updating… action and the selected resolution visible; phases and downloads are the real event path (a deterministic diagnostic transaction: the fixture root deliberately carries no game tree, so the apply truthfully ends in validated rollback) | FIXTURE-DERIVED REAL UI |
| 08-update-complete.png | Update complete | LIVE (first pass) |
| 09-updated-pack-details.png | Updated pack details at 2.2 | LIVE (first pass) |
| 10-updated-installed-content.png | Updated installed content | LIVE (first pass) |
| 11-restart-persistence.png | Restart persistence | LIVE (first pass) |
| 12-no-update-state.png | Pristine 2.2 instance: "2.2 is the newest published version of this pack.", pack identity visible, no contradictory update action | FIXTURE-DERIVED REAL UI |
| 13-error-or-stale-plan-state.png | Stale plan refused safely: a pack-owned file changed after review; apply re-derived, refused before mutation, and the UI asks for an explicit choice/review — no corruption claim, no path or secret beyond the pack-relative file name | FIXTURE-DERIVED REAL UI |

### Fixture method (disclosed)

The disposable roots were built once by a temporary diagnostic harness
(removed before commit; nothing evidence-specific remains in the committed
tree) that resolved the exact live published pack snapshot through the real
resolver and writers: verified archive acquisition (SHA-512 against the
published digest), real component bytes from the Modrinth CDN, real
`pack-installed.json`/content-state documents, a ready registry record, and a
deterministic local modification of one pack-owned override for the conflict
states. The fixture roots deliberately carry no game tree: the launcher
discloses that honestly wherever validation runs, and the apply transaction
on such a root deterministically exercises its real phases (checking,
downloading, verifying, applying) before failing post-commit validation and
rolling back — which the disk state after the capture confirms (old identity,
ready record, no receipt, no staging debris).

## Visual review

Clipping/overflow: none — every captured state's controls are fully inside
the window (verified per screenshot). Conflict controls name the exact
pack-relative path and offer exactly the two production choices, with the
primary action disabled until resolution. Progress wording is the production
phase vocabulary with no fabricated percentage. The no-update state names the
installed identity and offers no contradictory action. The stale/error state
is specific and non-alarming. Phase F visual baseline unchanged (no UI code
was modified in this pass).

## Final verification (final working tree)

- `npm run check`: 0 errors, 0 warnings
- `npm test`: 178/178 passed
- `npm run build`: passed
- `cargo fmt --all -- --check`: passed
- `cargo check --all-targets`: passed
- `cargo test`: 740 unit + 6 integration passed, 28 intentionally ignored
  (740 = the previous 738 plus the two new dedicated tests)
- `npm run tauri build`: MSI and NSIS bundles produced

Regression scope intact in the suite: Phase G provenance/recovery, Phase H
ordinary updates, Phase I pack install and the pack-owned ordinary-update
guard, Phase J discovery/reconciliation/divergence/transaction/rollback/
stale-plan/game-loader transition, accounts/skins, resource packs, shaders,
and server enrichment. Minecraft launch was not run and was not required.

## Diagnostic safety

- Debug dev executable only; the release executable was never used for
  evidence.
- Disposable roots under `%TEMP%\AuroraPhaseJ-Evidence-20261001\root-{a,b,c}`;
  the effective managed root was verified per run from the launcher's own
  backend log ("backend ready; managed data root: …") and by the UI showing
  exactly the fixture instance, before any mutation was allowed.
- The owner's production launcher state (`%LOCALAPPDATA%\com.aurora.launcher`
  and the phase-f review root) was snapshotted before and after: zero changes
  to `launcher/`, `instances/`, `runtimes/`, or the verified cache; the only
  differences anywhere are WebView2 cache churn.
- The temporary fixture-builder harness was removed before commit; no
  developer menus, fake-state buttons, artificial delays, polling, timers, or
  watchers were added.

## Repository safety

No `git clean`, no force push, no history rewrite, no push, no merge, no tag,
no release, no version bump. Local `main` remains at `565ca8a`;
`origin/main` remains at `20d7d5c`; diagnostics and all prior evidence files
are preserved. Launcher 1.3.1 and Aurora Client 2.1.5 are untouched.

## Acceptance limitations

- Minecraft launch acceptance: **NOT REQUIRED / NOT RUN** (owner direction).
- The five completion-pass screenshots are fixture-derived (as permitted):
  the live conflict behavior itself was already accepted natively in the
  first pass; the fixture instances carry no game tree, which the launcher
  discloses wherever validation runs.
- Update discovery offers only the newest published version; optional-file
  selection carries by path; historical user-facing rollback remains
  machinery-only; the irrecoverable crash window between game-tree
  replacement and document commit still leaves the instance unavailable for
  inspection (all as documented in `PHASE_J_MODPACK_UPDATES.md`).

## Deferred beyond Phase J

Unchanged from the established list: full user-facing pack history/rollback
browser; non-Windows Recent Server SRV; loader-specific shader activation;
historical content rollback; directory-wide cache eviction; NeoForge/Quilt/
Forge support; cross-device sync; renderer compatibility advisory; Aurora
Client remote update manifest; storage dedup/reflink; generic modpack
providers; CurseForge/Prism import; automatic background pack updates.
