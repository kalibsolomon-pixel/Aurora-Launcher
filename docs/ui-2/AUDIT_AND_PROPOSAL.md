# UI-2 Home audit and proposal

Recorded before application edits. Baseline branch `codex/client-3-production-authority`,
HEAD `42e323b00c8ea8b744b3714952b9b09b66d834d8`, 956 tracked paths, seven worktrees,
no tracked edits. The latest four commits are the approved UI-1 implementation
and report. `baseline.json` records paths, local-file hashes, processes and evidence.
The only observed relevant processes are existing Codex Node/WebView helpers;
no game, Cargo, review server or launcher process was present before this task.

Read AGENTS.md; UI_1_NAVIGATION_DESIGN_PILOT.md and its audit/proposal;
ARCHITECTURE.md current Home/widgets, frontend/native, navigation, appearance,
launch and Phase D sections; PHASE_F_DESIGN.md; LAUNCHER_DESIGN_LANGUAGE.md current
composition and token/accessibility sections; RR1_A_NATIVE_STARTUP_ACCEPTANCE.md;
and tools/visual/README.md. Historical statements defer to implemented source.
Inspected UI-1's Home comparison and actual new UI-2 baseline render.
The referenced Note screenshots were not attached; use the supplied hierarchy
principles, preserving Aurora materials and branding.

## Actual architecture and ownership

| Concern | Source and authority | Scope |
| --- | --- | --- |
| Home composition | `src/lib/pages/HomePage.svelte` | Home only |
| Selection, metadata, installation | `launcher/store.svelte.ts`: selectedInstance from Rust launcherState/config/instances; `instanceStatus.ts`: installedConfigurationLabel | Shared; preserve |
| Selector | `launcher/InstanceSwitcher.svelte`: runSelect, keyboard listbox | Shared; Home wrapper styles only |
| Play, checking, starting/running, blockers/failure | `launcher/home.ts`: homeLaunchState; Home filters instance/account IDs; store playReadiness/playProcess/playBusy, runPlay and refreshPlayReadiness | Native decision and supervisor remain authoritative |
| Instance management | `navigation.svelte.ts`: openInstance with existing Settings/Overview tabs | Existing workspace destination |
| Account | `launcher/accountManager.svelte.ts`, store accountsState/selectedAccount; shell AppShell/AccountDialog; AccountIdentity | Shared bottom-left dock; preserve |
| Player skin | `launcher/PlayerPreview.svelte`: account-keyed accountAvatars, playerModel/playerPixels/skinTextures | Home-only preview; existing renderer, angle and manual controls |
| Preview pose/lifetime | playerModel constants/renderPlayer; PlayerPreview yaw, visibility matchMedia and document visibility | No continuous loop; adjust layout/visibility threshold only |
| Widget identity/sizes/order | `launcher/widgets.ts` catalog and pure placement functions; `HomeWidgets.svelte` presentation | Keep two logical columns, small one/wide+large two, narrow clamp |
| Widget persistence/editing | `homeLayout.svelte.ts`, typed backend get/save/resetHomeWidgets; Rust home_widgets/config | Ordered IDs/enabled/size, no schema or store changes |
| Widget content | `launcher/widgets/*` eight registered widgets | Reuse all content and data sources |
| Playtime | PlaytimeWidget -> getPlaytimeSummary/getDailyPlaytime; native gameplay_history | Loading/error/zero/valid remain distinct |
| Recent servers/worlds | RecentTargetsWidget -> getRecentServers/getRecentWorlds; Rust gameplay_history/server_enrichment | History and optional server enrichment separate; no bridge rewrite |
| Discord | launcher/discord.svelte.ts, DiscordSettings, native discord worker | Settings-only; no Home indicator currently |
| Responsive shell/materials | AppShell, phase-f.css, app.css | UI-1 96px rail, gutters, tokens and Mods unchanged |

## Five highest-impact findings

1. Play precedes identity and readiness, weakening the requested identity-first hierarchy.
2. Logo/player sizing creates a 686px primary composition at 1440x900 and widgets
   start at y=726 (y=632 at 1280x800). Actual widget content falls below the fold.
3. Two equal columns give the player the same width as the functional card, even
   though the model only occupies part of its region; the card can use more width.
4. A single small widget occupies only half a row; below 900px all widgets stack
   even when two readable columns could fit. A blank section heading makes the
   pencil's purpose less discoverable.
5. Home offers no direct instance management entry, and hides the player at 900px
   despite usable horizontal room just below that threshold.

## Proposed implementation

- Home: retain 1280px centered cap, UI-1 shell/gutters/typography/materials.
  Give the launch column slightly more width than the external preview. Reduce
  branding to 112–144px and player to a bounded 280–420px region. Identity and
  readiness precede Play; concise installed versions and an existing Manage
  instance destination follow it. All state filtering/commands remain exact.
- Play: keep the existing Borealis accent family, hover/press/focus and disabled
  guards. Use rem typography, generous height and existing state labels.
- Preview: same canvas, validated account-keyed pixels and pose/manual controls;
  no new dependency, skin request, customization or animation. Visible >800px;
  hide <=800px to reserve card width at the 720px minimum. Missing skin retains
  existing quiet empty fallback. Do not create an account row or decorative card.
- Widgets: add a restrained section heading next to the existing pencil/Done.
  Reuse surface, border, radius, rem and 4/8/12/16/24/32px tokens. Keep DOM/saved
  order and small/wide/large semantics. A sole widget fills its row; below 640px
  widget-container width, all spans clamp to one. No dense auto-placement,
  persistence migration or reset. Editor remains existing edit mode.
- Leave navigation, account dock, Mods, global styling, native state/backend,
  authentication, release/source authority and configuration unchanged.

## Acceptance plan

`before/` has 15 matching browser fixture captures: four desktop/minimum sizes,
sparse/dense/mixed sizes, long name, blocked/running, missing skin, valid/empty/
unavailable/loading history and OLED/Simple. Provenance: actual components with
synthetic DTOs, local-only network, DPR 1, 100% CSS scale, reduced motion. Geometry
is in `before/geometry.json`. No owner state is used to produce coverage.

Repeat after; measure containment/order/overlap and visually inspect rendered
screens. Exercise selection, unchanged Play action/guards, all statuses, keyboard
focus, account isolation, widget editing/reload, large text, accent/material modes,
and existing UI-1/L1/L3/correction tests. Run frontend type check, tests, build.
Build/boot a native debug application with isolated managed and WebView data;
observe window resizing, Home, fallback, widget edits/restart and clean exit.
No game install/launch, owner credentials, production installation or release.
