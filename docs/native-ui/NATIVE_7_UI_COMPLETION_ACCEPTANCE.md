# Aurora Launcher — NATIVE-7 acceptance

The subsequent owner-requested appearance/performance refinement is recorded in
[docs/native-ui/NATIVE_7_UI_REFINEMENT_ACCEPTANCE.md](NATIVE_7_UI_REFINEMENT_ACCEPTANCE.md).
It corrects cape orientation, moves preview rendering off the UI thread and
refines the native layouts. The original phase classification below remains a
historical statement; outstanding backend features are not implicitly complete.

**Classification: PARTIAL — implemented native corrections, incomplete desktop/performance acceptance.**

2026-10-31. Worktree: `C:\Dev\aurora-launcher-native`, branch
`codex/native-ui-migration`. Starting SHA:
`7641d0b3b84a9b01b30ecc3cc0487495c5e09b30` (full SHA recorded in the preservation evidence). Ending implementation
SHA: `2fcf1bf53b1eb24592b0e78371e841fc63aa42a7`. Documentation/evidence are committed separately; see the
local commit history. No push, release, installation over production or updater
publication occurred. Owner visual acceptance is not claimed.

The owner stopped Computer Use with the physical Escape key during optimized
desktop acceptance. No further desktop input was sent. Checks requiring window
control after that point remain explicitly unverified; coding, build checks and
read-only process sampling continued. The disposable observed launcher was left
running, without closing it through an alternative automation path.

## Implemented corrections

### Input and selection

The first-click failure was reproduced against the original native binary:
one primary click on Instances focused its navigation control while Home stayed
visible. Slint 1.18.1's `FocusScope::input_event` accepts the mouse-down that first
gives it focus. The launcher's scopes were siblings above their `TouchArea`s,
so the intended target never saw that press. This affected the shared control
family and custom navigation/rows. It was not a Rust callback or network delay.

Shared `KeyboardFocus` sets `focus-on-click: false`; pointer handlers deliberately
focus and activate their target once. Tab focus and keyboard focus rings remain
separate from pointer selection. No second click is synthesized. Slider release
previously dispatched from both mouse-up and clicked; it now dispatches once.
Checkbox hit testing now covers its complete label/box rectangle.

Sidebar selected fills/bars, tile outlines, tab underlines and account/cape
selection fills were removed. Selected text/icons use the current accent.
Hover feedback remains; focus rings remain for keyboard use. Accent swatches use
a check instead of an outline. Shared button heights are bounded in list rows.

The headless regression dispatches real Slint pointer press/release pairs into
buttons, toggles, checkboxes, tabs, a dropdown, a slider and all sidebar entries,
with both active and inactive `WindowActiveChanged` state. It counts exactly one
callback per click. This tests Slint delivery, not Windows' physical inactive
window mouse-activation message sequence. Live single-click navigation, tabs,
Home popup selection and the account trigger were also exercised. Full live
coverage of every screen and initially unfocused OS windows remains outstanding.

Sources: [Slint FocusScope reference](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/focusscope/)
and the locally pinned 1.18.1 implementation in `items/input_items.rs`.

### Home and accounts

Home now has a compact scrolling instance dropdown, concise installed versions,
authoritative readiness/blockers, progress/errors and the original Play action.
The oversized management panel and secondary grid of instance selectors are gone.
Available instances stay selectable when the current selection is empty. The
controlled dropdown retains Rust’s acknowledged value until selection succeeds,
so a failed save cannot leave a false selected label. Selection still calls
`application::select_instance` and reloads persisted state;
readiness and launch authority are unchanged. No arbitrary fallback instance is
selected. Empty selection and unavailable instances retain explicit blocked states.

Accounts extend from the 88 px sidebar into the workspace, without a full-window
dark scrim. The account list scrolls within a window-dependent bound. Outside
click/Escape dismiss; Escape was observed to return focus to the account trigger.
The Escape scope is an ancestor of dialog controls, rather than a disconnected
sibling. Full keyboard trapping, Narrator, 200% text and a many-account/small-window
desktop run have not been accepted. Switching/sign-out mutations were not tested.

The owner manually interacted with the disposable debug window, signed in and
installed Java while the agent continued working. A subsequent screenshot showed
Rust-owned **Ready**. The agent did not initiate that sign-in, click Play, change
a real skin/cape, sign out, or remove any real account. Shared Windows Credential
Manager semantics remain unchanged; the owner-created disposable account record
and credential were preserved.

### Modrinth and modpacks

Discover connects all four provider kinds to the existing application services.
It now supports project details, exact compatible version selection, review,
safe error/retry states and real project artwork. Metadata and installation remain
Rust-owned. Search/pagination keep request generations; provider offset + 20
advances pages even when normalization filters hits. Duplicate project IDs are
deduplicated; retention caps at 200 results. Icons use the core's isolated decoder,
128 cached/pending identities and a process-wide four-request gate. Failures keep
the glyph fallback and never block browsing.

The original provider globally rejected Vanilla before examining content kind.
Resource/shader compatibility now depends on Minecraft version instead; mods still
reject unsupported loaders. Existing Forge mod restrictions remain deliberate.
Shader ZIPs use the shader content path; “Installed” does not assert that a shader
loader is present. Provider search facet contracts are preserved. See the
[official search documentation](https://docs.modrinth.com/api/operations/searchprojects/).

Find More retains instance + content kind, avoids the previous nested UI-state
borrow during navigation, and offers Back to instance. Returning reloads that
workspace's inventory. The complete live round trip was not captured before
desktop control stopped.

New modpack instances and existing-instance merges are distinct reviewed choices.
The UI freezes target, project, version, workflow and the returned fingerprint;
commit reuses `install_modrinth_pack` or `install_modrinth_pack_merge`, never a
new engine. Pre-install review can be dismissed/cancelled. Once installation has
started, the shared backend still has no cancellation operation; the native UI
does not present a false Cancel action. Optional pack files are explicitly omitted
in this initial native workflow, with their paths/counts shown in review; optional
file selection is not yet exposed.

Live acceptance used only the marked disposable root
`%TEMP%\aurora-native7-live-20261031\managed`:

| Kind/workflow | Live result |
| --- | --- |
| Mods | Initial/search/page 2, metadata/icon, Sodium install and persisted provider identity passed |
| Resource packs | Initial/search/page 2, metadata/icon, Fresh Animations ZIP install and provider identity passed |
| Shaders | Initial/search/empty page 2, metadata/icon, Complementary Reimagined ZIP install and provider identity passed |
| Modpacks | Initial/search/page 2 and real project/version metadata passed |
| New pack instance | Adrenaline for Minecraft 1.21.11 / Fabric 0.19.5 installed as `eaf749874a9a473d84a10412779e22ea` |
| Reviewed ordinary-instance merge | Separate instance `33d424563f564676887bc7d53abfbbd0`: 27 content files, 7 overrides; no managed pack identity |
| Stale review | Wrong fingerprint refused before installation |
| Unsupported snapshots | Simply Optimized Reloaded and a Fabulously Optimized candidate were specifically rejected for an absent declared Modrinth dependency; checks were preserved |
| Interrupted network/install recovery | Not newly fault-injected live; existing core safety/recovery tests retained |

`native/examples/provider_acceptance.rs` exercises the same typed native adapter.
It never handles credentials or launches Minecraft. The retained registry check confirms all three instances are Ready and only
the new pack instance has a managed pack identity. Detailed evidence is in
[live-provider.txt](native7/live-provider.txt).

### Native 3D appearance

`native/src/player.rs` is an isolated CPU rasterizer for at most 78 textured quads.
It produces one fixed 320×440 straight-RGBA image for Slint's existing software
renderer. The mesh includes head, torso, limbs, classic/slim geometry, legacy
mirrored limbs, outer layers and a 10×16 cape attached behind the torso.
Projection uses bounded yaw/pitch/zoom, backface rejection, opaque depth testing,
back-to-front translucent faces and perspective-correct nearest texels. A
top-left edge rule avoids double blending along triangle seams. Empty alpha
regions are skipped before rasterization.

The controller coalesces pointer movement with a one-shot 16 ms timer, renders
only while Skins is shown, and has no idle animation/render loop. Drag, wheel,
arrows and Reset change the view; Outer layers toggles geometry. Library Preview
and cape Preview are local-only. Apply has a separate confirmation bound to the
captured account; Wear/No cape remain explicit account actions. Late cosmetic
responses and library reloads have separate generations; wrong-account responses
cannot replace the preview. Textures are decoded/validated by the existing core,
not repeatedly decoded by the renderer. No GPU renderer, game engine, WebView,
Three.js or browser canvas was added.

Windows PNG import uses a native COM file chooser off the UI thread, reads at
most 128 KiB and calls the existing validated cosmetics importer. Classic/Slim
choice is explicit. Other platforms return a specific unsupported error.
Library thumbnails are pruned to the bounded authoritative library. File chooser
interaction itself was not driven before desktop control stopped.

Tests cover dimensions/UV extents, classic/slim/legacy geometry, real head/torso
UV samples, zero-alpha textures, translucent layers, malformed input, rotation,
cape visibility and the camelCase full-texture DTO contract. Synthetic Classic
and Slim PNGs were imported through the actual core and rendered with a synthetic
cape; malformed PNG import was refused. These are labelled test textures, not
screenshots of a real account's appearance. Live cape selection, authenticated
appearance fidelity and interactive rotation/zoom acceptance remain incomplete.

### Branding

The owner confirmed `C:\Users\kalib\Downloads\aurora.svg` after the named
`aurora(4).svg` attachment could not be found. `native/assets/aurora.svg` is a
byte-identical copy (SHA-256
`e172e46968318a8e540f90a1f86159a11bf1b607d7ffbba6393a2ebaea7b1966`).
The sidebar uses the SVG directly. Generated PNGs feed the window icon and the
multi-resolution ICO embedded by `native/build.rs`. The generator adds transparent
padding only and retains the existing 40-first ICO ordering. The confirmed ribbon
was visible in the running native sidebar and title bar. Native installer
packaging is still absent; its future asset source is documented. Historical
WebView artwork and installer outputs were preserved.

## Verification and resources

| Check | Result |
| --- | --- |
| Native Rust tests | 14 passed |
| Core `--no-default-features` | 1,002 passed, 31 ignored; 6 icon integration tests passed |
| Core default Tauri feature set | 1,008 passed, 31 ignored; 6 icon integration tests passed |
| Rust format | Both crates passed |
| Core/native all-target checks | Passed; final UI also compiled in native tests |
| Svelte type check | 0 errors / 0 warnings |
| Preserved frontend production build | Passed |
| Frontend tests | 301 passed with four workers, ~178.5 s |
| Optimized native build | Passed with `disposable-data-root`; final build log retained |
| Actual native boots | Debug and optimized disposable-root boots observed |

The initial serialized frontend run completed in ~329 s: 300 passed and one
pre-existing CRLF-sensitive CSS assertion failed. Normalizing that test's read
to LF keeps the exact assertion intact. Four-worker verification passed every
test; `npm test` now uses that bounded concurrency. The historical hang was not
reproduced under these bounded runs; its exact earlier cause is not claimed.

First optimized window-handle detection after build was **576 ms** (one run;
not first physical paint, not a warm-start median). An uncontrolled 12-second
post-start sample observed 39.4 MiB working set / 12.8 MiB private and 0.38% of
one core. A later 12-second sample observed 44.2 / 17.6 MiB and 0.00% CPU.
The process samples used the first optimized NATIVE-7 build; final UI edge-case
fixes were compiled/tested but not driven after the stop. Desktop state/activity
was not controlled after the stop, so these do not pass
the requested scenario matrix or establish a comparison with historical NATIVE-6
medians. GPU counters had no matching PID samples; allocation is **not quantified**.

The first optimized renderer microbenchmark averaged ~7.3 ms/frame over 120
changing views for each model, including cape/layers. Final optimized renderer
timing: **2.469 ms/frame Classic; 2.432 ms/frame Slim**, after skipping alpha-empty face work. This measures raster work, not physical frame latency.
No full-app idle Discover, scrolling, active rotation, skin/cape switching,
repeat-navigation, minimized or memory-growth acceptance was completed. Near-zero
idle CPU and full performance preservation therefore remain **partially tested**.

## Functional parity and remaining work

| Surface | Implementation | Verification status |
| --- | --- | --- |
| Single-click shared controls/sidebar | Implemented | Pointer regressions pass; representative live clicks pass; complete inactive-OS matrix pending |
| Accent-only selection and Home dropdown | Implemented | Native screenshots and pointer selection; full focus/accessibility audit pending |
| Existing Rust Play/readiness/lifecycle | Preserved | Tests pass; owner-seeded Ready observed; no new authenticated game launch performed by agent |
| Mod/resource/shader browsing and install | Implemented | All three live service flows pass; screenshots captured |
| Details/version/conflict reviews | Implemented | Compile/unit/service checks; full desktop review flow pending |
| Modpack browse/new install/merge | Implemented | Live services passed; complete desktop flow and interruption recovery pending |
| Post-confirmation cancellation | Unimplemented | Shared backend has no operation |
| Optional pack-file selection | Unimplemented in native UI | Default omission is displayed |
| 3D skin/cape rendering | Implemented | Synthetic texture/UV/alpha/cape tests and native neutral page; live account fidelity/interaction incomplete |
| PNG import | Implemented on Windows | Actual importer positive/negative checks; chooser interaction pending |
| Sidebar accounts | Implemented | Open/Escape/focus return observed; real switching/removal intentionally untested |
| Confirmed logo/native icons | Implemented | SVG hash and native sidebar/title bar observed; no native installer exists |
| Content updates, Home activity widgets, instance icons, exact loader pin UI | Still missing | Existing core functionality retained |
| Discover thematic tag/sort controls, Borealis, Discord settings, full skin library metadata editing | Still missing | Existing services/preferences/data preserved |
| Native installer, self-update migration, Linux runtime acceptance | Still missing/separately gated | No release authority changed |

Next work: resume desktop acceptance only when requested; verify all clicks and
keyboard focus at minimum/default sizes, real appearance rendering without
account-changing tests, complete the optimized scenario matrix, then address
native cancellation/optional pack selection and remaining parity. Investigate
rejected provider dependency snapshots before broadening pack compatibility.
No investigation observation is presented as a verified Client defect.

## Evidence and preservation

Native screenshots and raw checks: [docs/native-ui/native7](native7).
`before-home.png` captures the initial empty native Home; populated pre-phase
comparisons remain in `docs/native-ui/screens/` (historical NATIVE-6 evidence).
The attached request directory contained the written brief but no screenshot files.
Written requirements and real native baseline captures guided the implementation.
Current captures include Home/dropdown, owner-seeded Ready, four Discover kinds,
neutral 3D Skins and the sidebar account panel. `classic-*.png` / `slim-*.png` are
standalone rasterizer outputs using synthetic textures. Final modpack-review,
live cape, narrow-window and theme-comparison captures are outstanding.

The native worktree started clean. The tracked-file baseline, status/history and
critical-file hashes were recorded under `%TEMP%\aurora-native7-20261031` before
editing. No baseline tracked path disappeared, no deletions/renames are staged,
and no game/runtime/cache data or build target is staged. The separate main
checkout's pre-existing modifications/untracked files were not edited. No
`git clean`, `cargo clean`, reset, broad cleanup or process-name termination was
used. Disposable data remains outside the repository. The observed executable
was renamed within ignored build outputs to preserve the running window while
allowing the final build; no window input was sent after the owner's stop.

Authentication, Credential Manager, artifact hashes, normalized plans, managed
path containment, protected content, supervision and schema failure semantics
remain the existing core's authority. No tokens, passwords or launch arguments
were added to UI state or evidence. No separate Aurora Client source was changed.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
