# UI-3.1 — Single-column Modpack layout acceptance

**PASS — ready for owner visual review.** Modpacks now use compact full-width
rows matching the approved Mods presentation. Exact pack review and independent
instance creation remain distinct. Stop after UI-3.1; no UI-4 or release activity.

## Baseline and scope

- Branch: `codex/client-3-production-authority`, unchanged.
- Starting HEAD: `c0c934263e8ff6d0fb25d91cc4db719815bf31a1`.
- Since reported UI-3 HEAD `4b3cab0`: only `c0c9342`, the owner-requested page-wide
  selection accent correction from this chat. It is retained unchanged.
- Ending HEAD: the single local commit containing this report. Its exact SHA is
  in the delivery message and `git log -1 --format=%H -- UI_3_1_MODPACK_LAYOUT_ACCEPTANCE.md`.
- Initial tracked changes: none. Baseline: 1,696 tracked files, seven worktrees,
  four protected developer-local files. [Full baseline](docs/ui-3-1/baseline.json).
- AGENTS.md and ARCHITECTURE.md were read in full during UI-3; current hashes
  were confirmed byte-identical before implementation. UI-3 acceptance, audit,
  review guide and current Mods/Modpacks source were reviewed for this correction.

The sole application file changed is `src/lib/instances/ModrinthBrowse.svelte`.
Existing test updates are limited to `tools/visual/ui3.test.mjs` and `l1.test.mjs`.
New UI-3.1 capture, test, review, preservation and native helpers live under
`tools/visual/ui31-*`; this report and `docs/ui-3-1/` contain evidence.
[Complete changed paths](docs/ui-3-1/changed-files.txt).

## Exact visual correction

The existing Mods row declarations now also select `.browse-modpacks`, using
the same CSS specificity and values. No generic component or provider layer
was added. Removed the modpack grid, 16:9 frame, 104px image and card-specific
action layout. The same cached provider image now renders at 44×44px, contained,
with the same fallback and reserved frame as Mods.

Each result is one full-width row: artwork left, title/description/author/provider
metadata center, and New instance plus Review pack right. Rows share 12px padding,
zero inter-row gaps, separators, typography, two-line descriptions and 40px action
targets. At content widths of 560px or less, actions wrap below the metadata using
the existing Mods rules. Titles can wrap; text and controls remain available.
The shared material and interaction rules remain the same, including selected
appearance accents. Search, filters, headings, navigation and other pages are unchanged.

## Responsive measurements and visual evidence

[Screenshot guide](docs/ui-3-1/REVIEW_GUIDE.md),
[matched comparison](docs/ui-3-1/modpacks-before-after.png),
[Mods/Modpacks side by side](docs/ui-3-1/mods-modpacks-rows.png),
[measured geometry](docs/ui-3-1/geometry-comparison.json).

| CSS viewport | Before columns | After columns | First result height before → after |
| --- | --- | --- | --- |
| 1440×900 | 4 | 1 | 347.47 → 91.47px |
| 1280×800 | 4 | 1 | 326.77 → 91.47px |
| 1024×768 | 3 | 1 | 330.81 → 110.50px |
| 720×520 | 2 | 1 | 331.47 → 158.50px |
| 706×512 | 2 | 1 | 327.53 → 158.50px |

Artwork is 44×44px at every size. Padding, gaps, borders, title typography,
description truncation and artwork geometry match approved Mods. At 1024px,
the pack's longer metadata and distinct action consume more height. At narrow
sizes both types wrap actions to the next line. No horizontal overflow occurred.
The existing parent headers require vertical scrolling at minimum size; additional
scrolled result captures show the complete primary action.

All comparison sheets and representative full-resolution/native captures were
visually inspected. The originals are retained. Fewer columns change total visible
result counts; the measured shorter rows do not claim higher total density at every
viewport. No new artwork request, random image, cropped banner or metadata was added.

## Workflow and accessibility preservation

Both Svelte script blocks are byte-identical to baseline. Native code, provider
contracts, request construction, caches, version choice, optional files, review
fingerprints, instance creation, verification, progress and error handling are unchanged.
Review pack still opens the existing project/details interface; that interface replaces
results within the same browser rather than expanding inside a constrained row.

Focused browser checks verify keyboard focus and Enter on Review pack, accessible
project-specific names, exact version selection, optional selection disabling Install
until renewed review, and the renewed fingerprint passed to the distinct pack command.
A successful browser-only command fixture creates/selects a different instance ID;
no quick-mod install request occurs. This proves frontend dispatch/selection and does
not claim a real native pack installation. Real pack installation was not initiated.
Existing UI-3 coverage retains loading/error/status announcements and safe failure checks.

Long multilingual titles, descriptions and authors, action/metadata separation,
artwork containment, 100%/200% text, keyboard outlines, reduced motion and reduced
transparency pass. Full descriptions remain in Details. Existing Simple metadata
contrast remains 9.65:1 in UI-1. No per-row blur, network loop, dependency or new route.
No complete screen-reader or every-background-frame certification is claimed.

## Automated verification

Actual commands and exit codes are in [checks/exits.json](docs/ui-3-1/checks/exits.json)
and adjacent full logs. All final checks below exited **0**:

| Check | Result |
| --- | --- |
| `npm run check` through installed npm CLI | 0 errors, 0 warnings |
| `node --test --test-concurrency=1 --test-timeout=60000 "src/lib/**/*.test.ts"` | 272 tests / 16 suites; all pass, no skips/cancellations |
| `node tools/visual/ui31.test.mjs` | 13 groups, ten Modpack size/text combinations, exact review workflow; zero page errors |
| `node tools/visual/ui3.test.mjs` | 11 groups, all five surfaces at five sizes and 100%/200% text; zero page errors |
| `node tools/visual/ui2-ui1-regression.mjs` | 11 UI-1 cases, pass |
| `node tools/visual/ui2.test.mjs` | 17 UI-2 cases, pass |
| `node tools/visual/ui2b.test.mjs` | 6 UI-2B groups, pass |
| `node tools/visual/l1.test.mjs` | All four content types, artwork retry/fallback, filters, long content and containment pass |
| `npm run tauri build -- --debug --no-bundle` | Production static frontend and debug executable built; no installer |
| Capture/review helpers | 30 matched originals, three scrolled results, six focused review states, two comparison sheets |
| Preservation and Git whitespace/deletion checks | Pass |

UI-3/L1 assertions expecting modpack grids were replaced by one-column and 44px
artwork assertions. Unrelated checks remain. UI-3 output now accepts an environment
override so regression evidence cannot overwrite prior UI-3 files. The existing
external Playwright/Python runtimes were used; no package or lockfile changed.

The first focused test had an incorrect expected fixture prefix (`mU` versus `pU`);
it was corrected to the existing fixture's distinct modpack identity. The final
test passes. An initial launch through Windows PowerShell 5 was blocked by its
script policy; the local script ran under the already-active PowerShell 7 host
without changing execution policy. The native build retained its existing
STATIC_VCRUNTIME deprecation warning. No Rust source changed and no Rust test pass
is claimed for this styling correction.

## Mods, Resource Packs and Library regression proof

All 20 before/after measured Mods, Resource Packs and Installed Mods/Packs layouts
are exactly equal. All ten matched Mods/Resource Pack screenshot PNGs are byte-identical
at the five sizes. Installed components, Home, navigation, accent styles and launcher
configuration files retain their original hashes. The approved Mods appearance and
the page-wide accent correction remain intact.

## Actual native acceptance

The built debug executable ran in a new marked temporary data root and a separate
WebView profile, with no accounts or instances. [Native start record](docs/ui-3-1/native/start.json)
contains paths, PID and executable hash. The existing owner-installed launcher
was visible as a separate process and was never operated on.

Computer Use navigated the actual app; read-only CDP captured its compiled
`http://tauri.localhost/` frontend after verifying the native managed root.
Real Rust Modrinth results and cached artwork rendered as a single column.

- Default: **1120×790** CSS client, 125% OS scale / DPR 1.25, 20 live results,
  44px artwork, 40px Review pack action, no overflow.
- Actual enforced minimum: **706×512** CSS client, same scale, one column;
  scrolling exposes full rows and Review pack. Configured 720×520 minimum unchanged.
- Review Fabulously Optimized opens its real project details and version selector
  at minimum size, without clipping/overflow. Results navigation returns to the list.
- Close through the isolated app's own button: PID 20392, **exit 0**, retained
  process handle; [exit record](docs/ui-3-1/native/exit.json).

[Default](docs/ui-3-1/native/modpacks-default.png),
[minimum results](docs/ui-3-1/native/minimum-results.png),
[minimum details](docs/ui-3-1/native/minimum-details.png).
Native medium sizing is supplemented by the deterministic 1024×768 browser capture.
Intermediate 722×512 and unsuccessful frame-drag attempts are documented in the guide;
they are not mislabeled as enforced-minimum or medium acceptance. No native modpack
preview acquisition, installation, sign-in, game launch or owner-content copy occurred.

## Preservation and limitations

[Preservation](docs/ui-3-1/preservation.json) verifies every original tracked file
outside the three scoped source/test files, all four protected local file hashes,
all seven worktrees and zero deletions/renames. Prior screenshots, acceptance reports
and logs retain their bytes. Production Launcher 1.4.2, owner configuration/caches,
instances, worlds/saves/mods/packs, credentials and release assets were not touched.
Only the fresh marked native root received ordinary app/cache data; it is retained.
Generated builds and managed data are not staged. No clean/reset/prune, process-name
termination, push, merge, tag, publish, version bump or release preparation.

No blocking regression or visual defect was observed. Native acceptance covers live
rows/details and minimum scrolling; optional-file and successful new-instance paths
use deterministic browser command fixtures. Real installation correctness remains
with the unchanged backend. Owner visual review remains pending. Stop at UI-3.1.
