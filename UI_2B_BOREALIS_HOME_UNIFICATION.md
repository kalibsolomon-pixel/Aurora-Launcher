# Aurora Launcher — UI-2B Borealis Home visual unification

## A. Verdict

**PARTIAL — visual implementation ready for owner review; mandatory native
medium/minimum size acceptance remains incomplete.** The shared hero, instance
hierarchy, player grounding, local background contrast, widget material and rail
boundary are implemented. All automated checks pass. Actual isolated native
startup, default/maximized Home, cyan appearance, navigation, widget editing and
clean exit passed. Native resize attempts did not produce measured medium or
minimum dimensions, so those requirements are explicitly unverified.

## B. Repository baseline

Branch `codex/client-3-production-authority`; starting HEAD
`a3154bdc32e1daba961c827c12148dd8f78d4eba`. The request named `bfa44d5`; the
three intervening commits were the owner's card, split-control and reference
proportion refinements, inspected and reported before implementation.

1195 original tracked paths, seven worktrees, zero tracked modifications and only
the four expected developer-local artifacts untracked. Full inventory, history,
critical manifest hashes, protected hashes, worktrees, process inventory and
existing acceptance paths are in [baseline.json](docs/ui-2b/baseline.json).

Local independently revertible implementation commits:

- `01ec2a4`: shared instance/player hero and hierarchy.
- `a6b1b89`: localized Borealis contrast and Home materials.
- `4eefea2`: widget spacing/type and faint rail boundary.
- `0d10048`: viewport-aware selector placement for enlarged text.
- `2c2587d`: flat full-width blockers and absent-player explanation, following
  actual native pilot inspection; final implementation HEAD.

The subsequent tests/documentation/evidence commit is the delivery HEAD. Its SHA
is reported in the delivery message; resolve it with
`git log -1 --format=%H -- UI_2B_BOREALIS_HOME_UNIFICATION.md`.
Complete changed paths: [changed-files.txt](docs/ui-2b/changed-files.txt).
Original paths, protected hashes, critical hashes, all worktree identities and
the other six worktree HEADs remain intact; zero tracked deletions.

## C. Visual diagnosis

[Audit and proposal](docs/ui-2b/AUDIT_AND_PROPOSAL.md) records actual components,
tokens, responsibilities, measurements, risks and the single proposed direction.

UI-2's compact action floated in a broad open region opposite a large player.
Bright curtains competed with both. The control lacked a full-name context and
version line; widgets switched abruptly to darker standalone surfaces; the rail
had little separation. These were composition judgments, not baseline overflow
or backend defects. Existing sources remained authoritative throughout.

The Note screenshot in the conversation and current Aurora evidence were
inspected. No Lunar screenshot was available; its supplied written principles
were used without claiming artwork inspection. No third-party assets were fetched.

## D. Design decisions

Home uses a shared two-region hero. The left region supplies a bounded full
instance heading, truthful installed Minecraft/platform/Aurora details, the
existing 320×80 split Launch control, and quiet existing Settings access. Its
compact name still identifies the selector; the heading gives long names more
context. Readiness remains the accessible inline dot, with the same native
decision and explicit messages. Blockers span a flat bottom row of the hero,
avoiding a nested diagnostic card. Starting/Running and failed/exited messages
retain the original state projection.

The player remains a separate sibling component, outside the instance card and
launch column. Its raster stage is bounded to 240–320px, with static accent-derived
illumination and a soft floor only when a validated skin exists. Rotation/reset,
account-keyed input, visibility policy and renderer lifecycle are unchanged.
Absent skin shows quiet explanatory text without a generic player. The existing
fallback accessible loading label is retained; no native authenticated skin is
claimed for this account-free acceptance.

Static Home-only CSS overlays quiet the same bundled Borealis image/video.
Home locally reuses the established panel/edge/shadow tokens with a darker backing;
no new theme preference, image, background loader or animation is introduced.
Hero and widgets share those materials and outer alignment, with clearer section
spacing. Widgets receive no enclosing decorative dashboard card. The rail gains
only a low-opacity backing and hairline boundary over Borealis. Logo, destinations,
targets, tooltips and bottom account entry remain intact. Other page interiors
and the global background lifecycle are unchanged.

At <=800px the player still yields, Home stacks and saved widget spans clamp to
one column. Ordinary vertical scrolling accommodates long text, multiple blockers
and dense widgets. The configured minimum size remains unchanged. The selector
now opens above or below based on available viewport space, and clamps its height;
it reads geometry on opening/resizing only.

## E. Before-and-after evidence

- [Matched Home comparison](docs/ui-2b/home-comparison.png).
- [Matched state/appearance comparison](docs/ui-2b/states-comparison.png).
- Original [before geometry](docs/ui-2b/before/geometry.json) and
  [after geometry](docs/ui-2b/after/geometry.json), with captures in those folders.
- [Geometry comparison](docs/ui-2b/geometry-comparison.json).
- [Actual native review sheet](docs/ui-2b/native-review.png), native client PNGs,
  OS-window JPGs and measurements under [native-final](docs/ui-2b/native-final/start.json).
- Sequential hero/background pilot captures remain in `docs/ui-2b/hero/` and
  `background/`; the first native pilot remains under `native/`.

Browser captures use real components, deterministic synthetic DTOs, DPR 1,
100% CSS scaling, reduced motion and blocked external requests, without native
titlebar. Before/after states include Ready, Running, blocked, populated/sparse/
dense widgets, missing skin, long names, loading/empty/error, OLED/Simple and cyan.
No fabricated fixture data enters native readiness or production.

| CSS viewport | Hero height before → after | First widget y before → after |
| --- | --- | --- |
| 1440×900 | 502 → 436px | 594 → 540px |
| 1280×800 | 458 → 404px | 550 → 508px |
| 1024×768 | 444 → 392px | 536 → 496px |
| 720×520 | 96 → 295px | 180 → 391px |

Desktop empty space and scrolling decrease. The minimum layout intentionally
uses more height to retain the new instance context; this is a tradeoff, not a
claim that widgets move earlier at every size. No horizontal overflow was measured.

Screenshot inspection and geometry support these evaluations:

| Required design question | Evaluation |
| --- | --- |
| One unified interface? | Yes: shared hero and widget alignment/material establish one workspace. |
| Cohesive Launch region? | Yes: heading, metadata and action now form a readable group. |
| Integrated player? | Yes in skin fixtures: bounded scale, local lighting and floor ground it. Native verified only the absent-skin fallback. |
| Borealis distinctive? | Yes: curtains remain visible inside the hero and around its edges. |
| Background less distracting? | Yes: backing is darkest behind the instance text; contrast samples support readability. |
| Connected widgets? | Yes: same backing/edge and consistent 24px rhythm, without another wrapper. |
| Clearer hierarchy? | Yes: full identity precedes subordinate versions and the primary action. |
| Small-size legibility? | Browser checks pass including 200% text; native minimum remains unverified. |
| Unnecessary decorative surfaces? | No additional nested cards; one hero foundation, existing widget surfaces, static player lighting. |
| Recognizably Aurora? | Yes: original artwork/logo, configured accents and established controls remain. |

## F. Automated verification

All final commands exited 0. Logs and exit records are under `docs/ui-2b/`.

| Command | Result |
| --- | --- |
| `node --test --test-concurrency=1 --test-timeout=60000 "src/lib/**/*.test.ts"` | 272 tests, 16 suites, all pass. |
| Same runner for `productSurfaces.test.ts` and `releasePolish.test.ts` after native-driven corrections | 53/53 pass. |
| `npm run check` after final corrections | 0 errors, 0 warnings. |
| `npm run tauri build -- --debug --no-bundle` | Production static frontend and debug native executable built; no installer. |
| `node tools/visual/ui2b.test.mjs` | 6/6 new browser groups. |
| `node tools/visual/ui2.test.mjs` | 17/17 Home regression cases. |
| `node tools/visual/ui2-ui1-regression.mjs` | 11/11 approved UI-1 browser cases. |
| `node tools/visual/ui2b-capture.mjs before/after` | Four matched sizes plus state/appearance captures; no page errors. |
| `node tools/visual/ui2b-preserve.mjs`, Git checks | Paths, hashes, other worktrees/HEADs preserved; no deletions or whitespace errors. |

Widget ordering/size/hide/add, persistence through reload and unknown/disabled
slot retention are included in the frontend and browser suites. Final 34 browser
groups report zero page errors. Existing UI-1 Mods contrast remains 5.37:1.
No new Rust format/check/test pass is claimed; no Rust source or native contract
changed. The native build repeats the pre-existing STATIC_VCRUNTIME warning.

Early harness errors used an incorrect Settings selector and a plural locator
where one element was expected; logs are retained. The Home regression caught a
real 200% dropdown overflow, corrected with viewport-aware placement. A CSS
compatibility warning was fixed by supplying the standard line-clamp property.
The first native inspection caught the tall nested blocker card; the final flat
presentation was rebuilt and rechecked. Final outputs are `*-final*` or
`*-corrected*`, with authoritative exit records `browser-final-exits.json` and
`native-correction-exits.json`.

## G. Native acceptance

Fresh exclusive direct-temp roots and a separate WebView2 profile were created
by `tools/visual/ui2b-native.ps1`. The unchanged debug-only managed-data override
redirected all launcher persistence. The compiled frontend used actual Rust
commands and a synthetic Installing Vanilla registry, no account, no game or
runtime installation, and no fabricated Ready result. Executable hash, exact
root and process IDs are recorded in each start.json.

Computer Use inspected the real window; CDP read-only measurement/screenshots
captured the actual `http://tauri.localhost/` WebView, not the browser review entry.
OS display scaling, WebView DPR and native scale factor were **1.25 / 125%**.
Default client measured **1120×790 CSS px**; maximized **2048×1152**. Native chrome
affects actual client height, so nominal configuration dimensions are not relabeled
as measured dimensions. No horizontal overflow was measured.

Native Home, real blockers/disabled Launch, absent-player explanation, widget
layout, scrolling, Settings/Home navigation, persisted Borealis/cyan appearance,
widget editor entry/Done and account dock placement were inspected. Default,
maximized, cyan and widget-editor screenshots are retained. Both isolated runs
exited normally with code 0; the final process was 12596. Expected missing native
inventory/runtime diagnostics from the incomplete fixture remain in stderr.txt.

**Incomplete requirement:** boundary drags and Windows Size-menu attempts did
not yield a verified medium or minimum client. The file `medium.json` truthfully
records 1120×790, the unchanged size after an attempt; it is not a passing medium
capture. No minimum native screenshot exists for this pass. Maximize/restore
did change actual native dimensions, but does not substitute for the missing
size checks. This is an automation/acceptance limitation, not evidence that the
application cannot be resized. Startup configuration/capabilities were not changed
to bypass it. Authenticated native player rendering and real game launch were
also outside this account-free presentation run.

## H. Functional preservation

Launch still sends the same instance/account IDs through the existing command;
three immediate clicks produce one fixture request. Native readiness/account
ownership guards, process supervision and state transitions are unchanged.
Selector keyboard/type-ahead, focus return and long-name handling remain. Player
manual rotation changes pixels and Reset restores them; account switching removes
the previous model. All eight widget registrations, saved order/size/enabled state,
schema and acquisition paths remain unchanged. Navigation and sole account entry
remain in the shell. No new routes, widgets, dependencies or cosmetics behavior.

## I. Accessibility and performance

Browser checks verify semantic independent buttons/listbox, keyboard focus,
disabled/hover/pressed states, accessible readiness names/tooltips, error text,
long names, 200% text, reduced motion/transparency and appearance compatibility.
Sampled hero secondary-text contrast against nearby screenshot background pixels
is **9.75–10.49:1**: [samples](docs/ui-2b/contrast-samples.json). This is a sampled
reduced-motion capture, not a whole-app WCAG audit or a guarantee about every frame.
OLED/Simple and alternate cyan Launch contrast pass existing checks.

Home's foundation and widgets add no backdrop filters; inherited Home blur is
none. The player control bar's redundant filter is removed. Gradients are static;
no new image load, rendering dependency, timer, idle player loop or reactive
subscription. Existing Borealis playback/focus/minimize/reduced-motion behavior
is preserved. No new performance profiling benchmark is claimed.

## J. Preservation

Production Launcher 1.4.2 was neither installed over nor changed. No owner app
data, instances, worlds, mods, credentials or account records were read or mutated
for acceptance. Only fresh marked temporary data/profile roots were used and
retained. Protected files were never staged, moved or cleaned. All seven worktrees
and other six HEADs remain. Previous UI-1/UI-2 evidence remains intact. Critical
manifests/lockfiles/native configuration hashes are unchanged.
No push, merge, tag, publication, deployment, release/version changes or UI-3/UI-4.

## K. Remaining concerns

- Acceptance blocker: measured native medium/minimum screenshots and layout
  confirmation remain required before a full PASS.
- Visual tradeoff: minimum browser layout shows less of the first widget without
  scrolling, because the full instance context is restored.
- Minor/accessibility carryover: the existing absent-player accessible image label
  says “Player skin loading”; visible fallback copy now explains sign-in/availability.
- Pre-existing fixture integration: absent game/runtime/mod directories produce
  truthful blockers and unavailable counts, not visual-phase repairs.
- Authenticated native skin and real game launch are not claimed. No new UI-3/UI-4
  features were attempted.

## L. Recommendation

**UI-2B PARTIAL — corrective visual pass required:** complete the native medium/
minimum acceptance checks, then obtain owner visual approval of the supplied
comparison. The implemented composition materially improves cohesion; do not
start another UI phase or prepare a release.
