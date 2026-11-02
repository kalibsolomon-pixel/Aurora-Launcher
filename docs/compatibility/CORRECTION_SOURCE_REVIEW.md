# Launcher correction source review

This is a source correction pass on `eb32c0fd1b6c2614d567b88477742de4f40b4e22`, not a 1.4.1 release. Package/native versions remain 1.4.0. No production publication or authority mutation is authorized.

## Investigation before implementation

- Installed Mods, Resource Packs and Shaders already share `InstalledArtwork` and the Rust project-identity cache. Cached raster bytes become data URLs. Production CSP admits the Modrinth CDN but omits `data:` from `img-src`, so all three installed surfaces fail together. Browse already admits only Rust-normalized official CDN artwork and handles image errors; it does not share this CSP failure. Keep that provider architecture and unify the deliberate fallback presentation.
- Recent Servers receives actual status-protocol favicons, not website banners. Native normalization and serialization retain the data URL, including persisted offline reuse. The installed 1.4.0 app visibly reproduces broken images for `mcpvp.club`, `minemen.club` and `pvphq.com`. The same CSP omission blocks them, and the widget has no image-error fallback. Cached favicons also need validation on read before frontend exposure. No new favicon websites, scraping or banner provider is needed.
- The Home listbox already owns correct native selection, arrows/Home/End, typeahead and focus return. Its trigger/menu use older small radii and strong menu borders; the caret is a Unicode character. Restyle this existing component with Phase-F tokens and the existing local SVG chevron.
- Instances combines the global group's bottom margin with a separate grid gap; the Modpacks-to-columns gap differs from the column/card gap. Use one outer gap and remove outer group margins within this screen. Preserve existing-first DOM order and responsive stacking.
- Settings currently exposes Appearance, Home, Updates and Discord & privacy. Windows shortcut management remains implemented in Rust and its frontend store but disappeared from normal Settings during Phase F. General will contain that existing desktop-integration behavior: live Desktop shortcut status/action, installer-owned Start menu status, and Refresh status. No new config fields, sync, startup policy, Java settings or invented launch behavior.
- Discord already runs in a detached native actor. It tries enabled connections on boot, but repeats at a fixed 15-second interval, exposes manual Connect/Reconnect, and keeps publishing null activity while disabled after connection. Add capped retry backoff and a disabled clear/disconnect transition; retain all privacy opt-ins and narrow commands.
- Accounts uses a native `<dialog>` with a centered 680px maximum width. Keep native modal focus/inert/Escape semantics and existing account contents/actions; change geometry to a restrained full-height left drawer beside the left account dock, with internal scrolling and backdrop dismissal.
- Create-instance help hard-codes Aurora 2.1.2 / Fabric 0.19.5, while the existing bundled production release source already contains Client 2.1.5. Derive compatible help from the native compatibility DTO instead of introducing another version constant. Historical acceptance documents describe their original releases and are not current UI copy.

## Evidence and safety baseline

Full status, tracked paths, protected untracked SHA-256s and critical-file hashes are recorded under `C:\Users\kalib\AppData\Local\Temp\aurora-correction-f66955a11232483ebf30702bbd44f08e`. HEAD, main and origin/main all match the baseline; main/origin divergence is 0/0. Original branch: `codex/ref-backed-update-authority`; correction branch: `codex/launcher-correction-source`. No tracked modifications existed. All existing diagnostics are protected.

## Implemented corrections

1. Recent Servers uses the shared raster component and a local server SVG fallback. Rust normalizes wrapped base64, bounds encoded/decoded data and PNG decoding, and revalidates cache payloads on read. Production CSP admits raster data URLs without expanding network permissions.
2. The Home selector uses Phase-F radii, edges and glass highlights, a centered local 24px chevron, soft selected fill and an explicit Selected marker. A dense semantic surface under the menu shields underlying text where nested backdrop filters stop at the parent card. Keyboard arrows/Home/End/typeahead, Escape/outside close and native selection remain. Browser regression found and fixed focus loss caused by the native selection's temporary disabled state.
3. Instances uses one 24px gap for outer panels and columns, 16px form grouping and consistent help padding, retaining existing-first responsive stacking.
4. Installed and browsed Mods share the artwork component.
5. Installed and browsed Resource Packs share the same component; provider ownership gates installed project identity.
6. Installed and browsed Shaders share the component. Known recognition results can show provider artwork before adoption. Unknown local content stays a deterministic type-specific SVG; ownership and game activation are unchanged.
7. Discord's detached actor connects automatically while enabled, checks healthy IPC every 15 seconds and retries failures after 15/30/60/120 seconds (capped). Activity events cannot bypass pending retries. Disable clears once, disconnects, then waits without connection/publication polling; re-enable starts immediately. Normal UI has no Connect/Reconnect button. All privacy opt-ins remain.
8. General exposes existing Windows desktop integration: live Desktop shortcut status with Create/Remove where native capability permits, installer-managed Start menu status and Refresh status. Debug/uninstalled/conflicting/unsupported cases are honest. No configuration schema, migration, new preference or sync feature was added.
9. Accounts is a 420px left native-dialog drawer below the 40px titlebar, full remaining height, full width at narrow sizes, with internal scrolling, close button, backdrop dismissal, native background inertness/Escape and focus return. Add/switch/check/remove/avatar/authentication actions remain unchanged.
10. Creation copy derives the compatible Client version and required loader from the existing native compatibility/release authority. Current Client is 2.1.5; no non-test frontend 2.1.2 text remains.

## Verification

- `npm test`: **228 passed, 0 failed**, 16 suites. Final log: [frontend-tests-final.txt](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/frontend-tests-final.txt).
- `npm run check`: **0 errors, 0 warnings**. [type-check-final.txt](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/type-check-final.txt).
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: passed.
- `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`: passed. [rust-check.txt](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/rust-check.txt).
- `cargo test --manifest-path src-tauri/Cargo.toml`: **823 unit tests passed, 29 existing opt-in tests ignored; 6 icon integration tests passed**. [rust-tests.txt](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/rust-tests.txt).
- Focused native suites: artwork 4 passed; server enrichment 26 passed/1 ignored; Discord 17 passed. Final frontend focus/settings suite: 39 passed. The complete suite includes artwork allow-list/fallback, cache coalescing, server payload normalization/corrupt cache, selection/native boundaries, General capability/commands and authoritative version copy.
- `npm run tauri -- build --debug --no-bundle` with the existing public Discord application ID: passed; this also ran the **production frontend build** and 1.4.0 version-consistency check. Unsigned debug executable only; no installer. [tauri-build.txt](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/tauri-build.txt). Existing STATIC_VCRUNTIME deprecation and test-only dead-code warnings remain.
- Bundled Playwright/Chromium on the real Svelte review surfaces: keyboard selection and focus return, Escape/outside close, drawer geometry/tab containment/Escape/backdrop/focus return/narrow width, malformed provider image removal with SVG fallback, General capability gate and absent manual Discord control all passed, with no uncaught browser errors. [browser-tests.txt](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/browser-tests.txt).
- An intervening unbounded frontend rerun invalidated workers' shared Vite pre-bundle and timed out. The newly added General SSR suite now has its own temporary cache; the final ordinary `npm test` passes. A bounded full rerun also passed 228/228.

## Native runtime acceptance remains pending

The installed 1.4.0 app reproduced the three broken server images before implementation. During preparation for the correction-build boot, an existing Minecraft 1.21.11 multiplayer session was found running. No game, installed launcher or Discord process was closed, and no correction app was started against that active data. A user-input request asks whether that session may be closed normally for acceptance; no response was received during this pass.

Consequently **PARTIAL**: source and automated/build verification are complete, but the correction build's real-state boot/screenshots, actual managed icons/offline reuse/content controls, actual instance selection/creation controls, and normal Ready → Starting → Running → Aurora menu → normal Quit → Ready/exit 0 are not claimed. Live desktop Discord already-open/absent/late-start/disconnect/disabled/re-enabled scenarios are also not claimed; the corresponding deterministic worker lifecycle tests pass. These fixture screenshots do not replace native runtime acceptance or owner visual approval.

Configuration, instance registry and ordinary accounts match the saved baseline byte-for-byte: [launcher-state-preservation.json](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/launcher-state-preservation.json). Credentials were not inspected or changed. This agent performed no launcher-data mutations.

## Visual evidence

The installed before image is 2048×1152. All corrected review-fixture images are **1440×960**, explicitly labelled fixtures. Artwork in these fixtures intentionally demonstrates deterministic fallback, not live provider acquisition; there are no invented provider images.

- [Installed 1.4.0 Home baseline](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/00-baseline-home.png)
- [Home, selector closed](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/01-home-closed-fixture.png)
- [Home, selector open](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/02-home-selector-open-fixture.png)
- [Accounts drawer](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/03-accounts-drawer-fixture.png)
- [Recent Servers fallback](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/03b-recent-servers-fallback-fixture.png)
- [Instances complete desktop composition](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/04-instances-fixture.png)
- [Mods installed](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/05-mods-installed-fixture.png)
- [Mods Browse Modrinth](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/06-mods-browse-fixture.png)
- [Resource Packs installed](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/05-resource-packs-installed-fixture.png)
- [Resource Packs Browse Modrinth](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/06-resource-packs-browse-fixture.png)
- [Shaders installed](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/05-shaders-installed-fixture.png)
- [Shaders Browse Modrinth](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/06-shaders-browse-fixture.png)
- [Settings General](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/07-settings-general-fixture.png)
- [Settings Discord & privacy](C:/Users/kalib/AppData/Local/Temp/aurora-correction-f66955a11232483ebf30702bbd44f08e/visual-fixtures/08-settings-discord-fixture.png)

## Repository and release boundary

Before every commit the 376 original tracked paths and every protected untracked file/hash were checked, all critical files confirmed present, name-status/deletions inspected and working/staged diff-check passed. No unexpected deletion occurred and no generated/native/game/runtime data or developer diagnostics were staged. Tauri CLI normalized Cargo.toml line endings during build; the original bytes were restored by matching the recorded baseline SHA-256. Lockfiles, versions and release/update code remain unchanged.

```
SOURCE PUSHED: NO
VERSION CHANGED TO 1.4.1: NO
TAG CREATED: NO
RELEASE CREATED/MODIFIED: NO
AUTHORITY REF MODIFIED: NO
MANIFEST PUBLISHED: NO
RELEASE WORKFLOW RUN: NO
DEPLOYMENT APPROVED: NO
```

Finish native runtime acceptance when the existing game session may be closed normally, then seek owner visual review and independent review. No release preparation or publication is authorized by this correction pass.
