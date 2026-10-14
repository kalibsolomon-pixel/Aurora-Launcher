# UI-3 — Discover and Library acceptance

## A. Verdict

**PASS — UI-3 ready for owner visual review.** The requested frontend refinement
is implemented and verified. Existing native installation/provider authority is
preserved. Automated checks and actual isolated Windows presentation acceptance
pass. This is not a release or an authenticated-gameplay acceptance.

## B. Repository baseline

| Item | Result |
| --- | --- |
| Branch | `codex/client-3-production-authority`, unchanged |
| Starting HEAD | `7c76626464a8e0c1c581ddbf976cfb5b6c71e6f3`, exactly the request |
| Intervening owner commits | None |
| Ending application HEAD | `fd7e48b0a0d00541f059ffa23147fc90ce847a6c` |
| Ending verified code/test HEAD | `3b41535` |
| Final delivery HEAD | The documentation/evidence commit containing this report; exact SHA in the delivery message, also `git log -1 --format=%H -- UI_3_DISCOVER_LIBRARY_ACCEPTANCE.md`. A commit cannot embed its own hash. |
| Original tracked inventory | 1,464 paths, all still present/tracked |
| Initial tracked changes | None |
| Worktrees | All seven retained; other six HEADs/identities unchanged |
| Tracked deletions/renames | Zero |
| Protected developer-local files | All four SHA-256 values unchanged; never staged |
| Prior acceptance evidence | All 539 recorded UI artifacts retain their hashes |
| Critical files | Fourteen baseline hashes unchanged, including package/locks, native manifests/configuration and backend.ts |

[Baseline](docs/ui-3/baseline.json), [preservation](docs/ui-3/preservation.json),
[changed paths](docs/ui-3/changed-files.txt), [local history](docs/ui-3/commits.txt).
Initial full inventories, relevant processes and documentation paths were also
recorded under `%TEMP%/aurora-ui3-20261014` before application edits.

Local, independently revertible commits:

| Commit | Scope |
| --- | --- |
| `35ce1ee` | Audit, baseline and deterministic capture harness |
| `3740a2a` | Shared Discover context, search, counts and feedback |
| `a4c39ea` | Compact Mods rows and guarded explicit install actions |
| `e7d1fd9` | Artwork-led modpack grid and exact-version review |
| `895f36b` | Distinct resource-pack cards |
| `b799abf` | Installed Library controls, state and cancellation focus |
| `fd7e48b` | Reduced-transparency content backing |
| `3b41535` | Browser/native fixtures, regression checks and capture tools |
| This delivery commit | Acceptance report, review guide, screenshots and logs |

Application edits are confined to `ModrinthBrowse.svelte`, `InstalledArtwork.svelte`,
`InstanceModsPanel.svelte`, `InstancePacksPanel.svelte`, the new presentation-only
`ContentViewSwitch.svelte`, and five lines in `phase-f.css`. Design documentation,
two existing browser assertions and UI-3 test/evidence files complete the change.
No Rust, DTO, dependency, package, lockfile, version, window minimum or release edit.

## C. Feature inventory

The [pre-implementation audit](docs/ui-3/AUDIT_AND_PROPOSAL.md) records architecture,
capabilities, weaknesses, preservation risks, test strategy and implementation order.
It was committed before broad implementation.

One shared ModrinthBrowse remains inside existing instance workspaces and the
Instances standalone modpack flow. There is no new Discover route or global
Library. Installed Mods and Packs retain distinct state/operation owners.

- Explicit submitted search; clearing requests the default query. No debounce
  existed and none was introduced. Provider categories, four sorts, native
  compatibility facets, 20-item pagination and 45-second default cache remain.
- Mods keep quick install, dependency-only Keep, exact Details/review, conflicts,
  verified acquisition and native dependency handling. Resource Packs use their
  existing native version/format restrictions and install paths.
- Modpacks keep exact version choice, optional files, fingerprinted renewed
  review and independent instance creation. They never enter quick mod install.
- Installed Mods keep snapshot search/filter/sort, disk-backed enable/disable,
  local/provider removal confirmations, recognition, explicit updates and Details.
  Packs keep snapshot search/filter, existing native activation capabilities,
  provider/local removal, recognition and updates. No pack sort was fabricated.
- Installed modpacks remain instances with their existing update workflow;
  there is no fictitious installed-pack toggle. Shaders retain existing shared
  browsing and truthful in-game activation; no shader feature was added.
- Search hits supply title, author, summary, downloads and categories. They do
  not supply exact compatible game versions, update timestamps or ratings.
  Those facts were not invented. Exact compatibility remains in Details/preview.

## D. Design decisions

Shared controls now give the content type, source and explicit target instance;
standalone modpacks state that they create a new instance. Search has a specific
placeholder, independent accessible clear button and focus return. Controls align
at 40px and grow with text. Result counts reflect the last completed request,
not an unsubmitted draft. Loading, empty, error/retry and installation feedback
are distinct. The compatibility context uses the supplied loader label instead
of hard-coding Fabric for NeoForge.

Mods use a single compact list with 44px cached artwork, bounded descriptions,
secondary provider metadata and aligned visible Install/Keep/Installed/Blocked
actions. Local duplicate guards cover rapid clicks on quick and reviewed actions;
native enforcement remains authoritative. Per-project Details retain their inline
panel. Buttons have project-specific accessible names. No whole-card click target.

Modpacks use a responsive artwork-led grid: four columns at 1440/1280, three at
1024 and two at 720 with default text in the matched fixture. The same cached
square provider icon enlarges to 104px inside a 16:9 frame; images remain contained
and uncropped. No banner scraping, extra artwork request or invented imagery.
Review pack is the explicit action; New instance remains visible.

Resource packs use 64px cached artwork and medium cards, two columns above 760px
of content width and one below. They retain their own compatibility wording.

Installed views share only the two-button visual view switch. Operation/state
logic stays in its current owner. Rows show truthful metadata and textual state,
local counts, consistent 40px switch/trash/details targets, and full content in
Details. Long pack descriptions collapse to two lines. Pack removal cancellation
now returns focus to its trigger. Existing confirmations and capabilities remain.
No generic provider framework or broad duplication refactor was introduced.

Existing Borealis backgrounds/materials, typography, rail and configurable accents
remain. No new backdrop filter; existing enclosing workspace glass is reused.
Reduced transparency now also makes the localized content backing opaque.
The configured minimum remains unchanged; vertical scrolling retains actions.

## E. Visual evidence

Use the [review guide](docs/ui-3/REVIEW_GUIDE.md) for all comparison sheets,
original captures and actual native screenshots. Six matched sheets cover Mods,
Modpacks, Resource Packs, Installed Mods/Packs, and empty/error/loading. Originals
are in `docs/ui-3/before/` and `after/`: 23 per side, actual components with
deterministic provider fixtures, reduced motion, DPR 1 and blocked public requests.
Populated installed libraries are clearly synthetic. They do not claim native
installation, account identity or readiness.

All six sheets and representative originals were visually inspected. Final
after evidence was regenerated against a freshly started review server (see F).

| Matched measurement | Before → after |
| --- | --- |
| Mods first row, 1440×900 | 140.47 → 91.47px, 34.9% shorter |
| Mods first row, 1280×800 | 159.50 → 91.47px, 42.7% shorter |
| Mods first row, 1024×768 | 92.47 → 91.47px |
| Modpack columns, 1440 / 1024 | 2 / 1 → 4 / 3 |
| Long installed-pack row, 1280×800 | 167 → 99px |
| Long collapsed pack description | 102 → 34px; full text remains in Details |
| Horizontal overflow, 20 matched layouts | None before or after |

[Geometry comparison](docs/ui-3/geometry-comparison.json). The single-column Mods
decision favors scan order and action alignment; fewer columns mean these shorter
rows do not claim greater total visible-result count at every wide viewport.
Larger Library action targets can add height at narrow widths. Modpacks deliberately
spend more height on artwork. The added count and state context require some space.
At minimum size the existing header/navigation requires scrolling to results.

No Note/Lunar screenshots were attached. Their written density/artwork principles
guided the work without copying branding, icons, artwork or exact layouts.

## F. Automated verification

Actual commands/results are in [checks/exits.json](docs/ui-3/checks/exits.json)
and adjacent logs. All final verification below exited **0**:

| Command | Actual result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings; final run through installed npm-cli.js (`npm-check-direct`) |
| `node --test --test-concurrency=1 --test-timeout=60000 "src/lib/**/*.test.ts"` | 272 tests / 16 suites; all pass, no skips/cancellations |
| Same runner on borealis, phaseF, releasePolish and productSurfaces after the shared CSS correction | 61 tests, all pass |
| `node tools/visual/ui3.test.mjs` | 11 browser groups; 50 surface/size/text-scale combinations; zero page errors |
| `node tools/visual/ui3-pilot.mjs` | Four Mods layouts plus search/clear/focus/category/sort pilot |
| `node tools/visual/ui2-ui1-regression.mjs` | 11 UI-1 cases; final fresh-style rerun passes |
| `node tools/visual/ui2.test.mjs` | 17 Home cases, pass |
| `node tools/visual/ui2b.test.mjs` | Six Borealis/Home groups, pass |
| `node tools/visual/l1.test.mjs` | Four content kinds, artwork retry/fallback, filters, long metadata and geometry; fresh-style rerun passes |
| `node tools/visual/ui-corrections.test.mjs` | Six groups including 100 installed rows and four 100-result views; fresh-style rerun passes |
| `node tools/visual/l3.test.mjs` | Four layouts plus account/cape/widget interaction sequence, pass |
| `npm run tauri build -- --debug --no-bundle` | Production static frontend plus debug native executable, no installer; final-native-build passes |
| Matched before/after capture and review generation | 46 screenshots, six comparison sheets, no page errors |
| `node tools/visual/ui3-preserve.mjs` and Git whitespace/deletion checks | Original paths, hashes and worktrees preserved |

Browser checks use the existing external Playwright runtime through
`AURORA_PLAYWRIGHT_MODULE`. No dependency was installed. UI-1/UI-2 outputs are
redirected under UI-3, preserving their historical evidence. The two existing
assertion changes express the approved new content-kind geometry and empty-state
copy; containment, artwork, native-request and interaction assertions remain.

**Failed/intermediate attempts:** fixture request recording initially tried to
clone Svelte proxies; fixture metadata/filenames also overlapped under a renamed
test mod, and an empty setter retained an old snapshot. These fixture defects
were corrected. A clear button nested in a label changed the search accessible
name; separating the label/input fixed it. Initial unused CSS warnings were removed.
An early staged test correctly stopped at a resource size not yet implemented.

The baseline source-swap helper once failed restoration under a Windows file lock.
Work paused, the intact baseline was confirmed, committed shared controls and
recorded pending changes were restored, and preservation/type/pilot checks passed.
The helper now retains disk backups and retries restoration. Visual inspection
later found Vite retaining baseline styles across the temporary swap: restart fixed
it, long descriptions measured 34px, final captures were regenerated and affected
browser suites rerun. A focused compact-description assertion now guards this.

An initial native test reader encountered PowerShell's UTF-16 JSON output; it now
handles the BOM. A nested Windows PowerShell npm wrapper was blocked by its script
policy despite returning shell exit 0; **that was not a successful check**. The
final direct npm CLI run actually executed the complete check and passed. No
system execution policy was changed. Final logs identify the passing executions.

No Rust source/startup/native boundary changed, so no new Rust format/check/test
pass is claimed. The native build retained the existing STATIC_VCRUNTIME warning.

## G. Native acceptance

Actual compiled frontend/Rust commands ran in fresh marked direct-temp managed
and WebView roots, recorded in [native/start.json](docs/ui-3/native/start.json).
Production identifier/configuration stayed unchanged. The root is retained.
No owner content or credentials were copied. The initial Fabric fixture honestly
remained Installing and Browse refused it. The existing native installer then
completed its real Minecraft 1.21.11 / Fabric 0.19.5 installation in 92.229 seconds:
[install result](docs/ui-3/native/install-result.json). No Ready flag was fabricated.
This is content readiness; no authenticated game launch or runtime readiness claim.

Computer Use inspected/navigated the actual Windows app. Read-only CDP captured
the real `http://tauri.localhost/` WebView, separately from browser fixtures.
Native acceptance includes live Mods/Modpacks/Resource Pack results and artwork,
submitted Sodium search, real Adventure filtering to an empty result, Escape
focus return, filter reset, empty Installed Mods/Packs, navigation, scrolling,
window resizing, Borealis/cyan persistence across restart, and clean exit.

| Native state | Measured CSS client | Scale | Overflow |
| --- | --- | --- | --- |
| Default | 1120×790 | 1.25 / 125% | None |
| Medium after actual frame resizing | 1016×768 | 1.25 / 125% | None |
| Actual enforced minimum | 706×512 | 1.25 / 125% | None |
| Maximized | 2048×1152 | 1.25 / 125% | None |
| Restart | 1120×790, Borealis/cyan retained | 1.25 / 125% | None |

The configured 720×520 minimum was never changed. Its actual client measurement
is reported instead of assuming nominal values. Browser checks additionally cover
706×512 on all five surfaces at 100% and 200% text. Native minimum captures show
Mods at the top and after scrolling, with Install/Details reachable. Other kinds'
native captures use default/maximized sizes; their complete size matrix is browser
acceptance. Resize attempts that only moved scrollbars remain labeled attempts.
Actual top/right frame resizing subsequently established the measured sizes.

The first app (PID 18100) closed through its own Close button; its helper recorded
null for exit code, so zero is not claimed for that run. The corrected helper
retained the process handle. Restart PID 23316 closed normally with **exit 0**:
[restart/exit.json](docs/ui-3/native/restart/exit.json). Both processes are gone.
No game, login, mod or pack installation was initiated. Only the disposable base
Minecraft/Fabric game was installed through the existing native command.

## H. Functional preservation

Rust source, backend.ts, DTOs, error codes, registry/pack schemas, native minimum,
provider endpoints, hash verification, dependency/compatibility rules, exact pack
selection/creation, filename conventions, ownership and rollback remain unchanged.
Artwork uses the same shared native normalized-PNG/cache boundary and promises.
Search submission/facet/pagination requests are asserted with deterministic data.
Quick duplicate clicks send one native request; installed state comes from refreshed
inventory. Pack optional changes require a renewed preview. Local/provider removal
and backend capability distinctions are retained. No indexing/scanning changes.

## I. Accessibility and performance

Checks cover keyboard search/clear/focus, category checkboxes/Escape, native selects,
project-specific button names, textual installed/disabled/blocked states, live status
announcements, independent Details controls, cancellation focus, long multilingual
content, 200% text, cyan and reduced motion/transparency. Full descriptions/names
remain readable in Details. Simple result metadata contrast measures **9.65:1**
in the UI-1 regression. Dynamic Borealis readability was visually inspected; this
is not a complete screen-reader, high-contrast or every-animation-frame certification.

No new polling, subscription, download path, backdrop filter per row, virtualization,
framework or dependency. Existing 100-row/result coverage, artwork cache reuse,
bounded retries and unmount behavior pass. No new performance benchmark is claimed.

## J. Preservation

Production Launcher 1.4.2, owner instances/worlds/saves/mods/packs/authentication,
ordinary launcher preferences/caches and published artifacts were not modified.
All native filesystem mutation was confined to the new marked temporary root.
Its game and appearance changes were intentional acceptance actions, retained for
inspection. No owner sign-in, credential copy or account mutation occurred.
All original tracked paths, critical/protected hashes and prior UI artifacts remain;
the other worktrees were not operated on. Review servers and native apps were closed
using only this run's exact verified PIDs/window handles. No process-name termination,
clean/reset/prune, push, merge, tag, publish, deploy, bundle, signing or version bump.
Generated build/game/runtime data is not staged. No UI-4 work began.

## K. Remaining concerns

- Blocking regressions: none observed in final checks.
- Visual defects: no blocking defect observed in final captures; owner aesthetic
  approval remains outstanding. Single-column Mods
  trade total wide-window result count for alignment; larger controls use some space.
- Existing provider limitations: broad search facets do not prove an installable
  exact version; inline Details/native preview remain authoritative. Cross-type
  browsing uses the host's supplied installed inventory and does not add scans solely
  to decorate every other type. Missing search DTO metadata is not fabricated.
- Acceptance limits: populated Library operations are deterministic browser fixtures;
  native libraries were empty. No live modpack/component transaction or authenticated
  gameplay was tested in UI-3. Native minimum measured Mods; all other size matrices
  use actual components in browser fixtures.
- Deferred: new routes, loaders/providers, shader features, indexing, bulk deletion,
  backend installation changes, UI-4 and release work.

## L. Recommendation

**UI-3 PASS — ready for owner visual review.** Review the six comparison sheets,
the actual native discovery/minimum/cyan captures, and the noted density tradeoffs.
Stop at this checkpoint. No automatic UI-4 or release activity.
