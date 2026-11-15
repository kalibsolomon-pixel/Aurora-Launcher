# Editing Minecraft servers in Aurora

Home → Servers → **Manage** opens the selected instance's complete server list
in Minecraft order. Add and Edit expose name, address and the server resource
pack preference (Prompt, Enabled, Disabled). Delete requires a separate explicit
confirmation; Up and Down persist one move at a time. Each operation returns the
new revision and list. Cancel and Escape do not write. Exact name/address
duplicates show a warning and remain allowed. Existing Home Play, favorites and
status controls continue to use their original APIs and compatibility gate.
After a saved edit, Home shows the Saved tab so the changed list is visible.

The GPUI dialog lives in `native-gpui/src/server_editor.rs`; Home integration is
in `home_widgets.rs`. Typed commands in `native/src/api.rs` call the application
boundary in `core/src/application.rs`. All validation and filesystem mutation
are in `core/src/saved_servers/editor.rs`.

## NBT and validation

- Uncompressed named compound root, `servers` list of compounds. A missing
  file starts with an empty root name and compound list. Existing root names,
  unknown root tags, entry icons, hidden flags and unknown entry tags retain
  their original bytes. Only `name`, `ip` and `acceptTextures` are replaced on
  Edit. Untouched entries remain byte-identical, including when moved.
- Names use Java modified UTF-8, accept Unicode and require 1–128 characters
  with no controls or all-whitespace value. Addresses use the existing canonical
  server-target validator, with an additional bracket-tail check for IPv6.
  Original address spelling is stored, while favorites and Play use the
  canonical destination. There is no DNS or status request during editing.
- Prompt omits `acceptTextures`; Enabled writes TAG_Byte 1 and Disabled writes
  TAG_Byte 0. Bounds are 1 MiB and 1,024 entries, plus the shared NBT reader's
  depth/node/list limits. Malformed, compressed, oversized, redirected and
  unsupported documents are refused without rewriting them.

## Safety and identity

Writes acquire the Play preparation reservation, stable per-instance parent
operation lock, legacy in-instance operation lock and
`instance_content::with_instance_lock`. Registration and Ready state are
rechecked while the parent lock is held, before any instance-directory creation.
The stopped-process snapshot lock is held through commit. This excludes Aurora
launch, installation, deletion, content changes and sync writers.

On Windows the writer also opens an existing `logs/latest.log` with sharing
disabled and retains that handle through commit. Sharing/access failures refuse
the edit with the stopped/busy-instance message, including a game launched by
another launcher. Missing logs/files are allowed. No Java process is launched,
stopped, attached to or modified by the editor.

The loaded revision is the SHA-256 of the exact bytes (`missing` for an absent
file), rechecked again after preflight and backup. The previous bytes are
atomically saved beside the list as `servers.dat_old`. Commit uses
`install::journal::atomic_write`; neither the UI nor its row indexes can supply
paths. A changed file requires an explicit Reload list before retrying.

Favorites are keyed by instance + canonical address, not name. Address edits
move the favorite to the new key and name edits refresh its label. Historical
visit addresses/times are retained as evidence. If another saved entry still
uses the old address, both addresses retain the favorite. Existing destination
favorites are combined. A favorite-persistence failure rolls the list and sync
suppression file back; exceeding the 100-favorite limit is tested. Home reloads
its saved rows and enrichment after acknowledged writes, so old status and
quick-play identifiers do not become the new address's authority.

## Interaction with instance sync

The legacy pool's Servers category is an **additions-only union**, preserving
destination order. Without coordination, a peer's unchanged old entry would
immediately reappear after a local Delete or an Edit of both name and address.

The editor atomically records hashes of removed exact `(name, address)` identities
in the instance-local, schema-versioned `.aurora-server-edits.json`. Before every
Servers merge, the shared manual/automatic sync plan filters those identities
from incoming additions. The file is durable across worker/launcher restarts,
bounded to 4,096 identities and fails closed if damaged or unknown-version.
It is written before the list and restored on a normal commit failure. It is
launcher metadata, not part of the Minecraft NBT or a synced content category.

This is local suppression, not propagation of deletions or an overwrite policy:
peers retain their own original lists. Reorder keeps each destination's order.
Unrelated incoming additions still merge. A player can explicitly Add a removed
identity again. Whole-list conflict replacement remains unsupported; the sync
message now points to Home's editor or Minecraft. The integration test runs both
manual sync and restarted automatic workers after local edits/deletions.

All `servers.dat` consumers were searched. Besides discovery/NBT/sync, mrpack
import permits the file as a pack override, pack-state validation already treats
it as runtime-mutable user/game content, and release/layout acceptance code
contains fixtures. No additional automatic writer was found that requires a
new editing hook. Pack update policy remains the existing reviewed flow.

## Verification and showcase

Core tests cover preservation, all four edits, missing-file creation, resource
pack encoding, Unicode, malformed/bounded inputs, stale revisions, starting and
running snapshots, preparation/content/lifecycle contention, Windows log sharing,
favorite migration/duplicates/rollback, deleted-instance refusal and sync-worker
restart. GPUI state tests cover loading/row bounds, confirmation, cancellation,
busy guards, frozen instance identity, stale replies, errors, duplicate warnings
and acknowledged revisions. Tests use disposable fixtures only.

`cargo run --manifest-path core/Cargo.toml --example seed_server_editor -- <root>`
creates a fully offline showcase registry/list. It requires a root containing an
`AURORA_DISPOSABLE_DIAGNOSTIC` marker starting with `Disposable`, and refuses an
existing launcher or instances directory. It does not install or launch a game.
The GPUI `--showcase <root>` mode selects the secondary portrait display and
suppresses background network work. Never use production data for captures.

The editor reuses Aurora's Dialog, buttons, inputs, theme and spacing. Repeated
actions have row-specific accessible names; resource-pack choices expose selected
state. The list supports Tab, arrows, Page Up/Down, Home and End with a visible
focus ring. Dialogs use existing focus/escape behavior and explicit destructive
submission. No decorative surfaces, animations or renderer dependencies were
added. Performance and full screen-reader behavior require separate measurement.

Acceptance on Windows (2026-11-15): core suite **1,228 passed, 34 ignored**;
GPUI binary suite **394 passed** (including six new dialog-state tests). The
feature adds 15 core tests. Touched-file rustfmt and `git diff --check` pass.
Debug and release builds use `C:/Dev/aurora-gpui-build-h`.

Five inspected 1402×951 window captures at 125% scale are saved under
`C:/Users/kalib/Aurora Launcher/work/server-list/captures/`: the original
management list, Add, Edit, delete confirmation, and Home's Saved tab after a
resource-pack edit, upward move and confirmed deletion. The disposable showcase
window was wholly inside the non-primary portrait monitor; no `javaw` was
running. Only the marked fixture was edited, and the showcase process was closed
after capture. No pre-implementation build capture, light-theme sweep, performance
benchmark or complete screen-reader certification was performed.
