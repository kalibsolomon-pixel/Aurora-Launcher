# Phase H content controls — resource packs, shaders, adopted content and player arrows

Owner-directed corrective pass, verified on 2026-10-01 on the development
machine (Windows, 125% display scale). Native evidence images are committed
under `docs/phase-h-corrections/` (identical working copies live in
`.zcode-diag/phase-h-content-controls/`); every capture is the real Tauri
window with the real backend and the developer's real launcher data.
Destructive acceptance operated only on content added by this session
(Complementary Reimagined, BSL v10.1.8, Fast Better Grass); the owner's
packs, mods, options and skin were restored to their original state and
verified on disk afterward.

## Root cause — why packs showed no controls and the word "Unavailable"

Three stacked causes, not one:

1. **DTO capability inference** (`instance_content.rs`): `can_remove` was
   `zip && UserManaged` only, so every verified provider-managed pack
   (Modrinth-installed and Phase-G adopted) reported `canRemove: false`, and
   the packs row's only fallback rendering for that state was the literal
   `Unavailable` span. The entry-level `remove` even contained a dead
   provider-managed branch that the gate made unreachable.
2. **UI-only omission**: the packs panel never routed managed rows to the
   existing graph-safe provider removal (`preview_provider_removal` /
   `apply_provider_removal`) the way the Mods panel does, and passed
   `showRemoval={false}` to the provider lifecycle details.
3. **Missing backend operation**: nothing read or wrote Minecraft's
   `options.txt`, so resource packs had no enable/disable representation at
   all, and shader activation had no truthful representation — both collapsed
   into "Unavailable".

A fourth, **latent parser defect** surfaced during live acceptance:
Minecraft 1.21.11 writes `resourcePacks` as a bracketed list of *quoted*
entries (`resourcePacks:["vanilla","file/Pack.zip"]`). The first
implementation compared raw entries without unquoting, which misreported
genuinely-enabled packs as disabled and could duplicate entries on write.
The final `pack_activation` module parses both quoted (modern) and unquoted
(legacy) forms, matches by value, and writes the modern quoted form with
`\"`/`\\` escaping; a write also collapses duplicate values (quoted plus
legacy duplicates) to each value's first position — the same normalization
Minecraft itself performs.

## Capability model (backend-owned)

`ContentEntry.management` (`ContentManagement`) is the backend's explicit
statement of what is safely supported; Svelte renders it and never infers
legality from content types, ownership strings or provider names:

- `canRemove` + `removalPath` (`localFile` | `providerGraph` | `blocked`) —
  verified managed ZIPs route to the provider graph (dependency-enforced
  preview/transaction); unmanaged local ZIPs to the entry-level file
  removal; folders/links/hash-mismatched records are blocked with a real
  reason.
- `canToggle`, `active`, `activationManagedInGame`, `toggleBlockedReason` —
  resource packs report live options.txt state; shader packs truthfully
  report `activationManagedInGame` with `active: null` (Aurora does not own
  loader-specific shader configuration and never fakes a toggle).

## Resource packs

- Enable/disable owns only the `resourcePacks:` line of the instance game
  directory's `options.txt` (the game directory is the instance root):
  spliced byte-exactly, other lines and line endings preserved, written
  atomically (temp sibling + rename) under the instance content lock. No
  watchers, timers or polling — state is read on demand per scan.
- Enabling inserts `file/<name>` at the top (in-game behavior) and preserves
  every other entry's order; disabling removes all references to the value.
- Malformed options fail safely: the scan reports `active: null` with
  `canToggle: false` and a reason; mutations are refused (`options_malformed`
  / `unsupported_content_action`) and the document is never rewritten.
  Names that cannot be represented unambiguously (control characters, padded
  names) are refused rather than guessed at.
- Removing an enabled local pack retires its enabled reference in the same
  locked operation; a failed options update restores the file to its
  original name. When options.txt is unreadable, removal proceeds without
  activation reconciliation (nothing was provably enabled) and leaves the
  malformed document byte-for-byte intact.
- Provider transactions reconcile options.txt coherently: a removed managed
  pack loses its enabled reference; a managed update that renames the pack
  file migrates the reference, so an enabled pack stays enabled across
  updates. The reconciliation runs inside the existing staged/verified
  rollback lifecycle (a failure restores files, state and the original
  options.txt bytes).

## Shader packs

- Remove is available for managed (including adopted) shader packs through
  the provider graph preview/transaction and for unmanaged local ZIPs
  through the entry-level removal; unrelated files are preserved.
- Active-state control is deliberately **not** faked: Aurora instances ship
  Fabric + Aurora Client only; shader loaders (Iris, OptiFine-like) are
  user-installed and each owns activation state in loader-specific
  configuration Aurora does not own. The row shows the truthful
  `Activation managed in-game` status; `set_instance_pack_enabled` refuses
  shader packs with that reason. No shader loader is installed automatically.

## Adopted / recognized content

- Recovered packs (Phase G) receive the same capabilities as direct managed
  content — verified live: a downloaded BSL v10.1.8.zip placed as a local
  file was recognized by SHA-512, adopted with `origin: recovered`, rendered
  with provider artwork and full controls, and removed through the provider
  preview. Provenance stays `recovered`; nothing is rewritten to unlock
  controls, and there is no parallel updater (updates flow through the same
  Phase H multi-root transaction, which also migrates enabled references).
- Unmanaged local packs keep exactly their honest capabilities: file removal
  and options toggling, never provider-only operations.

## Installed-content UI

- Pack rows follow the Mods visual language: artwork/fallback, name,
  concise managed/local + version state, activation status, toggle,
  trash, Details chevron; everything verbose (file name, provider identity,
  origin, removal/activation reasons, warnings, provider lifecycle) lives in
  the expanded details.
- `Unavailable` no longer exists on these surfaces: blocked operations are
  disabled controls with their real reason (title text and details rows);
  shader activation says what is true (`Activation managed in-game`).
- Mods behavior is untouched (the panel is the reference; its tests and
  dependency-blocked trash were re-verified).

## Home player preview — reversed arrows

- Root cause: the renderer projects the model's front (its −z face) to
  screen x as `−sin(yaw)·z`, so *positive* yaw turns the displayed player
  toward screen **left**. The handlers were inverted (`left → yaw -= .35`,
  `right → yaw += .35`), producing the owner-reported opposite rotation.
- Fix: `arrowYawDelta` in `playerModel.ts` binds the arrows to the rendered
  direction (left increases yaw, right decreases); the canonical −0.42
  default and 0.35 increment are unchanged and reset restores exactly the
  canonical angle. No drag handlers, pointer capture, grab cursors or
  animation loops exist (source probe + tests), and pointer movement still
  causes zero redraws.
- Direction is regression-tested **visually**: the default skin's eye pixels
  exist only on the front face, so the tests measure their rendered
  horizontal centroid — left must move it left, right must move it right,
  and left+right / right+left must reproduce the original frame byte-exactly.

## Evidence index (`docs/phase-h-corrections/`)

All captures are native/live unless noted.

- `01-mods-management-reference.png` — Mods reference surface (unchanged).
- `02-resourcepacks-managed-collapsed.png`, `02b-resourcepacks-local-controls.png` — collapsed pack rows with toggles, trash, details and truthful Enabled/Disabled states.
- `03-resourcepack-expanded.png` / `06-resourcepack-adopted-local.png` — expanded details: file, provider identity, `Origin: Recovered from local file`.
- `04-resourcepack-disabled.png` / `05-resourcepack-reenabled.png` — live disable/re-enable with the options.txt line verified after each step.
- `07-shaders-managed-collapsed.png`, `07b-shaders-empty-installed.png`, `08-shader-expanded.png` — managed shader rows with provider artwork.
- `09a-shader-recognition-panel.png` / `09-shader-adopted-local.png` — BSL recognized by hash and adopted (`origin: recovered` verified in `content-managed.json`).
- `10-shader-activation-state.png` — truthful `Activation managed in-game` with Remove available.
- `11-removal-confirmation-or-result.png` — provider removal preview (will-remove delta, Cancel/Approve).
- `12-content-final-regression.png` — final state after cleanup: only the owner's six packs, restored activation set.
- `13`–`17` player direction sequence (same skin): default → left (visibly rotated left) → reset → right (visibly rotated right) → reset; visual direction independently confirmed by image analysis.
- `18`–`20` player controls at 1280×900, 1000×720 and the narrowest supported width (930×700, above the 900 px hide breakpoint) — all three controls visible and clickable.

## Checks

- `cargo fmt --all -- --check` clean; `cargo check --all-targets` 0 errors;
  `cargo test` **660 passed, 0 failed** (25 ignored live-network tests) plus
  6 integration tests.
- `npm test` **171 passed, 0 failed**; `svelte-check` 0 errors 0 warnings;
  production `vite build` and full `tauri build` (msi + nsis) succeed.
- Phase G recognition tests, Phase H managed-update/bulk-transaction tests,
  provider lifecycle, dependency and content-inventory tests all pass
  unchanged; artwork behavior is covered by the existing
  `projectArtwork.test.ts` and live captures show provider artwork on
  managed rows.
- No timers, watchers, polling or startup network work were added; the only
  new I/O is on-demand options.txt parsing inside existing user-triggered
  scans/mutations.
