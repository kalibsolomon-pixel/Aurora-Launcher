# Phase E3 acceptance record

Status: **PASSED**. Implementation acceptance and automated acceptance passed, and rendered visual acceptance passed through the combination of the prior agent native inspection and the owner's final manual inspection of the remaining required states. Live remote skin/cape mutation was optional and was not performed, so no live mutation is claimed. The earlier continuation session stopped further native interaction because its accessibility tree became unsafe (another application's elements mixed into the launcher's tree and unintended local state changes showed a real risk of an accidental genuine mutation); that record is preserved below.

## Implementation and trust boundary

- `skin-manager` and `cape-selector` are registered Home widgets with small, wide and large sizes. Fresh defaults place them after E2 widgets; persisted E2 layouts retain their exact order, sizes and visibility until a user enables the new IDs. Show/hide, reorder, resize and reset use the existing layout commands.
- Current account skin and capes come from `GET /minecraft/profile` through a Rust-owned Minecraft token. Accounts remain separate from launcher-global local presets. Account switching clears the former account's remote state before fetching the new selection.
- The browser's system file picker supplies selected PNG bytes, never a path. Native import requires a complete CRC-checked PNG stream, 64×64 or legacy 64×32 pixels, RGB/RGBA output, a 128 KiB compressed bound and a 1 MiB decoder bound. Broken, oversized and unsupported images fail with structured errors.
- Schema-1 preset metadata and generated UUID filenames live under managed `launcher/skin-presets/`. A SHA-256 is checked again before upload. Writes use synced temporary siblings and rename. A damaged or future-schema document is preserved. Removing a preset removes only that generated managed PNG and cannot change a remote skin.
- Skin upload sends explicit `classic` or `slim` in a multipart request, then re-fetches the authenticated profile. Cape selection re-fetches ownership before PUT; disabling sends DELETE and re-fetches. Service failures and 429/401 statuses become bounded structured errors; raw bodies, tokens and Authorization values are not returned. No mutation is retried automatically.
- Skin Manager reuses the existing 3D player renderer at a compact size, so the current skin remains visible when Home's large hero preview is hidden at narrow widths. A successful skin apply asks the existing account avatar command to bypass its texture cache, updating both previews. Owned cape textures are fetched only from native-validated `textures.minecraft.net` locators, decoded under bounds and returned as thumbnail pixels. The existing 3D renderer remains skin-only.
- Cosmetics are optional: remote failure leaves Play's native readiness and launch path separate; local presets remain available. Profile requests are on account selection and explicit refresh/mutation, with no polling loop.

## Automated verification

| Check | Result |
|---|---|
| Rust unit tests | 562 passed, 21 ignored, 0 failed |
| Icon integration tests | 6 passed, 0 failed |
| Frontend tests | 130 passed, 0 failed |
| Svelte/TypeScript | 0 errors, 0 warnings |
| Rust format and all-target check | Passed |
| Frontend production build | Passed |
| Tauri MSI/NSIS build | Passed — `Aurora Launcher_1.1.0_x64_en-US.msi` and `..._x64-setup.exe` produced at the exact E3 tree; the release binary contains the cosmetics code (installers stay uncommitted) |

Deterministic native cases cover PNG bounds and malformed structure, generated preset identity and duplicate display names, damage/future-schema preservation, profile/account isolation, multipart model mapping, owned cape checks, PUT/DELETE refresh, rate-limit redaction and native cape thumbnail decoding. Existing auth refresh, Play, E1 history, E2 Quick Launch, Discord/privacy, release/version and icon suites all ran in the full regressions above. Frontend tests cover current/no-account/offline skin states, model controls, preset row limits, owned/empty/selected/disabled cape states, account isolation and exact semantic command payloads.

## Native-window visual inspection

The built Windows Tauri executable reached a real Home window. I inspected a normal 922×671 window and a wide 2048×1152 window. The real account's current skin rendered in the existing 3D preview, the Skin Manager showed its Classic state and a launcher-local preset, and Cape Selector showed three owned capes with one active. The E2 widgets remained in the combined layout. The Import PNG button opened the Windows file picker; Escape returned focus to the button with a visible focus ring. I did not use the Apply, Select or Disable actions.

### Continuation session (same working tree, unchanged code)

The release build was re-launched from the exact E3 tree and reached the Home window again. A normal 1168×847 window showed the combined E2+E3 layout (Recent Worlds, Recent Servers, Skin Manager, Cape Selector, Playtime) with real account data, captured in `.zcode-diag6/e3-visual/01-home-combined-e2-e3-normal.png`. The rendered Skin Manager reported "Current account skin · Classic" with the saved `Spxcterr-skin` preset, model dropdown and Import/Refresh controls; the rendered Cape Selector listed the selected account's three owned capes (Common active, Pan, Cherry Blossom) with native thumbnail images, a disabled Select on the active cape and an enabled Disable action. A maximized (wide) window kept the combined layout readable (`.zcode-diag6/e3-visual/02-wide-window-widgets.png`). A real account switch to a second signed-in account occurred in the native window and the account chip, name and head avatar all followed the new selection immediately; no cosmetic mutation was issued.

Continuation was then halted deliberately: the accessibility tree of the session host mixed another application's elements into this window's tree, index-based interaction became unreliable, and unintended local state changes (widget visibility toggles, account selection) demonstrated a real risk of accidentally invoking a genuine Apply/Select/Disable against the owner's account. The launcher process was closed and the owner's application data was restored exactly: `config.json` byte-for-byte from a pre-launch backup, the persisted selected account returned to its original value, and the preset store verified unchanged. No fixture presets were imported and no remote mutation occurred. Remaining visual states were deliberately left to owner manual acceptance rather than risk a live mutation.

These states were **not** all visually witnessed in the native window: multiple presets, Slim selection, mutation loading/error/success, disabled cape, no owned capes, offline cosmetics, the E3 widgets' content after account switch (the switch itself was witnessed; the widgets had been hidden by the accidental toggles by then), minimum 720×520 size, expanded widget sizes, every theme, and all long-name/hover combinations. Deterministic rendered Svelte assertions cover their text/control states but do not substitute for complete native visual acceptance. The full E3 visual gate therefore remains open.

### Owner visual acceptance steps

1. Start a built E3 launcher with the desired account. On Home, enable Skin Manager and Cape Selector in Settings if the saved E2 layout omits them. Confirm current skin and owned cape identities, then switch accounts and confirm remote state follows the selected account. Do not use Apply/Select/Disable for this visual step.
2. Import two safe 64×64 PNGs as local presets, one named long enough to ellipsize. Choose Classic and Slim in the preset model controls without applying either. Inspect compact, wide and large widget sizes through Customize Home at 720×520, normal and wide windows, including the E2 widgets.
3. Inspect the system file picker, keyboard Tab focus, hover, default/Midnight/OLED themes, a no-account state and an offline profile state. For no-cape, error and loading visuals, use a disposable deterministic native/mock fixture or a controlled account state; do not alter the owner's cape merely to create screenshots.
4. Record screenshots or a clear owner statement identifying each of the sixteen required visual states and any issues fixed. Until that evidence exists, report E3 as stopped at the visual gate.

### Owner visual acceptance (completed)

The repository owner has manually completed the remaining E3 visual acceptance. Owner attestation, recorded verbatim:

> "E3 visual acceptance passed. I checked the remaining required visual states and found no issues. I did not perform the optional live skin/cape mutation."

This is owner-supplied manual evidence. The owner-inspected states were those the agent had not witnessed in the native window: multiple saved presets, Slim model presentation/selection, apply loading state, apply error state, disabled/no-active-cape state, no-owned-capes state, offline state, second-account cosmetic presentation, small/wide/large widget sizes, the minimum 720×520 window, long preset-name truncation/ellipsis, and additional theme presentation. The owner also confirmed overall that the required E3 visual acceptance states were checked and no visual issues were found. No screenshots of the owner's pass are claimed, no computer-vision verification of owner-only states is claimed, and no detail beyond the attestation above is asserted.

Evidence attribution: agent-gathered evidence covers the states listed under "Native-window visual inspection" above (combined E2+E3 layout, current skin, one preset, three owned capes with one active, native cape thumbnails, import file picker, keyboard focus ring, normal and maximized windows, Skin Manager and Cape Selector controls, and an account switch). Everything else rests on the owner's manual inspection recorded in this section. The two agent screenshots under `.zcode-diag6/e3-visual/` remain protected local diagnostics and are not committed.

## Voluntary live mutation

LIVE REMOTE COSMETIC MUTATION: **NOT PERFORMED**. No real skin was changed, no real cape was selected, and no real cape was disabled for acceptance — neither by the agent in any session nor by the owner. The owner explicitly declined the optional live mutation in the attestation above, and this is acceptable: E3 acceptance does not require it. The mutation paths are instead supported by deterministic native service tests (multipart model mapping, owned-cape recheck, PUT/DELETE refresh confirmation, structured redacted failures) plus the authentication-boundary, profile-refresh, frontend and rendered UI acceptance above. Mojang/Minecraft production mutation behavior has therefore not been manually proven; if a live check is ever wanted, the owner-only steps below remain the template.

The agent did not invoke a real-account skin or cape mutation. If the owner elects to verify remote behavior, they should explicitly choose a local preset, note the current skin/cape for restoration, Apply once, and confirm the official Minecraft profile plus launcher preview update. Separately select one owned cape and Disable once, checking the official profile after each. These actions affect the real Minecraft account and were not authorized for automated acceptance. No remote mutation is needed to keep the code or automated tests.

## Repository and release scope

Starting HEAD and `origin/main` were `b7b5abc87d71c5c3d9c1f1c7bbaf43a93004f2eb`, with 0 ahead/0 behind. The original tracked set contained 197 paths; the previously observed protected untracked diagnostic set (607 files) was left untouched and grew only by this continuation's two evidence screenshots under `.zcode-diag6/e3-visual/`. The pre-existing Cargo.toml working-tree status is untouched and its content diff is empty (line-ending state only; no E3 change was required — all cosmetics dependencies already existed at the baseline). The launcher remains version 1.1.0 and the production Aurora Client pin and bridge behavior are unchanged. No push or client work occurred.
