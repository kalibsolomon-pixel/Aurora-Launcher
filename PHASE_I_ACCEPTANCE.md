# Phase I acceptance — provider-aware Modrinth modpack installation

Phase I baseline: `5197f70` (main). Implementation: `c53f881` "Add provider-aware
Modrinth modpack preview and installation". A completion pass (interrupted by a
usage limit, then resumed) added file-level pack identity, acquisition
deduplication, pack-owned update guarding, deterministic rollback coverage, and
the live acceptance below. This record supersedes the earlier PARTIAL report in
which installation, restart, installed-content and ownership had not yet been
exercised.

Verdict: **COMPLETE** for Phase I scope. Minecraft launch acceptance is
**OWNER-SKIPPED / NOT RUN** by explicit owner direction; it is an acceptance
limitation, not a product failure, and is not counted among the failed cases.

## Live successful pack — Sodium Booster 2.0.0

Installed through the real UI and command path against a disposable managed
root (`AuroraPhaseI-Acceptance-20261001`, outside the repository):

- Modrinth identity: project `tRDjDmhJ`, version `SHYlVjNn`, Sodium Booster 2.0.0
- Minecraft 1.21.11, Fabric Loader 0.19.3 (independent Fabric instance)
- 15 components, all Modrinth-recognized (15 provider records, origin `pack`);
  0 external files; 18 overrides
- Transaction committed; registry record **Ready**; `pack-installed.json` and
  the game completion manifest present
- Full launcher restart against the same root: instance still Ready with the
  same exact pack identity; before/after disk summaries identical
  (`.zcode-diag/phase-i-modpacks/17-…-before-restart.json` and `18-…-after-restart.json`)
- Installed-state inspection: all component and override hashes valid,
  15 pack-origin provider records, **zero** injected `aurora-*.jar` — third-party
  packs are not silently converted into Aurora Client instances
- Installed content and Details show truthful pack ownership ("Installed by
  modpack"); update discovery represents pack-owned projects as blocked with the
  pack's name and version

The disposable root has since been deleted by the owner environment; the
preserved screenshots and disk summaries under `.zcode-diag/phase-i-modpacks/`
are the evidence of record. Nothing was reinstalled to recreate them.

## Rejected live packs (fail-closed evidence)

Several published packs were deliberately rejected rather than silently
repaired. These are evidence of validation strength, not failed acceptance:

- **Pulse lite 0.1.0** (`jbnIniFa`/`8lArL427`): its exact published snapshot
  embeds a `placeholder-api` JAR requiring Minecraft ≥ 26.2 while the pack pins
  1.21.11. Aurora rejects the incoherent snapshot; compatibility validation was
  not weakened to install it. Two interrupted attempts remain in the copied
  registry evidence as `installing` records with staging debris — unavailable,
  unselected, not launchable, and refused by ordinary instance retry.
- **FPS Modpack 2.9**: declares a dependency absent from the exact snapshot
  (`pack_dependency_conflict`).
- Another candidate: unsupported/unsafe override destination (path policy).
- **Sodium Extra**: exact dependency/version conflict involving Iris/Sodium.

No second live pack was installed; Sodium Booster remains the single successful
live pack (documented limitation — fixture coverage below carries the
architectural cases).

## Case S — provider file identity (final semantics)

- Duplicate destination: rejected by the parser (`mrpack::PathCollision`).
- Repeated reference to the same exact provider artifact: one verified
  acquisition. `acquire_provider_plans` groups identical expected digests, so
  the same bytes are fetched once and reused for every destination
  (`redundant_exact_pack_file_acquisition_reuses_verified_object`).
- Distinct files from the same Modrinth project: preserved as distinct provider
  records. `file_id` is the published file SHA-512 (not the filename), the
  content state accepts same-project pack records with distinct file identities
  and rejects one identity carrying conflicting bytes, and ownership matching in
  `pack_state` uses the full identity plus version, SHA-512 and SHA-256
  (`pack_records_keep_distinct_files_from_one_project`,
  `pack_installs_distinct_files_from_one_project`).
- Multi-artifact concurrency is unchanged and still exercised with three
  genuinely distinct artifacts (one request each).

## Pack-owned update protection

- Discovery (`check_updates`): pack-owned projects appear **blocked** with
  "Required by \<pack\> \<version\>. Pack component updates require a future
  reconciliation workflow." — one row per project, zero network requests
  (`pack_owned_files_share_one_blocked_project_row_without_network`).
- Update preview (`resolve_updates_preview`): rejects a pack-owned target with
  `pack_component_owned` before any provider discovery.
- UI: blocked rows stay visible with their reason; an all-blocked report never
  claims "Everything is up to date"; Update All only includes ready entries.
- Native apply guard remains defense-in-depth: pack-owned records cannot be
  mutated by the ordinary provider transaction.

## Transaction and rollback (deterministic)

- Early game-install failure: an explicit retryable `installing` record
  remains — unavailable, unselected, not launchable; ordinary instance retry
  refuses it deterministically; unrelated owner state is preserved
  (`failed_pack_game_install_stays_unavailable_and_ordinary_retry_refuses_it`).
- Post-game component failure (hash-valid artifact without valid fabric
  metadata): the whole transaction rolls back — registry record retired, the
  validated game tree and hash-proven transaction files removed, override
  materialization rolled back, no half-ready instance, unrelated state intact,
  and the shared verified cache survives so the corrected retry reuses every
  game artifact without re-downloading
  (`pack_component_failure_after_game_install_rolls_back_and_keeps_the_cache`).
- Acquisition failure before the instance exists: nothing registered, created
  or selected; the cached pack archive then serves a successful retry
  (`pack_acquisition_failure_leaves_no_instance_and_the_cache_serves_a_retry`).
- Digest failures: bad pack SHA-1/SHA-512 and unsafe fallback hosts/paths are
  rejected with no instance file created
  (`unresolved_fallback_keeps_external_identity_and_rejects_bad_bytes`).
- A rollback that cannot prove ownership of every byte fails with
  `pack_rollback_failed` and leaves the incomplete instance unavailable for
  inspection instead of deleting uncertain files.

## Stale preview

Install re-resolves provider authority and compares the preview fingerprint
before creating any instance; a changed pack version is rejected with
`pack_preview_stale` and no registry file exists
(`preview_recovers_no_fake_provider_identity_for_an_unresolved_file`).

## Unresolved/external component (FIXTURE)

Sodium Booster had no external files, so the fallback path is covered by the
deterministic fixture: strong SHA-512 acquisition plus the pack's SHA-1
verification, approved-host-only fallback URLs, verified cache path, no
fabricated Modrinth identity, explicit pack ownership in `pack-installed.json`,
and no provider record for external bytes.

## Native UI final check

The isolated Phase I root no longer exists, so the pack-instance screens were
not re-shot; the preserved numbered evidence (`01`–`20` under
`.zcode-diag/phase-i-modpacks/`) remains the record, including
`05-install-progress.png`, `06-installed-instance.png`, `07-installed-content.png`,
`09-pack-ownership.png`, `10-restart-persistence.png` and
`19-pack-update-guard.png`. A final-tree smoke check ran the current build
against a fresh disposable root: boot, Home, Instances (empty state, creation
form, Modpacks section) and the Modpacks browse panel with live Modrinth results
all render (`21-final-tree-smoke-boot.png`, `21-final-tree-instances-page.png`,
`22-final-tree-modpacks-browse.png`). No pack was reinstalled for screenshots.

## Performance and network posture

No polling, watchers, per-pack timers, startup pack requests, or background
modpack update checks. Component identity lookup is batched (SHA-512 batches),
identical expected digests share one acquisition, verified cache objects are
reused, and a launcher restart does not re-download a completed pack to
reconstruct identity.

## Final verification (final working tree)

- `npm run check`: 0 errors, 0 warnings
- `npm test`: 178/178 passed
- `npm run build`: passed
- `cargo fmt --all -- --check`: passed
- `cargo check --all-targets`: passed
- `cargo test`: 708 unit + 6 integration passed, 26 intentionally ignored
- `npm run tauri build`: MSI and NSIS bundles produced

## Acceptance limitations

- Minecraft launch acceptance: **OWNER-SKIPPED / NOT RUN** (owner direction).
- Single successful live pack (Sodium Booster 2.0.0); no second live pack.
- The disposable acceptance data root was deleted after the session; live
  restart/persistence evidence is the preserved disk summaries and screenshots,
  not a currently-mounted installation.
- The all-blocked Updates presentation is proven by the deterministic discovery
  test and the quiet-state condition; no live screenshot of that exact state
  exists (`19-pack-update-guard.png` shows the mixed ready+blocked state).

## Deferred to Phase J

Modpack update discovery, Update Modpack, pack-version reconciliation, user
divergence, cross-version override conflicts, pack rollback/history, and
automatic old-pack cleanup.
