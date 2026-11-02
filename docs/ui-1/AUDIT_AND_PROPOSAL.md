# UI-1 audit and proposal — completed before application edits

Baseline: `fa6a03a6e76e3f8f3d8fe2ea07ca788ce9b8bdbc`, branch
`codex/client-3-production-authority`, 901 tracked files, no tracked edits.
The owner confirmed the four local artifacts listed in `baseline.json`; their
hashes and all seven worktree identities are recorded there. No relevant launcher,
game, Cargo or Vite development process was present before this run. Historical
acceptance paths are inventoried separately from newly generated UI-1 evidence.

Read: root AGENTS.md; ARCHITECTURE.md frontend/native, navigation, appearance,
Home and local content sections; docs/development/ui/LAUNCHER_DESIGN_LANGUAGE.md including its Phase F
supersession; docs/development/phases/PHASE_F_DESIGN.md and docs/development/phases/PHASE_F_ACCEPTANCE.md; current L3 architecture
and acceptance; docs/development/ui/LAUNCHER_PRERELEASE_UI_CORRECTIONS.md (including the per-result
blur correction); docs/troubleshooting/RR1_A_NATIVE_STARTUP_ACCEPTANCE.md; and the Client 3 production
correction. Source is authoritative where historical descriptions differ (for
example the current Home cap is 1280px and the rail has no instance shortcuts).

## Architecture inventory

Paths below are repository-relative.

| Component/subsystem | Source | Responsibility/shared styling | Observed inconsistency | Pilot improvement | Risk |
|---|---|---|---|---|---|
| SPA entry | `src/routes/+layout.svelte`, `+page.svelte` | Appearance load, single typed page switch, native store initialization | None requiring change | Preserve | High: startup authority |
| Shell | `src/lib/shell/AppShell.svelte`, `phase-f.css` | Background/titlebar, rail, independently scrolling main; f-* tokens | Rail 112px around 48px targets (96px below 800px) | Uniform 96px rail, preserve main gutters | Medium: every page gains 16px at larger widths |
| Navigation | `src/lib/launcher/navigation.ts`, `navigation.svelte.ts`; `AppShell.svelte` | Home, Instances, Skins & Capes, Settings, dev-only Developer; aria-current, tooltips | 12px gaps and 15px radius differ from nearby control rhythm | 8px gaps, existing 14px control radius; retain indicator/focus | Low/medium: global visual effect |
| Window chrome | `src/lib/shell/TitleBar.svelte` | Native Windows-only 40px titlebar; native decorations elsewhere | Browser harness correctly omits native controls | Preserve; disclose preview boundary | High: native boot |
| Home | `src/lib/pages/HomePage.svelte` | Logo/launch left, player right, widgets below; Rust-derived launch state | Literal 22/10/14/28px spacing; version line fixed 12px | Existing 4px scale; rem metadata; modest bottom-space reduction | Low: CSS only |
| Instance card/selector | `HomePage.svelte`, `src/lib/launcher/InstanceSwitcher.svelte` | Play, selector, installed version, readiness; bounded listbox | Card internals use inconsistent indents | Align card padding only; preserve information order and selector | Medium: long names/popup containment |
| Account dock/dialog | `src/lib/launcher/AccountIdentity.svelte`, `src/lib/shell/AccountDialog.svelte` | Single bottom-left entry, avatar/name, dialog account actions | Dock width follows rail | Preserve markup/behavior; inspect narrower rail | Medium: names and dialog reachability |
| Home widgets | `src/lib/launcher/HomeWidgets.svelte`, `homeLayout.svelte.ts`, `widgets/*` | Persisted order/visibility/sizes, drag and keyboard edits | Existing 16px grid is coherent | No edits; regression-test persistence/resize | High: persistence |
| Player | `src/lib/launcher/PlayerPreview.svelte`, `playerModel.ts` | Bounded software rendering; controls below; hidden <=900px | Large vertical footprint intentional | Preserve scale, placement and rendering | Medium/performance |
| Workspace | `src/lib/instances/InstanceWorkspace.svelte` | Header, readiness, contextual Play, wrapping tab row | No clipping at requested sizes | Preserve page interior | High: context/selection |
| Mods host | `src/lib/instances/InstanceModsPanel.svelte` | Installed/Browse, authoritative local state/provider context | None requiring logic change | Preserve | High: mutation authority |
| Modrinth browser | `src/lib/instances/ModrinthBrowse.svelte` | Search, categories/sort, pagination, details and reviewed/quick install | 38px search vs 40px filters; 141px single-column cards despite room for inline actions | Mod-only 40px controls; inline actions on medium single-column results, compact gaps, clearer text | Medium: shared across content types; explicitly scope changes to mods |
| Shared cards/buttons | `src/app.css`, `src/lib/shell/phase-f.css` | Group surfaces, btn, status, field labels; pilot controls | Layers intentionally differ: legacy vs f-* | No broad consolidation; reuse pilot radius | High for global changes |
| Typography/spacing | `src/app.css` | rem roles identity/page/section/body/secondary/metadata/label; 4/8/12/16/24/32/48 scale | Home versions bypass rem metadata | Use existing tokens, no global type changes | Low |
| Surfaces/borders/glass | `src/app.css`, `phase-f.css` | Theme palettes; f-panel/edge; workspace background diffusion, unfiltered rows | Differences intentional after scrolling correction | Preserve every glass material and filter count | High/performance |
| Accents | `appearance.svelte.ts`, native `appearance` | Root derived palette, presets/custom; focus and actions | No defect found | Preserve all values and authority | High |
| Motion/states | `src/app.css`, `phase-f.css`, `Background.svelte` | 90/150ms legacy, 120/180ms pilot; focus outlines, hover/press, reduced motion | Different durations intentional | Reuse existing; no new animation | Medium |
| Responsive | Page/component CSS, `src-tauri/tauri.conf.json` | Desktop 720x520 minimum; 800 shell, 900 Home/player, 900 browse container | Single-column results waste vertical space at 1024 | Inline result actions above 560px container; preserve stacked actions below | Medium |

Duplicate input, card and button declarations exist across installed mods,
Modrinth and launcher settings. They encode different scopes/materials; this
pilot does not consolidate them or introduce a generic card/control framework.

## Baseline visual acceptance and five findings

`before/` contains actual pre-edit component screenshots and geometry at
1440x900, 1280x800, 1024x768 and the configured minimum 720x520. Settings,
Instances and Skins & Capes are also captured at 1280x800 for shared-shell review.
These are **browser fixtures**, not native acceptance. Chromium DPR=1, 100% CSS
scaling, reduced motion, bundled still, no external requests. The Windows-only
native titlebar is absent; screen dimensions are CSS app viewports, not Windows
outer-window sizes. Header/actions/rail/account are real mounted components.

1. **Measured mismatch:** search input/button 38px, filters 40px. Align to 40px
   for the Mods pilot; focus outlines and labels already exist.
2. **Layout inefficiency:** at 1024px, single-column result cards are ~141px tall
   with actions below short text. There is enough width for inline actions.
3. **Type inconsistency:** Home installed versions use fixed 12px while shared
   metadata is 0.8rem (12.8px at default scale). Use metadata role.
4. **Rhythm preference:** Home mixes 10/14/22/28px with the shared 4px scale.
   Regularize card/section spacing, without changing the composition.
5. **Density preference:** rail reserves 112px at larger widths versus 96px at
   narrow widths around the same 48px target. Use 96px consistently, retaining
   bottom-left account, all labels/tooltips and non-color active marker.

No baseline horizontal overflow was measured. Vertical content scrolling at
720x520 is expected, not clipping. Icon-only navigation, large Home Play/player,
and tinted workspace selection are established decisions, not defects. All
five findings preserve that distinction; no claim of a launcher-wide visual defect.

## Exact implementation proposal (authorized pilot)

- Shared shell: only `src/lib/shell/phase-f.css`: `--f-rail-width` 112 -> 96px,
  rail gap `--space-2`, rail radius `--f-radius-control`, sidebar inline padding
  `--space-2` above the existing narrow breakpoint. No global gutter/type/color
  token changes and no destination or account movement.
- Home: only `src/lib/pages/HomePage.svelte` CSS: composition padding
  `--space-2` / `--space-4`, launch column gap `--space-5`, card padding
  `--space-4`, card gap `--space-3`, heading inset on shared scale, versions
  `--text-metadata` with `--space-2` margins. Preserve logo, 78/68px Play,
  selector location, launch states, player, widgets and width cap.
- Mods: only `src/lib/instances/ModrinthBrowse.svelte`: presentation class tied
  to `browseKind === 'mod'`; scoped 40px controls using the existing pilot radius;
  title/body/metadata roles; compact 8px list gap and 12px card padding;
  inline glyph/copy/actions in single-column containers 561..900px, retain
  two-column cards above 900px and wrapped actions below 561px; modest empty
  and loading-state backing. No query/provider/backend changes.
- Unchanged: resource packs, shaders, modpacks, every native file/DTO, production
  configuration, accent/material values, dependencies, release state, Settings
  and Cosmetics interiors. Shared-shell geometry is the sole expected change
  on unrelated screens.
- Verify: repeat exact captures, measure row/control/rail geometry and horizontal
  containment, inspect all screenshots, compare unrelated interiors; check
  keyboard/focus/hover, long titles/names, 125% and 200% text scaling, reduced
  motion, dialogs, empty/loading/error states, selection and widget persistence,
  search/filter/pagination requests, installed state, account and Settings entry.
  Use synthetic state only; no real install/launch/account mutation. Run npm
  check/test/build and existing affected browser regression scripts. No configured
  formatter/linter exists. Rust checks unnecessary for CSS/presentation work.

The attachment includes only the written Note principles, not the referenced
screenshots. The proposal uses compact navigation, aligned controls and readable
density from that description. No Note colors, artwork, branding or layout are copied.
