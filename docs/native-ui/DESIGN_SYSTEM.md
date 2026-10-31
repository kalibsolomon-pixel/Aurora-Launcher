# Spxctra native design system (Aurora Launcher)

## NATIVE-8 amendments

The owner's NATIVE-8 brief explicitly authorizes subtle Borealis idle motion.
One pre-blurred contour image drifts at 2 Hz only while the visible foreground
window allows motion; Reduced Motion is static. Translucent tonal surfaces keep
text stable and legible. This supersedes the historical no-Borealis/no-custom-
title-bar/no-idle-motion statements below. Simple and OLED remain opaque and
static. Windows uses the confirmed Aurora identity and native drag/resize/window
controls behind a custom caption; other platforms retain their OS frame.

The compact icon rail retains accessible names and hover/focus tooltips. Instance
identity uses the existing persisted Material Symbols names, with search and a
full native path catalog. Home widgets and the custom accent picker expose
implemented services; they introduce no framework or runtime blur. See
[NATIVE_8_ACCEPTANCE](../../NATIVE_8_ACCEPTANCE.md) for native screenshots,
measured resources and verification limits. The refinement's retained raster
measurements are not end-to-end preview latency.

## Preview and layout refinement (2026-10-31 follow-up)

Home has a compact, bounded launch surface with a clear page heading and direct
routes to instance management and content. Discover and Instances retain their
headers while their lists scroll. Skins uses a stationary fitting room beside
an independently scrolling library: local skins are selectable rows, owned
capes are a responsive artwork gallery. Front/Back, outer layers and reset stay
beside the player. Selecting a cape shows the player's back immediately.

Shared tabs have one keyboard focus owner, repeated Left/Right navigation,
Home/End and a focus ring on the current tab. Selection remains accent text and
icons only. The existing palettes, type roles, radii and spacing remain in use;
there are no new effects, fonts, dependencies or background animations.

See `NATIVE_7_UI_REFINEMENT_ACCEPTANCE.md` at the repository root for the new
Windows screenshots, measured preview latency and remaining acceptance limits.

## NATIVE-7 amendments

Selected navigation, tabs, rows and popup options use accent text/icons only.
Selection backgrounds, tile borders, sidebar bars and tab underlines are removed;
hover surfaces remain. Shared `KeyboardFocus` never consumes pointer presses and
distinguishes pointer focus from keyboard focus rings. Switches/checkboxes retain
their functional checked-state geometry. Accent swatches use a check instead of
a selection outline. Shared button heights remain bounded in content rows.

Home contains only the instance dropdown, compact version/readiness information
and the existing Play pipeline. Accounts extend from the 88 px rail, without a
dark scrim, with a bounded scrolling account list, outside/Escape dismissal and
focus return. Skins groups a change-driven CPU player preview, local library and
account-owned capes; previewing and applying are separate actions. Branding uses
the owner-confirmed SVG, unchanged in silhouette. These amendments supersede
the historical selection treatment and listed visual gaps below. Desktop review
is incomplete; see the NATIVE-7 acceptance report, including the explicit stop of
desktop control.

Governing guidance: the installed `spxctra-ui` skill (1.0.0-draft) and the
2026-10-31 Spxctra refinement of the WebView launcher
([`docs/spxctra-ui/README.md`](../spxctra-ui/README.md)). The skill references
`references/REVIEW_CHECKLIST.md`, which is **not installed**; it was not
invented. The in-skill workflow and the owner brief were used instead.

## User jobs per screen

| Screen | Job (one sentence) | Primary action |
| --- | --- | --- |
| Home | Pick the instance to play and start it. | Play |
| Instances | See installations and create a new one. | New instance |
| Instance workspace | Configure one instance and manage its content. | Save (settings) / toggles (content) |
| Discover | Find content on Modrinth and install it into one instance. | Install |
| Settings | Change launcher-wide preferences. | — (immediate controls) |
| Accounts dialog | Choose or add the Microsoft account used to play. | Sign in with Microsoft |

## Tokens (`native/ui/theme.slint`)

Values mirror `src/app.css`; nothing new was invented.

* Surfaces: background, sidebar, surface, raised, hover, pressed, sunken,
  border, border-strong, text, text-secondary, text-muted — three dark themes
  (Aurora Dark, Midnight, OLED) set from Rust.
* Accent family: accent, strong, hover, pressed, contrast, soft, outline —
  always the Rust-derived `AccentPalette`, including validated custom accents.
* Status: success, warning, error, working (+ soft variants), stable across themes.
* Shape: 6 / 10 / 14 px radii. Spacing: 4, 8, 12, 16, 24, 32, 48 px.
* Type roles: page title 22, identity 16, section 15, body 14, secondary 13,
  metadata 12.5, label 12 (Segoe UI on Windows; system fallback elsewhere).
* Motion: 90 ms / 150 ms, ease-out, zero under Reduce motion.

## Components (`native/ui/components.slint`)

Button (primary, secondary, ghost, danger; icon; loading; compact),
IconButton (with hover tooltip and accessible name), Toggle, Checkbox,
Slider (arrow/Home/End keys, accessible value range), TextField (leading
icon, placeholder, invalid state, Escape/Enter), Dropdown (popup list,
arrow-key selection), Tabs (arrow-key navigation), Group, SettingRow,
Divider, StatusPill (tone + text, never color alone), ProgressBar
(determinate; indeterminate shows the track only), Spinner (static under
reduced motion), Dialog (scrim, Escape, focus on open), Toast, Glyph,
Avatar, EmptyState, PageTitle, Label, FocusRing.

Every interactive component defines hover, pressed, focus (2 px accent ring
outside the control), disabled and, where relevant, selected/checked/loading
states, is reachable by keyboard (Tab, Space/Enter, arrows where natural),
and exposes an accessible role, name and state.

## What was removed relative to the WebView design

* No glass, backdrop blur, glow, decorative gradients or group shadows.
* No custom title bar: the OS title bar is used (native window controls,
  snap layouts, accessibility).
* No Borealis video/animated background in the native preview (the
  preference is preserved and untouched).
* No hover-only information: tooltips repeat accessible names of icon buttons.
* The navigation rail is 88 px with persistent labels and the account at the
  bottom.

## Atmosphere

Restricted to the brand mark, the accent, and tonal separation between the
sidebar, canvas and surfaces. No celestial imagery, particles or idle motion.

## Accessibility verified

Through Windows UI Automation (the tree screen readers use): navigation tabs
with selected state, buttons with names, the switch exposing checked state,
combo boxes with values, a list of instances, text fields with labels, and
dialogs as named groups. Acceptance drove the application exclusively
through UIA Invoke patterns, which proves keyboard-independent operability.
Narrator itself, high-contrast mode, and 200 % text scaling were **not**
tested.

## Known visual gaps

Project artwork in Discover, instance icons, skins/cape preview and
Borealis are not implemented; content uses neutral letter glyphs.
