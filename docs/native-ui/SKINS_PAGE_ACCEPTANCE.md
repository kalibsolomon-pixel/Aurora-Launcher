# Skins page: library application and workspace layout

## Behavior

The page now starts with the selected account's look, model and worn cape,
alongside Import PNG, Manage accounts, Refresh, and Save current skin. The
preview and account details share one surface. At narrow widths, account details
and quick actions come first. The preview keeps its rotation, zoom, front/back
and outer-layer controls.

The library is a responsive one-to-four-column grid. Every skin has a thumbnail,
name, model badge, Apply action, Preview and Favorite. Manage library exposes
Rename and Remove; removal keeps its existing review. The library uses the page
scroll rather than a second, height-limited list. Import controls appear on
request, and the empty library includes an Import PNG action. Capes use compact
wrapping cards with Worn, Preview, Wear cape and Stop wearing cape.

Apply opens the existing frozen account/preset/name/model review. Confirmation
still rejects stale targets, rechecks write eligibility, and calls the typed
`api::apply_skin_preset` boundary. No write starts from rendering or previewing.
Signed-out, reauthentication and busy states disable Apply and explain why in
the library and in its accessible name. Successful application installs the
returned cosmetics snapshot, returns the large preview to the current look,
emits the existing change events and shows success feedback. Errors retain the
existing error surface. Account changes still invalidate reviews and remote
results.

Applied matches the current account's PNG SHA-256 and classic/slim model, never
the inspected preview. These public content hashes already existed in the core;
the shared native DTOs now preserve them. Missing legacy hashes cannot create a
false Applied state. Core implementation and persisted schemas are unchanged.

## Visual and interaction acceptance

Captures are local artifacts at:

- `C:/Users/kalib/Aurora Launcher/work/skins-page/captures/before/`
- `C:/Users/kalib/Aurora Launcher/work/skins-page/captures/after/`

The initial baseline was captured before implementation. The additional
`signed-in` baseline uses the original render function with the new disposable
fixtures so the old and new layouts can be compared with the same invented
account and library.

After captures cover the library, confirmation, success, signed-out,
reauthentication, empty library, management controls, and the minimum window.
Default size is 1120 x 760 logical pixels; minimum is 720 x 520. At 120 DPI the
client captures are 1400 x 950 and 900 x 650 respectively. Narrow library and cape
captures verify reflow and readable controls. Scrolled captures intentionally
crop content at the viewport edge.

Self-review shortened the preview, removed its redundant enclosing card border,
removed decorative Apply icons, aligned the empty-state action, and put account
details first when columns stack. Existing theme tokens, typography, cards,
AuroraButton variants and focus behavior are reused. All built-in themes are
dark; no light-theme variant is introduced.

A live disposable window was exercised through Windows accessibility actions:
Apply Winter coat opened review; confirmation changed Applied to Winter coat;
previewing Sunset explorer left Winter coat Applied. Rename and Remove opened
their dialogs and were cancelled. Import PNG exposed the name/model form with
visible input focus. Repeated card actions have skin-specific accessible names.
Full assistive-technology and performance measurements were not performed.
Direct Enter/Escape automation was inconclusive in the non-activating showcase
window; the existing Dialog keyboard handlers remain unchanged.

## Verification

Use `CARGO_TARGET_DIR=C:/Dev/aurora-gpui-build-g` for all Cargo commands.

- Native binary tests: 393 passed, 0 failed. Five new tests cover Apply action
  gating/activation, Applied matching, success/preview settlement, error
  settlement, and content identity across the native DTO boundary. Existing
  frozen-review and stale-result tests remain passing.
- Touched Rust files pass rustfmt and `git diff --check`.
- Debug and release builds passed (`cargo build` and `cargo build --release`,
  both using `--manifest-path native-gpui/Cargo.toml --locked -j 2`).
- No core files were changed, so the core test suite was not rerun separately.
- No Minecraft launch, Java process mutation, real credentials or real
  skin-change API calls were used. Windows were confined to the non-primary
  portrait display. No files were deleted from the tracked tree.

## Reproduce captures

`tools/skins-showcase/seed.py` creates an entirely new marker-gated data root,
four invented PNG skins and minimal launcher documents. It needs only Python's
standard library. `capture.ps1` checks for Java processes and requires a portrait
secondary display. It preserves disposable roots for inspection and supports
`-KeepOpen` for interaction checks.

Build a debug executable, then run in PowerShell:

```powershell
$env:AURORA_SHOWCASE_SKINS = 'library'
& tools/skins-showcase/capture.ps1 `
  -Exe C:/Dev/aurora-gpui-build-g/debug/aurora-launcher.exe `
  -Out C:/Temp/skins-captures -Pages 'library:7,library-lower:7@420'
```

States: `library`, `review`, `success`, `no-account`, `reauth`, `empty`, `manage`,
and `narrow`. Narrow uses the launcher's actual minimum size. In showcase mode,
account cosmetics and application use local fixtures; Apply never calls the
skin-change service, and other account mutations are suppressed. Production
continues through the existing typed backend APIs.
