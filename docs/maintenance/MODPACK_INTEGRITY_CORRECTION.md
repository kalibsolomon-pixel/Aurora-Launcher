# Modpack integrity correction (recurring `pack_state_integrity_failure`)

Date: 2026-11-02 · Scope: native 2.0.0 corrective work, isolated from the
published release transaction.

## 1. Exact root cause

The owner's instance **Vulkan 11** (Modrinth `u76hn1pF` / `aRFc5ttQ`,
"1.21.11-4.1") repeatedly reported
`Needs repair. modpack: pack_state_integrity_failure` after every play
session, and the same failure blocked the Java panel.

Read-only forensics on the installed instance proved the mechanism:

- `pack-installed.json` recorded ten `keepLocal` divergences for settings
  files (`options.txt`, `config/*.toml|json|properties`) that earlier Smart
  recovery runs had reconciled.
- Exactly three paths failed revalidation:
  `config/asyncparticles/asyncparticles-mixin.properties`,
  `config/packetfixer.properties`, `config/vmp.properties`.
- Each file's current header contains `#Mon Nov 02 00:19:10 EST 2026`:
  these mods save their config through Java `Properties.store()`, which
  writes a **fresh timestamp on every save**. The mods rewrite the files on
  every game session, so byte-identical output is impossible.
- Timeline: Smart recovery applied at 00:18:30–00:19:00, the game launched
  at 00:19:09, the mods rewrote the three files at 00:19:10 → validation
  failed again. Recurrence was guaranteed by construction, not by damage.

`InstalledPack::validate_installed` byte-pinned every pack-owned path
forever, including paths the running game itself owns and rewrites
(`options.txt`, `servers.dat`, user caches, `config/**`). Smart recovery's
`keepLocal` divergence recorded one point-in-time hash, and the next game
session invalidated it. This was a false-positive integrity policy, not
corruption.

Two aggravating defects made the false positive severe:

1. **Stuck promotion.** A later game-install retry left the registry record
   `installing`. `retry_instance_install` hard-refused pack instances, and
   `complete_pack_instance` requires zero validation problems — unreachable
   while the mutable drift existed. Repair refused, Smart recovery
   re-broke every session.
2. **Overbroad Java gating.** `resolve_instance_game_plan` (the
   launch-blocking deep gate) was also the resolver behind runtime status,
   custom-Java checks, and runtime installation, so modpack byte damage
   blocked merely *viewing* Java state — the second error in the owner's
   screenshot.

## 2. Affected workflows

Modpack install validation, per-session readiness checks, Settings →
Installation Check/Smart recovery/Repair, Java status and custom-Java
checks, launch readiness, and cross-instance synchronization into pack
instances.

## 3. Reproduction

- Real: play any Modrinth pack shipping a `Properties`-based config
  (Vulkan Optimized does), then run Check → integrity failure on the
  config path; Smart recovery clears it; the next session re-fails it.
- Fixture: `instances::lifecycle::tests::
  runtime_mutable_drift_is_not_damage_but_mod_artifacts_stay_verified`
  rewrites the pack's `config/rollback-fixture.toml` and `options.txt`,
  disables a pack mod via the `.jar.disabled` twin, and finally deletes a
  mod to show precise `pack_state_missing_file` damage.

## 4. Relevant files and metadata

`core/src/pack_state.rs` (`validate_installed`, `is_runtime_mutable`),
`core/src/instances/lifecycle.rs` (validation, retry, plan gates),
`core/src/instances/recovery.rs` (Smart recovery), `core/src/application.rs`
(Java commands), `core/src/instance_sync.rs` (sync planning), plus the
instance documents `pack-installed.json` / `content-managed.json` and the
registry record state.

## 5. Aurora / Pandora comparison

Pandora Launcher (MIT, Moulberry; studied at the existing local clone)
verifies hashes **at download time** and **at update-application time**
(`should_override_file` in `crates/backend/src/backend.rs`: overwrite a
local file only when it still matches the hash the pack last applied;
user/game-modified files win). It performs **no recurring post-install
byte validation** of pack overrides at all — one-time materialization plus
update-time conflict detection.

Conclusion: mainstream ownership semantics treat pack-shipped settings as
defaults that become the game's/user's state after install. Aurora keeps
its stronger guarantees where they mean something — deterministic
artifacts (mods, resource packs, shader packs, any non-settings bytes)
stay SHA-256-verified from the trusted store — and now matches the correct
ownership model for runtime-mutable settings. No Pandora code was copied.

## 6. Implemented changes

1. `pack_state::is_runtime_mutable` defines the game write surface:
   `options.txt`, `servers.dat`, `usercache.json`, `usernamecache.json`,
   `config/**`. `validate_installed` no longer byte-pins those paths
   (present files must still be safe regular files; authored digests stay
   recorded for update planning and recovery restore). All other pack
   bytes remain hash-verified. The launcher's `.jar.disabled` mod-toggle
   twin satisfies a mod component exactly like the active file.
2. Precise errors: `PackStateError::Integrity` now carries the failing
   path (`pack_state_integrity_failure: <path>`), and a genuinely absent
   deterministic file reports `pack_state_missing_file: <path>` instead of
   a misleading IO code. Internal codes remain in logs and diagnostics.
3. `retry_instance_install` completes interrupted pack promotions:
   recover-installation replay, then the `complete_pack_instance` gate
   (full validation before Ready; precise problems on failure; the record
   stays retryable). Nothing is downloaded, wiped, or overwritten, and
   pack instances still never re-run the ordinary installer.
4. Java maintenance de-gated from content damage:
   `resolve_instance_game_plan_for_runtime` blocks only on problems that
   make the installed identity unresolvable (`platform`, `consistency`);
   runtime status, custom-Java checks, and runtime installation use it.
   Launch readiness and launch keep the strict gate.
5. Smart recovery no longer inspects runtime-mutable paths (nothing to
   reconcile: they are not validated), never records divergences for them,
   and its apply-time re-verification ignores them (the game may rewrite
   them during the apply window). Removed-mod review and deterministic
   damage blocking are unchanged.
6. Instance synchronization never plans writes onto destination paths a
   pack owns deterministically (`resourcepacks/`, `shaderpacks/`
   components); runtime-mutable settings remain syncable.

## 7. Recovery behavior

- Automatic: an interrupted pack promotion self-heals through Repair
  (retry → completion gate). Mutable-settings drift requires no recovery
  at all.
- Review-required: removed pack mods (explicit keep confirmation) and
  changed deterministic pack bytes (blockers naming the exact path).
- Unrecoverable: unreadable/unknown-schema pack state still fails closed;
  nothing is silently rewritten.

## 8. Data preservation

No user, game, or pack bytes are modified by validation, retry
completion, or recovery inspection. Retry completion writes only the
registry promotion. Sync guard and validator changes alter no files. The
owner's instance was only ever read.

## 9. Regression tests

- `pack_state::tests::runtime_mutable_vocabulary_covers_the_game_write_surface_and_errors_name_paths`
- `lifecycle::tests::runtime_mutable_drift_is_not_damage_but_mod_artifacts_stay_verified`
- `lifecycle::tests::retry_completes_an_interrupted_pack_promotion`
- `lifecycle::tests::runtime_resolution_ignores_pack_damage_while_launch_gate_refuses_it`
- `instance_sync_tests::pack_owned_destination_content_is_never_a_sync_target`
- `pack_update::tests::validating_after_commit_catches_damage_and_restores`
  (updated: deterministic tamper still fails; config rewrite passes)
- recovery tests reworked to the corrected ownership policy
  (no divergence recording for mutable paths; stale-review via metadata).

## 10. Performance impact

None: validation still hashes the same deterministic set and skips the
(bounded) mutable list; no polling, scans, caches, or new stores were
added.

## 11. Remaining limitations

- Sync source-side pack files may still be *read*; only destination
  writes are guarded (by design).
- Cross-process installation exclusion remains process-local, as before.
- Smart recovery still requires explicit confirmation for removed mods.

## 12. Local commits

See the git history of this branch for the correction commit(s).

## 13. Recommended release disposition

Native 2.0.0 is published and immutable. This correction is
**release-blocking for the next patch** (recommended `2.0.1`): the defect
reliably removes pack instances from service after ordinary play. Prepare
a corrective release candidate from this branch through the existing
signed update channel; do not advance update authority without owner
authorization.
