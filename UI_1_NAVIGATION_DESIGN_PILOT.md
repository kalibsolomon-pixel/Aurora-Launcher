# Aurora Launcher — UI-1 Navigation and Design-System Pilot

## A. Executive result

**PASS — pilot ready for owner visual review.** The implementation is limited to
the shared rail, Home card spacing/type and the existing Mods browser. All 272
frontend tests, type checking, production frontend build and affected browser
checks pass. Matched screenshots and geometry demonstrate aligned controls and
denser medium-width results with no measured horizontal overflow.

This is **representative browser fixture acceptance**, not native Tauri, live
Modrinth, authenticated gameplay or release acceptance. Native behavior and the
production installation were outside this visual pilot. No UI-2 work began.

## B. Repository baseline

- Branch: `codex/client-3-production-authority`, unchanged.
- Starting HEAD: `fa6a03a6e76e3f8f3d8fe2ea07ca788ce9b8bdbc`, matching the request.
- Ending application HEAD: `d5a707ddcc323ea2f7401e31b5d654254f422fe5`.
- Final documentation/evidence commit follows that application HEAD. Its exact
  ending SHA is reported in the delivery message and can also be resolved with
  `git log -1 --format=%H -- UI_1_NAVIGATION_DESIGN_PILOT.md` (a commit cannot
  embed its own hash in its contents).
- Initial tracked inventory: 901 paths, zero tracked modifications. Four
  developer-local artifacts were explicitly accepted by the owner and protected.
- Implementation commits, independently revertible:
  - `fb1fb95` — compact rail spacing and account helper wrapping.
  - `c783e14` — Home launch-card spacing and metadata tokens.
  - `d5a707d` — Mods browse control alignment/density and affected geometry test.
- No original tracked file deletions or renames. No original file is missing.
- All seven worktree paths/refs remain; the other six HEADs are unchanged.
- All four protected local files retain their original SHA-256 values and were
  never staged. Native configuration, manifests and lockfiles are byte-identical.

See [baseline inventory](docs/ui-1/baseline.json),
[preservation check](docs/ui-1/preservation.json) and
[complete changed-file list](docs/ui-1/changed-files.txt).

## C. Architecture inventory

The [pre-implementation audit and exact proposal](docs/ui-1/AUDIT_AND_PROPOSAL.md)
contains the required responsibility/style/inconsistency/improvement/risk table,
including typography, spacing, glass, accent, motion, responsive and widget systems.

| Boundary | Actual source and role |
|---|---|
| Application entry | `src/routes/+layout.svelte`, `+page.svelte`: appearance/startup and one typed SPA page switch |
| Shell/navigation | `src/lib/shell/AppShell.svelte`, `phase-f.css`; `src/lib/launcher/navigation.ts`, `navigation.svelte.ts`: icon rail, scrolling main, global/instance navigation |
| Home/card | `src/lib/pages/HomePage.svelte`; `src/lib/launcher/InstanceSwitcher.svelte`: Rust-owned readiness, Play, selected instance listbox |
| Account | `src/lib/shell/AccountDialog.svelte`; `src/lib/launcher/AccountIdentity.svelte`: sole bottom-left account entry and dialog |
| Mods | `src/lib/instances/InstanceWorkspace.svelte`, `InstanceModsPanel.svelte`, `ModrinthBrowse.svelte`: instance-local Installed/Browse, provider search/details/actions |
| Shared design | `src/app.css`, `src/lib/shell/phase-f.css`: semantic theme/type/spacing/control tokens and Borealis materials |
| Widgets/player | `src/lib/launcher/HomeWidgets.svelte`, `homeLayout.svelte.ts`, `widgets/*`, `PlayerPreview.svelte`: persisted customization and bounded skin rendering |

There is no global Discover route. Mods remain inside the existing workspace.
Legacy design-guide descriptions were checked against current source; no old
sidebar shortcuts, page labels or promotional structures were reintroduced.

## D. Baseline findings

1. Search input/button measured 38px versus 40px filters: an alignment mismatch.
2. Single-column results at 1024px measured 141.34px tall with actions occupying
   another row despite sufficient horizontal room: a density improvement.
3. Home version metadata used fixed 12px rather than the 0.8rem shared role.
4. Home mixed 10/14/22/28px spacing with the shared 4px scale: rhythm preference.
5. The 112px wide-window rail reserved extra space around 48px targets, while
   narrow windows already used 96px: density preference.

No baseline horizontal overflow was observed. The large Play/player composition,
icon-only navigation and content scrolling at minimum size are intentional.

## E. Design decisions

The rail now uses 96px at every supported size, preserving 48px targets (40px in
the existing short-window mode), focus tooltips, active marker, destination order,
Settings placement and bottom-left account entry. Home uses existing spacing and
metadata tokens; the hero, selector position, state hierarchy and player are intact.

Mods search and filters align at 40px at default text size and grow for larger
text. Only mod browsing receives the new treatment. Single-column results between
561 and 900px of **container width** put actions beside text; wider containers keep
two columns and smaller containers keep actions below. Existing title/secondary/
metadata roles clarify card hierarchy. Empty/loading states use an unfiltered backing.

Visual QA at 200% text size found fixed-height filters clipping their labels and
account helper text extending outside the dock. The final pilot lets filters grow,
expands/wraps their layout in em units and wraps account helper text. These are
acceptance-driven responsive corrections within the original scope. The audit
proposal predates these findings and is retained as the original proposal.

Borealis background, transparency, borders, shadows, accent derivation, selection
colors and motion values are unchanged. No new blur region, framework, font,
dependency, destination or widget was added. Play/account/install authority remains
Rust-owned; no command payload or backend behavior changed.

The attachment contained the written Note principles but **no Note screenshots**.
The influence is therefore limited to its described compact navigation, readable
density and aligned controls. No Note artwork, branding or exact styling is copied.

## F. Implementation summary

| Changed source | Responsibility |
|---|---|
| `src/lib/shell/phase-f.css` | Rail width/gaps/radius, padding and account helper wrapping |
| `src/lib/pages/HomePage.svelte` | CSS-only card padding/gaps, section spacing and rem metadata |
| `src/lib/instances/ModrinthBrowse.svelte` | Mod-only presentation class and scoped responsive CSS |
| `tools/visual/l1.test.mjs` | Detect actual rectangle overlap in both dimensions; assert pilot scope by content kind |
| `tools/visual/ui1-fixture.mjs` | Local synthetic search/artwork/install-error data; no native forwarding |
| `tools/visual/ui1-capture.mjs` | Matched pre/post screenshots and geometry records |
| `tools/visual/ui1.test.mjs` | Actual-component interaction, containment, text-scale and accessibility smoke checks |
| `LAUNCHER_DESIGN_LANGUAGE.md` | Link current UI-1 guidance and owner-review boundary |
| This report and `docs/ui-1/` | Audit, repository inventories, original screenshots, comparisons, measurements and verification outputs; exact paths in changed-files list |

No native files, production configuration, package metadata, lockfiles or release
infrastructure changed. The existing review entry is not part of production bundles.

## G. Visual acceptance

**Provenance:** actual Svelte components in the existing isolated review harness,
headless Chromium, DPR 1, 100% CSS scaling, reduced motion and the bundled Borealis
still. External browser requests were blocked for UI-1 captures/smoke checks.
Dimensions below are CSS application viewports; OS scaling and Windows frame size
are not native measurements. The Windows-only titlebar correctly does not mount
in browser preview. Workspace headers, actions and the entire app content viewport
are captured. The fixture retains its existing explicit synthetic-account label.

The fixture's older installed Aurora 2.1.5 pin is synthetic; it is not a claim
about the current production Client 3.0.0 authority, which was untouched.

| Screen/viewport | Before | After |
|---|---|---|
| Home 1440x900 | [before](docs/ui-1/before/home-1440x900.png) | [after](docs/ui-1/after/home-1440x900.png) |
| Home 1280x800 | [before](docs/ui-1/before/home-1280x800.png) | [after](docs/ui-1/after/home-1280x800.png) |
| Home 1024x768 | [before](docs/ui-1/before/home-1024x768.png) | [after](docs/ui-1/after/home-1024x768.png) |
| Home 720x520 | [before](docs/ui-1/before/home-720x520.png) | [after](docs/ui-1/after/home-720x520.png) |
| Mods 1440x900 | [before](docs/ui-1/before/mods-1440x900.png) | [after](docs/ui-1/after/mods-1440x900.png) |
| Mods 1280x800 | [before](docs/ui-1/before/mods-1280x800.png) | [after](docs/ui-1/after/mods-1280x800.png) |
| Mods 1024x768 | [before](docs/ui-1/before/mods-1024x768.png) | [after](docs/ui-1/after/mods-1024x768.png) |
| Mods 720x520 | [before](docs/ui-1/before/mods-720x520.png) | [after](docs/ui-1/after/mods-720x520.png) |

Side-by-side review sheets:
[Home](docs/ui-1/home-comparison.png), [Mods](docs/ui-1/mods-comparison.png),
[Settings / Instances / Skins & Capes](docs/ui-1/other-screens-comparison.png),
[empty/loading/error/long-name/200% text](docs/ui-1/states-review.png).
The comparison sheets are thumbnails; original captures above remain unmodified.

| Measurement | Before | After |
|---|---|---|
| Rail at 1440/1280/1024 | 112px | 96px |
| Rail at 720 | 96px | 96px |
| Mods search height at all four sizes | 38px | 40px |
| First Mods row at 1024 | 141.34px | 92.47px (34.6% shorter) |
| First Mods row at 1440/1280 | 141.34px | 140.47px |
| First Mods row at 720 | 141.34px | 140.47px |
| Home widget section top at 1440/1280/1024 | 738/644/612px | 726/632/600px |
| Home widget section top at 720 | 460px | 461px |
| Home Play height, desktop / narrow | 78 / 68px | 78 / 68px |
| Horizontal overflow, 8 pilot layouts | None | None |

Full geometry: [before](docs/ui-1/before/geometry.json),
[after](docs/ui-1/after/geometry.json). The 720x520 configuration stays unchanged;
its content scrolls vertically and player hides through the existing breakpoint.
Search/filter wrapping, long names/titles and 125%/200% root text size were checked.

All matched captures, state captures and unrelated-screen comparison sheets were
visually inspected. The unrelated interiors retain their structure/materials;
the expected rail change gives them 16px more width at larger sizes. Existing
resource-pack/shader/modpack checks pass and the pilot class is absent on them.
At 200%, the narrow account helper text wraps within the small dock; no text is
hidden. The minimum-height view necessarily requires scrolling to results/widgets.
Owner aesthetic approval remains pending; no physical GPU or native scaling claim.

## H. Automated verification

All results below are **new UI-1 executions**, separate from historical reports.
Browser commands use the existing external module override:
`AURORA_PLAYWRIGHT_MODULE=file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs`.
No dependency was installed. Start the local harness with
`node tools/visual/serve.mjs` before browser commands.

| Command | Final result | Exit |
|---|---|---|
| `npm run check` | 0 errors, 0 warnings | 0 |
| `npm test -- --test-concurrency=1 --test-timeout=60000` | 272/272 tests, 16 suites, no skips/failures/cancellations | 0 |
| `npm run build` | Static production frontend built successfully | 0 |
| `node tools/visual/l1.test.mjs` | 4 content kinds, each at 1600/1120/720 widths; long metadata, controls, categories, artwork and scope | 0 |
| `node tools/visual/l3.test.mjs` | 4 layout cases plus account/cape/widget interaction sequence | 0 |
| `node tools/visual/ui-corrections.test.mjs` | 6 reported groups, including 100-result content views, shared glass, reduced transparency and tooltip teardown | 0 |
| `node tools/visual/ui1.test.mjs` | 11 browser cases, zero page errors | 0 |
| `node tools/visual/ui1-capture.mjs before` | 11 pre-edit captures, zero page errors | 0 |
| `node tools/visual/ui1-capture.mjs after` | 11 final captures, zero page errors | 0 |
| `git diff --check` and staged equivalent | Whitespace checks pass | 0 |

Exact outputs and exit summaries are under `docs/ui-1/` (`frontend-exits.json`,
`browser-exits.json`, `frontend-tests.txt`, `type-check.txt`, `production-build.txt`,
and each browser script's `.txt` output). No formatter/linter is configured.
No Rust suite or Tauri build was run: no native-boundary/startup change occurred.
The package's existing version-consistency prehooks ran, without changing a version
or performing release activity.

**Failed/interrupted attempts:** the initial unrestricted `npm test` reported
three appearance setup failures and remained stalled with outstanding Vite test
workers; that run ended with exit 1 after terminating only verified run-owned
PID 7428 and its remaining children 25812/25024/18216. Exact process provenance
is retained in `stalled-test-processes.json`. Serial execution subsequently passed
all 272 tests twice; the concurrent setup cause was not diagnosed or changed.
The initial L1 run exited 1 because its vertical-only overlap assertion rejected
intentional inline actions; it now checks rectangle intersection in both axes.
Early new-harness attempts also exited 1 for a wrong fixture request field,
incorrect Settings selector/ambiguous Home button and an incorrect expected
provider-error string. Those test-only defects were corrected before final captures.

## I. Functional acceptance

Actual component interactions, all using safe fixtures:

- Sidebar focus/hover tooltips, active marker and Settings/Home navigation.
- Home Ready/Play enabled and blocked/Play disabled states; long instance names.
- Instance listbox keyboard opening, End/Enter selection of Vanilla, Home/Enter
  restoration of Aurora, Escape and contained popup. No game was launched.
- Bottom-left account dialog at all four sizes, viewport containment, Escape and
  focus return; existing L3 sequence additionally switches synthetic accounts.
- Widget size and keyboard order changes saved to tab session storage, retained
  through reload; Playtime/Recent Servers load. Native widget storage was not touched.
- Mods search submission, categories, sort and pagination (20 -> 24 results),
  verified request fields including instance/content identity. Installed controls
  remain disabled as before; loading, empty, errors and synthetic install-failure
  notice are visible. No real acquisition or installation occurs.
- Existing Installed mods, provider content kinds, artwork fallbacks/retries,
  shared materials, cosmetic controls and account isolation regression checks pass.

Live provider behavior, real native persistence and gameplay were not exercised
by this CSS/presentation pilot; existing Rust authority was preserved.

## J. Performance and accessibility

No new animation, subscription, image acquisition path or per-result backdrop
filter. The existing 100-result regression verifies unfiltered rows, artwork reuse,
one shared workspace filter and reduced-transparency fallback. No FPS/physical
presentation claim is inferred from these checks.

Keyboard controls, focus outlines/tooltips, input labels, semantic buttons,
aria-current/pressed/selected, disabled states and alert/status feedback remain.
UI-1 tests verify label-driven interactions, dialog focus return, no horizontal
overflow, growing controls and readable long content at 125%/200% text size.
Mods metadata on the Simple result backing measures **5.37:1** contrast. Borealis
readability was visually inspected on the fixed background; dynamic-background
contrast and full screen-reader/OS high-contrast certification are not claimed.
Reduced-motion transition removal and existing reduced-transparency behavior pass.

## K. Preservation

Production Launcher 1.4.2, owner instances/worlds/saves/mods, authentication,
application data, user configuration and existing release artifacts were not
accessed for mutation. Only synthetic browser state and generated build outputs
were used. No owner account sign-in, native instance mutation or installation.
All original tracked paths and protected file hashes remain; no other worktree
was modified. No cleanup/reset/merge/push/tag/publication/signing/version bump.
Process termination was limited to the verified stalled test run and this run's
own review server after verification. No unrelated process was terminated.

## L. Recommendation

**UI-1 PASS — pilot ready for owner visual review.** Review the matched Home/Mods
screens and minimum-size/text-scale states. Keep any broader navigation, Discover,
content browsing or release work in separately authorized phases. Do not
automatically proceed to UI-2.
