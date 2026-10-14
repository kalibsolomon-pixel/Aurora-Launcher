# Aurora Launcher — UI-2 Home composition acceptance

> Owner-requested card refinement supersedes the card metadata/Manage action and
> visible Ready badge described below: Home now shows Launch and an inline
> green/red selector dot. See [refinement acceptance](docs/ui-2/card-refinement/README.md).

## A. Verdict

**PASS — ready for owner visual review.** Home has an identity-first instance
card, prominent stateful Play, a bounded external player and earlier, aligned
widgets. UI-1 shell/Mods styling and all native authority remain unchanged.
Browser, frontend build and real isolated Windows Tauri presentation acceptance
passed. This is UI-2 only; no release or UI-3/UI-4 work was started.

## B. Repository baseline and local commits

- Branch: `codex/client-3-production-authority` throughout.
- Starting HEAD: `42e323b00c8ea8b744b3714952b9b09b66d834d8`.
- Ending implementation/tests HEAD: `cbde9aab967b93e9c14e12a927678703e576b891`.
  This report/evidence is the subsequent documentation-only commit. Its exact
  ending checkout HEAD is included in the delivery message; use `git rev-parse HEAD`
  to identify that commit when reading this report from the checkout.
- Latest four baseline commits: `42e323b`, `d5a707d`, `c783e14`, `fb1fb95`.
  Their approved UI-1 implementation and Home comparison were inspected.
- Baseline: 956 tracked files; zero tracked modifications; seven worktrees.
- Original tracked paths and critical root documentation, manifests, lockfiles,
  package metadata and native build configuration remain present. **Zero tracked
  deletions.** No unexpected concurrent source changes were observed.

| Local commit | Independently revertible change |
| --- | --- |
| `770ed45` | Instance identity/status before Play; quieter versions and existing Settings entry |
| `eb1a80d` | Home proportions, reduced logo footprint and bounded external preview |
| `192a73d` | Widget heading, spacing, sparse/full-row layout and container-responsive spans |
| `11e6e38` | Equal widget row heights and honest unavailable-history presentation |
| `cbde9aa` | Deterministic browser/native harness, persistence and interaction coverage |
| Report commit | This acceptance document and evidence under `docs/ui-2/` |

Application changes: `src/lib/pages/HomePage.svelte`,
`src/lib/launcher/PlayerPreview.svelte`, `src/lib/launcher/HomeWidgets.svelte`,
`src/lib/launcher/widgets/RecentTargetsWidget.svelte`. Test change:
`src/lib/launcher/releasePolish.test.ts`. Eight new review helpers are prefixed
`tools/visual/ui2-` or named `tools/visual/ui2.test.mjs`. Full code name-status:
[changed-code-files.txt](docs/ui-2/changed-code-files.txt).

The four protected local files retain their exact starting SHA-256/length:
`.zcode-icon-diag.py`, `.zcodeignore`,
`UserskalibAppDataLocalTempaurora-phase-d-final-evidencesodium-exit-zero-ready.txt`,
and `docs/performance-p0-2/__pycache__/analyze.cpython-312.pyc`.
Evidence: [baseline](docs/ui-2/baseline.json),
[preservation](docs/ui-2/preservation.txt),
[worktrees after](docs/ui-2/worktrees-after.txt).
No other worktree was modified, archived or created. Historical UI-1 evidence
was preserved: its regression suite ran through an output-routing wrapper into
`docs/ui-2/ui1-regression/`.

## C. Home architecture inventory

Detailed pre-edit inventory and proposal:
[AUDIT_AND_PROPOSAL.md](docs/ui-2/AUDIT_AND_PROPOSAL.md).

| Concern | Actual source / state owner |
| --- | --- |
| Home composition and scoped styles | `src/lib/pages/HomePage.svelte` |
| Selected instance, installed versions/loader/Aurora, installation record | `launcher/store.svelte.ts`: selectedInstance from Rust launcherState/config/instances; `instanceStatus.ts`: installedConfigurationLabel |
| Selector | `launcher/InstanceSwitcher.svelte`: runSelect and keyboard listbox, shared with empty Home |
| Play/readiness/process/errors | Rust-derived playReadiness/playProcess in store; Home checks both instance and account identity; `launcher/home.ts` presents the decision; existing runPlay/refreshPlayReadiness unchanged |
| Management | Existing typed `navigation.openInstance(id, "settings")`, same instance workspace and Settings tab |
| Account identity/selection | Store accountsState/selectedAccount and accountManager; AppShell/AccountIdentity/AccountDialog; bottom-left dock unchanged |
| Player skin/pose | PlayerPreview uses selected-account-keyed accountAvatars, playerModel/playerPixels/skinTextures, default yaw and existing manual controls |
| Widget registry/sizing/order | `launcher/widgets.ts`, `HomeWidgets.svelte`; eight existing registrations |
| Widget edit state/persistence | `homeLayout.svelte.ts`, backend get/save/resetHomeWidgets, Rust home_widgets/config; ordered ID/enabled/size format unchanged |
| Playtime | PlaytimeWidget -> getPlaytimeSummary/getDailyPlaytime -> native gameplay_history |
| Recent worlds/servers | RecentTargetsWidget -> native history; optional server_enrichment overlays presentation facts |
| Remaining widget content | InstanceDetailsWidget, ContentSummaryWidget, SessionWidget, SkinManagerWidget, CapeSelectorWidget |
| Discord | Existing discord store/DiscordSettings/native worker; no Home indicator is currently present |
| Shell/responsive/materials | AppShell, phase-f.css, app.css; shared UI-1 rail/gutters/materials untouched |

Home and the external player are Home-specific. Selector, account identity,
navigation, widget/store/native DTOs and design tokens remain shared authorities.
No parallel state model, readiness calculation or new native command was added.

## D. Baseline findings

1. Play preceded the selected identity and readiness.
2. Large branding/player regions pushed widget content below the fold: first
   cards started at y=778/684/652 at 1440/1280/1024 respectively.
3. Equal primary columns reserved as much width for the cosmetic player as the
   functional card, although the model used only part of that region.
4. A sole small widget left half a row unused; the old viewport-only widget
   breakpoint stacked cards even where readable columns could fit.
5. The unlabeled pencil and lack of a direct Home management entry weakened
   discoverability; the preview disappeared at 900px despite usable space.

These are composition findings, not native state/data defects. The supplied Note
screenshots were not attached; the written hierarchy principles informed the
proposal, while Aurora branding, accent and materials remain the visual authority.

## E. Design changes

**Instance and Play.** The integrated selector leads, followed by the actual
readiness/status badge, Play, then concise installed metadata and a quiet Manage
instance action. Long names ellipsize within the selector and remain available
through its title/listbox. Vanilla remains labeled Vanilla / Aurora Off; loader
differences come from the unchanged installedConfigurationLabel. Blocker messages,
technical codes, Check again, process failures and exit details remain available.
No optimistic Ready, account bypass or altered process transition was introduced.

Play retains the existing accent-derived Borealis fill/hover/pressed/contrast
system and visible focus. Its 78px desktop / 68px narrow minimum height remains;
rem typography grows with text scaling. The added management action opens an
existing Settings destination for the selected instance, with no new route.

**Player.** The launch column receives slightly more width (1.15:0.85). Branding
shrinks from 200–280px to 112–144px (96px narrow). Player height is bounded at
280–420px using the existing software renderer. Its canvas/controls have a
matching region instead of a large unused aspect-ratio area. Default pose,
account-keyed pixels, manual rotation/reset and redraw-only lifetime remain.
There is no new rendering dependency, animation, skin request or cosmetic feature.

The player stays outside the card and is visible above 800px. At <=800px it yields
to the primary controls; this covers the configured 720px minimum and the measured
smaller native client area at 125% scaling. It remains visible at 801/900/1024px.
Missing skin retains the established quiet empty region; another account's skin
never substitutes for it.

**Widgets.** A restrained Home widgets heading identifies the section and existing
pencil/Done controls. Cards use 24px desktop / 16px narrow padding and gaps, with
equal heights within a row. Small remains one logical column; Wide/Large span two;
Large retains its taller minimum. A sole visible widget fills the row. Below
640px of actual widget-container width, all saved spans clamp to one. DOM/saved
order is preserved without dense packing, so mixed wide layouts may intentionally
retain an unfilled slot rather than change reading order. No layout is migrated
or reset. Unknown/hidden registrations survive editing and reload.

The existing RecentTargetsWidget initially displayed both an error and “No
recently played servers” when acquisition failed. Its presentation now suppresses
the empty claim while retaining the error. Previously loaded rows remain usable
with their error. History acquisition, bridge, telemetry and native behavior are
unchanged.

**Borealis.** The 1280px centered cap, UI-1 96px rail, shell gutters, palette,
glass/backdrop-filter budget, typography roles and radius/accent tokens remain.
No global CSS, Mods style, navigation or account dock was changed. OLED, Simple,
alternate accents and reduced transparency retain their existing behavior.

## F. Before-and-after evidence

The comparison is [home-comparison.png](docs/ui-2/home-comparison.png).
State and text/accent reviews:
[states-review.png](docs/ui-2/states-review.png),
[accessibility-review.png](docs/ui-2/accessibility-review.png).
Sheets are labeled thumbnails; original captures are retained.

| Browser client size | Before | After |
| --- | --- | --- |
| 1440×900 | [Home](docs/ui-2/before/home-1440x900.png) | [Home](docs/ui-2/after/home-1440x900.png) |
| 1280×800 | [Home](docs/ui-2/before/home-1280x800.png) | [Home](docs/ui-2/after/home-1280x800.png) |
| 1024×768 | [Home](docs/ui-2/before/home-1024x768.png) | [Home](docs/ui-2/after/home-1024x768.png) |
| 720×520 | [Home](docs/ui-2/before/home-720x520.png) | [Home](docs/ui-2/after/home-720x520.png) |

Each before/after folder also contains matched 1280×800 sparse, all-eight/mixed
sizes, dense-bottom, long-name, blocked, running, missing-skin, empty, unavailable,
loading and OLED/Simple captures: 15 screenshots per phase. These are **Chromium
browser fixtures**, actual Svelte components with synthetic DTOs/session storage,
100% CSS scale, DPR 1, reduced motion and blocked external requests. They are not
native screenshots. Geometry:
[before](docs/ui-2/before/geometry.json), [after](docs/ui-2/after/geometry.json).

| Size | Card width before → after | First widget card y before → after |
| --- | --- | --- |
| 1440×900 | 599 → 670px | 778 → 594px |
| 1280×800 | 525 → 586px | 684 → 577px |
| 1024×768 | 408 → 454px | 652 → 549px |
| 720×520 | 569 → 569px | 513 → 462px |

Actual rendered screenshots and numeric measurements were both inspected.
Identity and Play are clear, metadata is quieter, widgets align beneath the
composition, the player has separate reserved controls, and no measured
horizontal overlap/overflow occurs. Narrow/short views scroll vertically.

## G. Functional acceptance

The new browser suite passes **17 cases**, including six viewport cases:
1440×900, 1280×800, 1024×768, 900×700, 801×600 and 720×520.
[functional.json](docs/ui-2/functional.json) records cases and geometry.

- Keyboard selector opens with ArrowDown, End/Enter selects Vanilla; installed
  metadata and selected identity follow the existing native selection boundary.
- Play invokes the existing `play_instance` with the same instance/account IDs.
  Three synchronous clicks produce one synthetic command; Running disables Play.
- Checking, blocked, unavailable, Starting, Running, failed and stale account/
  instance readiness are tested without changing their state model. Failed process
  presentation intentionally retains the existing native Ready decision plus
  the visible failure message; the frontend does not invent a new transition.
- Manage opens the existing selected-instance Settings, Home navigation returns,
  and the bottom-left account dialog closes with Escape/focus return.
- Manual rotation changes pixels, Reset restores them, and switching to an account
  with no skin immediately removes the previous canvas. Missing skin is contained.
- One/two/all-eight widgets, mixed sizes, all-off, long content, resize and saved
  reading order are covered. Keyboard size/order/hide/add actions persist through
  reload, including hidden and unknown slots. Native persistence also passed.
- History valid, zero, unavailable/error and loading render separately. No
  fabricated data enters the application; only explicitly labeled fixtures have
  synthetic playtime/server content.
- Focus, hover, pressed, disabled Play, 200% text, reduced motion and an alternate
  cyan accent/OLED Simple contrast are checked. Existing UI-1 Mods/navigation,
  content-kind, cosmetics and glass regressions pass unchanged.

No real game was installed or launched for this work. Native acceptance used
an Installing fixture and no account, rather than treating a fabricated Ready
flag as launch authority.

## H. Automated verification

Browser commands use the existing external Playwright module override documented
in UI-1 and `node tools/visual/serve.mjs`; no dependency was installed.

| Command | Actual result | Exit |
| --- | --- | --- |
| `npm run check` | 0 errors, 0 warnings | 0 |
| `node --test --test-concurrency=1 --test-timeout=60000 "src/lib/**/*.test.ts"` | 272/272 tests, 16 suites, no skips/cancellations | 0 |
| `node --test --test-concurrency=1 --test-timeout=60000 src/lib/launcher/historyWidgets.test.ts src/lib/launcher/serverEnrichmentWidgets.test.ts src/lib/launcher/releasePolish.test.ts` | Final widget correction: 27/27 | 0 |
| `npm run build` via Tauri's unchanged beforeBuildCommand | Final static production frontend built | 0 |
| `npm run tauri build -- --debug --no-bundle` | Final isolated-use debug executable built; no installer | 0 |
| `node tools/visual/ui2.test.mjs` | 17 browser cases, zero page errors | 0 |
| `node tools/visual/ui2-ui1-regression.mjs` | Approved UI-1 suite: 11 cases, zero page errors | 0 |
| `node tools/visual/l1.test.mjs` | Four content kinds, responsive controls/artwork/filter coverage | 0 |
| `node tools/visual/l3.test.mjs` | Four sizes plus account/cape/widget interaction sequence | 0 |
| `node tools/visual/ui-corrections.test.mjs` | Six groups including 100-row/results, glass and reduced transparency | 0 |
| `node tools/visual/ui2-capture.mjs before` / `after` | 15 matching captures each, zero page errors | 0 / 0 |
| `node tools/visual/ui2-preserve.mjs`, Git diff checks | Original paths/hashes/worktrees preserved; no deletions/whitespace errors | 0 |

Logs and `.exit` files are in `docs/ui-2/`. Final frontend output is in
`native-build-final.txt`; the unchanged version-contract prehook verifies 1.4.2
without release activity. Rust source, DTOs, startup and Tauri configuration were
not changed, so no separate Rust format/check/test suite was required or claimed.
The native build reports the pre-existing STATIC_VCRUNTIME deprecation warning.

**Failed/interrupted attempts.** The npm test wrapper placed options after the
file list; observed workers were concurrent and stalled. Only verified test-parent
PID 22268 and its children 29532/21092/26120/14956 were stopped. Provenance and
output remain in interrupted-test-processes.json/frontend-tests-interrupted.txt.
The corrected explicit Node invocation then reported 271/272: the sole failure
was a source-text assertion for the old literal 900px breakpoint. It was replaced
by actual rendered two-column/narrow-span tests; all other editor assertions remain.
The next full suite passed 272/272, followed by the final 27 widget tests.

Early new-browser harness attempts had an incorrect expectation for the existing
“Aurora Off” metadata, an overly broad alert selector, a wrong Settings selector,
and session storage leaking between fixture cases. Those fixture defects were
corrected; final tests include explicit isolation and native restart verification.
One capture rewrite hit a transient Windows file-open error; its retry completed
all 15 captures. One native rebuild met a locked running development executable;
after closing the retained app normally, rebuilding passed (native-build-locked.txt).
An optional Playwright import in the desktop REPL failed; native geometry was
instead read by the standalone existing-runtime CDP script. No application source
was changed to work around these tool/harness failures.

## I. Real native Tauri acceptance

The final debug executable embeds the final production frontend and was launched
directly without installation or Tauri configuration overrides. Managed data uses
the existing debug-only `AURORA_DIAGNOSTIC_DATA_ROOT`; WebView2 has a separate
`WEBVIEW2_USER_DATA_FOLDER`. The fresh marked `aurora-ui2-<UUID>` root is a direct
temporary-directory child. It contains only a synthetic Installing Vanilla
registry/config, no owner account, credential, world, runtime or game content.
The temporary evidence is retained; no cleanup/deletion was performed.

Windows input/capture used the installed computer-use skill. A loopback WebView2
debugging port was enabled only in the launched child environment for read-only
client geometry and screenshots. App interactions/resizing used its actual native
window; no browser fixture is labeled native. The port closed with the app.

Final binary SHA-256:
`71502853ca460419d47546c322e7f8745a6f03ba23d59a24faf865ef0d9ed4f3`.
Final retained PID: 8; native window ID: 73009130. Native logs confirm the isolated
managed root and backend ready. Three observed development runs exited **0**,
including final run 042308. No crash or startup regression was observed.

| Actual final native client | DPR / OS scale | Evidence |
| --- | --- | --- |
| 1120×790 CSS px | 1.25 / 125% | [window](docs/ui-2/native/final-default-window.png), [geometry](docs/ui-2/native/final-default.json) |
| Maximized 2048×1152 CSS px | 1.25 / 125% | [window](docs/ui-2/native/final-maximized-window.png), [geometry](docs/ui-2/native/final-maximized.json) |
| Smallest observed client 706×512 CSS px | 1.25 / 125% | [window](docs/ui-2/native/final-minimum-window.png), [geometry](docs/ui-2/native/final-minimum.json) |
| Minimum widgets/editor | 1.25 / 125% | [widgets](docs/ui-2/native/final-minimum-widgets-window.png), [editor](docs/ui-2/native/final-minimum-editor-window.png) |
| Resized back to 1266×960 CSS px | 1.25 / 125% | [window](docs/ui-2/native/final-resized-desktop-window.png), [geometry](docs/ui-2/native/final-resized-desktop.json) |

The configured minimum remains 720×520; actual native client dimensions at the
enforced Windows minimum are smaller, and are reported rather than assumed.
Desktop window screenshots are provided at capture API logical dimensions;
`*-client.png` are actual WebView2 DPR-resolution captures (e.g. 1400×988 at the
default client, 883×640 at minimum). [image-inventory.json](docs/ui-2/image-inventory.json)
records every saved image's pixel dimensions. Desktop captures were normalized
from the capture API's JPEG to PNG without resizing their decoded pixels.

Home renders the selected fixture, disabled Play and all real blockers. No skin
is available without an account; the existing empty fallback renders gracefully.
The preview region yields at minimum and reappears after resizing back. Native
widgets, empty playtime/recent-history states, scrolling and the editor remain
usable at minimum with no measured horizontal overflow. Size and keyboard order
changes made through the native editor were saved to isolated config and restored
on a real process restart: layout-before-restart.json equals layout-after-restart.json.
Both final native layout and accessibility text were inspected. No real skin
authentication, game launch, provider install or update installation was attempted.

## J. Accessibility and performance

Semantic buttons/listbox/selects, visible focus, labeled player controls and
account entry remain. Readiness uses text and badges, never color alone. Errors
remain readable and technical codes available. 125%/200% UI-1 text tests and
200% UI-2 tests pass; minimum-height content scrolls without horizontal clipping.
At 200%, long helper text in the fixed UI-1 account dock wraps tightly; no shell
redesign was introduced. Full screen-reader/high-contrast certification is not
claimed.

No new animation loop, rendering dependency, subscription, network data source or
backdrop-filter surface was added. The same player redraws only on skin/angle/
visibility changes, with CSS and visibility thresholds synchronized. Reduced
motion removes canvas transition. Existing 100-result glass/artwork regressions
and reduced-transparency fallback pass. This is presentation acceptance, not a
new FPS/CPU/GPU benchmark claim.

## K. Preservation

Production Launcher 1.4.2 executable and uninstaller hashes match before/after;
the executable remains
`711df646d834588b404c7170db76b6546b409433aecfd882b026299412d13818`.
All five currently present files in the owner's launcher-state directory retain
their starting hashes. See [owner-before](docs/ui-2/owner-before.json) and
[owner-preservation](docs/ui-2/owner-preservation.json). This is a hash comparison
of those files, not an exhaustive inventory of every world/mod/saved game.

Owner instances, worlds, saves, mods, configurations, credentials, local releases,
production binaries and `.minecraft` were not targeted for mutation. No login,
credential-store mutation, owner configuration write, game install/launch,
recursive cleanup, process-name termination or production installation occurred.
All seven worktrees and four protected local artifacts remain. Package/native
versions, release manifests/configuration and lockfiles are unchanged. No push,
merge, tag, workflow dispatch, publication or release preparation occurred.
The review server was stopped by its verified task-owned PID after acceptance.

## L. Remaining issues

- **UI-2 blockers:** none found. Owner visual approval is the next step.
- **Minor limitations:** short windows and non-ready instances with multiple
  blockers need vertical scrolling; the <=800px player threshold is intentional.
  Mixed saved Wide/Small arrangements preserve reading order rather than filling
  every slot. The unchanged missing-skin fallback is intentionally quiet.
- **Pre-existing data integration:** real gameplay bridge/telemetry health was not
  proven by fixture rendering. No backend defect was reproduced in this phase.
  The contradictory empty/error presentation was corrected locally. Future bridge
  investigation should use a separate narrowly scoped reproduction if owner data
  remains absent after actual gameplay. Native synthetic missing inventories
  correctly remain unavailable (dashes), never invented counts.
- **Deferred:** UI-3/UI-4, cape editing, new skin features, new widgets/routes,
  backend/auth/readiness changes and release preparation were not started.

## M. Final recommendation

**UI-2 PASS — ready for owner visual review.** Review the matched Home comparison,
state/accessibility sheets and native minimum/editor captures. Stop at UI-2.
