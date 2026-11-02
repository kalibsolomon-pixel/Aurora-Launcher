# Phase H interface corrections — C18/C19/C20 acceptance

Owner-directed corrections to three surfaces, verified on 2026-10-01 on the
development machine (Windows, 125% display scale). Evidence images are in
`docs/phase-h-corrections/`. Browser fixtures render the real production
components through the Phase F visual harness (`tools/visual/serve.mjs`,
port 1422); captures whose names contain `real-window` are the actual Tauri
window with the real backend and the developer's real launcher data.

## Instances page

- old hierarchy: `Create instance` (narrow left column) preceded `Your instances`; creation occupied the primary upper-left position.
- new hierarchy: with one or more instances, `Your instances` is the primary left/wider column and `Create instance` is secondary on the right; DOM order, reading order and keyboard order all follow the rendered branch (no CSS-only reordering).
- one-instance layout: `docs/phase-h-corrections/25-instances-one-existing-first.png` (fixture) and `25b-real-window-instances-one.png` (real backend). The selected instance, its Ready state, Open and Validate are immediately visible. Measured columns at 1440 CSS px: Your instances 675 px left, Create instance 499 px right; the first instance action button precedes the create form's Name input in DOM order.
- multiple-instance layout: `26-instances-multiple.png` (4 instances) — scannable full-width rows in the wider column; the creation form stays top-aligned and secondary; no overlap or clipping.
- zero-instance behavior: `27-instances-zero-empty-state.png` — creation becomes the obvious primary action (centered single column); only a compact `Your instances — Nothing here yet.` note follows it; no large empty list region is reserved above the form. While launcher state is loading (or failed to load) the existing-first arrangement is kept so the page never flashes to the empty-state layout and back.
- responsive stacking: `28-instances-narrow-stacked.png` at 1100 CSS px — `Your instances` stacks above `Create instance`; programmatic check confirmed DOM order (`compareDocumentPosition` following) and geometry (list top 134 px vs create top 485 px, same column). Pure grid flow — no media query reorders content.
- DOM/keyboard order: matches visual order in both arrangements (verified programmatically; Tab order follows the same DOM sequence).
- instance creation behavior preserved: the create form, its naming/version/platform/loader/Aurora/snapshot controls, compatibility gating, validation and the managed installation pipeline are unchanged — only the two sections' order and the layout grid changed. The development-fixture footer text remains at its existing prominence.

## Provider category UI

- root cause of malformed layout: the scoped `.browse input, .browse select` text-field rule also matched `input[type="checkbox"]` inside Browse. At equal specificity with `.f-pilot input[type="checkbox"]`, component order let it win, so every category checkbox rendered as a full-width padded, bordered, sunken box; inside the two-column list this produced separated checkbox/label columns, misaligned rows and the broken-table look. The popover also used `overflow: auto` (both axes) with a `100vw`-derived width.
- row structure: each category is one coherent `<label>` row — flex, `align-items: center`, fixed 16-18 px checkbox footprint (`flex: none`), label taking the remaining width with `overflow-wrap: anywhere`. The whole row is clickable with native checkbox semantics and implicit label association. Verified programmatically: every checkbox's right edge is 12 px from its label's left edge on the same centerline (`allAdjacent`/`rowAlignment` true for Mods, Resource Packs, Shaders, Modpacks).
- popover width: fixed 300 px, `max-width: calc(100vw - 48px)`, anchored to the Categories trigger; long names wrap inside rows instead of resizing the popover.
- horizontal overflow: none — `overflow-y: auto; overflow-x: hidden`; measured `scrollWidth <= clientWidth` in the popover and the document at every tested size (1440, 1000 CSS px).
- vertical overflow: `max-height: 320 px` with working vertical scrolling (Mods lists 28 provider categories, Resource Packs 20, Shaders 11, Modpacks 12).
- label formatting: provider slugs are presented readably (`game-mechanics` → `Game Mechanics`, `colored-lighting` → `Colored Lighting`, `path-tracing` → `Path Tracing`, `vanilla-like` → `Vanilla-like`, `semi-realistic` → `Semi-Realistic`) via `formatCategoryLabel` in `modrinthBrowse.ts`; the provider value sent in requests is unchanged. Unit-tested in `modrinthBrowse.test.ts`.
- multi-select: verified end to end — three categories checked simultaneously, chips shown outside the popover with matching readable labels, trigger reads `Categories (3)`, reopening preserves checks, Escape closes without losing selections, removing one chip leaves the others intact and the popover does not close when toggling. Category toggling now reuses the typed search term and current sort (previously the typed term was silently dropped) and keeps the existing request-serial stale-response protection.
- content-type switching: Mods/Modpacks/Resource Packs/Shaders each show only their provider-attributed categories; switching clears incompatible facets (verified: no checked rows and no chips after switching) — the pre-existing deliberate behavior.
- keyboard behavior: Tab reaches the Categories control (verified from the search box: Search → Categories); Escape closes and returns focus to the trigger; checkboxes are native controls with label association; Enter/Space activation is native button/checkbox semantics (Enter activation additionally demonstrated on the real Tauri window); focus outlines remain visible.
- responsive behavior: at 1000×720 the popover stays inside the viewport (right edge 452 px), causes no page-level horizontal scroll and does not shift the result grid.
- provider semantics preserved: `ProviderBrowseQuery`, live Modrinth tag discovery, project-type attribution, multi-select facets, Minecraft-version and loader filtering, sorting and stale-response serialization are untouched; the visible list remains provider-driven (the review fixture only supplies provider-shaped data, including realistic slug sets per project type).

## Responsive player controls

- root cause: the controls were hover-revealed (`opacity: 0` until hovering the preview), shrunk to 28 px icon buttons with 11 px reset text, and the width-driven canvas could overflow its max-height-clamped container, so at windowed sizes the only rotation affordance was easy to miss and hard to hit.
- control container architecture: the three controls remain structural children of `PlayerPreview` (`player-model` then `player-controls`), laid out in normal flow in a flex column with a reserved 14 px gap — never page-relative, absolute, or overlapped.
- model sizing strategy: one bound `--player-max-h: min(66vh, 640px, max(280px, calc(100dvh - 300px)))` governs both the model container and the canvas width (`min(100%, 480px, max-h × 0.7)`), so the approved large presentation applies while space allows, yields only when constrained, and the raster can never overflow into the control row (canvas-inside-container verified at every size). The model shrinks before control usability is ever compromised; controls have fixed pixel sizes that do not scale down.
- minimum control hit area: 40×40 px rotate icon buttons (default `.f-icon-button`) and a ~84×37 px Reset view button — verified at 1920×1080, 1600×900, 1440×900, 1280×800, 1100×720, 920×700 CSS.
- window sizes tested: browser matrix 1920×1080, 1440×900, 1280×800, 1100×720, 920×700 (minimum with the player visible, just above the 900 px presentation breakpoint) and 880×700 (below breakpoint); real Tauri window (125% scale) maximized, 1600×900, 1280×800, 1100×720, 920×700, 900×650 and restored-maximized.
- live resize: maximized → windowed → smaller → larger → maximized on the real window with Home open; the model, skin and controls returned without navigation, restart, account or skin refresh. Browser sequence additionally clicked rotate/reset after every resize step (controls remained usable at each size). No resize polling was added — the existing `matchMedia` breakpoint listener and the change-driven redraw effect handle it.
- left rotation: functional — canvas raster comparison changed on rotate-left at 1440×900, 1280×800, 1100×720, 920×700; visible angle change on the real window (`36b-real-window-1600x900-rotated.png`).
- right rotation: functional — distinct raster on rotate-right at every size; visible turn on the real window at 1100×720 (`38b`).
- reset: functional — restore produced the byte-identical baseline raster at every browser size; frame change on reset on the real window at 920×700 (`39c`).
- keyboard: Enter activation on the focused rotate control demonstrated on the real Tauri window (frame changed); no global Left/Right capture exists; accessible names preserved (`Rotate player left`, `Reset player view`, `Rotate player right`).
- hit testing: `document.elementFromPoint` at button centers and edges returned the button (or its child icon) at every size — no canvas, Borealis surface or widget intercepts clicks; z-index remains local (the pill has no global stacking change).
- overlap/clipping: controls sit strictly below the model raster, inside the preview container and inside the viewport at every supported size; nothing is clipped and they never overlap the player's raster, widgets or window edges. A hover capture shows the interactive surface (`36b-rotate-button-hover-surface.png`).
- player-hidden breakpoint: below 900 px width the entire preview (model and controls) hides — verified on the real window at 900 CSS px width; there is no state where the player is visible but its controls are unusable.
- drag rotation remains removed: no pointermove/drag/touch rotation, no grab cursors; the arrows and Reset view are the only manual view controls.
- timers/polling added: none — one redraw per change (existing effect), existing 30 ms reveal timeout and matchMedia listener only.
- skin behavior preserved: no generic player, empty region until a real skin exists, account-keyed cache, immediate cached known-good display, larger desktop presentation and no fake-placeholder layout shift; Skin Manager/Skin Library and avatar persistence work are untouched.

## Evidence index (`docs/phase-h-corrections/`)

25 one instance · 25b real window one instance · 26 multiple instances · 27 zero-instance empty state · 28 narrow stacked · 29 Mods picker · 30 Mods multi-select · 31 Resource Packs picker · 32 Shaders picker · 33 Modpacks picker · 34 picker at 1000×720 · 35 Home large · 35b real window maximized · 36 1440×900 · 36b real window 1600×900 rotated + hover surface · 37 1280×800 · 37b real window 1280×800 (Enter activation) · 38 1100×720 · 38b real window 1100×720 · 39 920×700 · 39b/39c real window 920×700 rotate/reset · 39d real window 900×650 (player hidden) · 40 live-resize restored · 40b real window restored maximized.

## Checks

`npm run check` 0 errors/0 warnings; `npm test` 152/152; `npm run build` clean; `cargo fmt --check` clean (no Rust changes). Real application boot verified via `npm run tauri dev` against the real backend and developer data; no game launch, account change or instance mutation was performed for evidence.
