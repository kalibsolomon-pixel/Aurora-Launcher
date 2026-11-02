# UI-2B.1 — fixed-angle character acceptance

## A. Verdict

**PARTIAL — implementation and native resizing pass; visible character cropping
in a native Ready instance remains unverified.** The native fixture remains
truthfully Installing/blocked. No readiness was fabricated to obtain a picture.
The actual enlarged character is inspected and measured in labeled browser skin
fixtures. UI-2B's outstanding medium/minimum native sizing gap is now closed.

## B. Repository baseline

Branch `codex/client-3-production-authority`; starting HEAD
`3daad85`. Full SHA, tracked inventory/count, worktrees, protected hashes and
existing evidence are recorded in [baseline](../../ui-2b-1/baseline.json).
Only the four expected local artifacts were initially untracked; all seven
worktrees and the other six HEADs are preserved. No tracked deletions.

Implementation commit: `e555c14`. Tests/evidence follow in the delivery commit;
the exact final SHA is reported in the delivery message. Changed paths are in
`docs/ui-2b-1/changed-files.txt`. No native production source/configuration change.

## C. Architecture findings

HomePage owns the shared hero and its functional instance region. PlayerPreview
owns account-keyed avatar selection, validation through playerModel, visibility
at >800px, and event-driven canvas redraws. playerModel.ts owns the software
rasterizer, reusable yaw parameter, default -0.42 radians, existing camera tilt
and arrow rotation helpers. Those renderer functions and their rotation tests
remain intact. Skin acquisition/cache/authentication still belong to the existing
store/native boundary. Previously Home exposed three controls and a 240–320px
stage with a floor and drop shadow.

## D. Changes

Home now calls the existing default orientation directly: yaw -0.42 radians
(approximately -24°), with existing subtle camera tilt and neutral pose intact.
No Home yaw state, rotation/reset buttons, drag listeners or hidden focus targets
remain. A descriptive image region wraps the decorative canvas.

The canvas displays at 480–560px high (512px at an 800px viewport), approximately
1.6× the prior stage. Its 560×800 intrinsic raster remains unchanged; pixelated
sampling preserves skin detail without enlarging beyond its source resolution.
The right half of the hero supplies the clipping boundary, with its bottom edge
and radius aligned to the hero. Only this artwork container clips: the hero and
page retain ordinary overflow so the selector is unaffected. Normal desktop hero
minimum height is 404px; the head and torso are visible, upper legs remain and
lower legs/feet extend beyond the hero crop. Widgets stay outside the clip.

The visible floor and heavy shadow are removed; only a static accent halo remains.
At <=1050px the raster uses 480px; at <=800px the player still yields. Functional
text can grow vertically. When blockers are present the decorative character
yields entirely, avoiding any overlap with the full-width readiness messages.
Missing-skin copy is preserved; accessible labels now distinguish unavailable
skin from the signed-out explanation rather than claiming it is always loading.
Launch, instance information, widgets, navigation and background styling were
otherwise preserved.

## E. Visual evidence

[Matched comparison](../../ui-2b-1/home-comparison.png),
[state comparison](../../ui-2b-1/states-comparison.png), originals in
`docs/ui-2b-1/before/` and `after/`, and measured geometry in each folder.
Matched sizes: 1440×900, 1280×800, 1024×768 and 720×520, DPR 1. Captures include
validated synthetic skin, missing skin, long names, blocked Launch, cyan and
OLED/Simple. These are browser fixtures, not native gameplay evidence.

Actual screenshots were inspected: the character is substantially larger,
three-quarter face and torso are readable, upper legs terminate cleanly at the
hero edge, and Launch remains the primary action. There is no floor/platform or
viewer toolbar. Pixel-bound tests confirm the head is inside the hero and actual
lower-body pixels extend below it. Launch/widgets are unobstructed; Aurora's
original colors and artwork remain recognizable. At the narrow breakpoint the
character disappears, preserving functional space.

Native screenshots and client geometry are in `docs/ui-2b-1/native/`, including
default, maximized, medium and minimum views. They show the real compiled Tauri
frontend and Installing fixture, not the browser review entry.

## F. Automated verification

All final commands exited 0:

| Command | Result |
| --- | --- |
| `node --test --test-concurrency=1 --test-timeout=60000 "src/lib/**/*.test.ts"` | 272 tests, 16 suites, all pass; reusable renderer rotation tests retained. |
| `npm run check` | 0 errors, 0 warnings. |
| `npm run tauri build -- --debug --no-bundle` | Production frontend and debug executable built, no installer. |
| `node tools/visual/ui2.test.mjs` | 17 Home cases; fixed output under this phase. |
| `node tools/visual/ui2-ui1-regression.mjs` | 11 UI-1 cases. |
| `node tools/visual/ui2b.test.mjs` | 6 Borealis cases. |
| `node tools/visual/ui2b1-crop.test.mjs` | 6 additional responsive pixel/geometry cases. |
| `node tools/visual/ui2b1-preserve.mjs` and Git checks | Original paths, critical/protected hashes and other worktree HEADs preserved. |

The Home interaction regression now verifies no controls and unchanged canvas
pixels after pointer dragging; account-switch invalidation still passes.
Renderer-level orientation tests verify the canonical default and reusable
rotation behavior. Existing manual-control assertions were updated for the
owner-approved Home removal, not deleted. An initial new crop harness used an
ambiguous canvas locator (account and player canvases); the final focused selector
passes all six cases. Logs, exit codes and measurements accompany this report.
No new Rust format/check/test pass is claimed. Native build retains the existing
STATIC_VCRUNTIME warning.

## G. Native acceptance

Fresh isolated data and WebView2 profile were created under the marked direct-temp
root in native/start.json. The owner confirmed signing in to this test window;
the agent did not initiate authentication or operate the login UI. The Installing
instance remained blocked by its missing installation/runtime. No game launch or
installation was attempted. Signed-in data in that isolated root was retained.

The previous boundary drags moved client scrollbars rather than establishing
verified sizes. This pass uses a **test-only Win32 window-sizing harness**,
`tools/visual/ui2b1-window.ps1`: exact PID, executable and marked isolated root
are checked before obtaining that process's window handle. Documented
SetWindowPos/ShowWindow APIs resize/maximize/restore that single window, accounting
for window DPI and client/frame differences. No production command, capability,
minimum, Tauri configuration or security policy is changed. The helper lives in
tools/visual and is not bundled. Requested dimensions are recorded separately
from the independent actual WebView measurements.

| Native state | Measured CSS client | DPR / display scale | Horizontal overflow |
| --- | --- | --- | --- |
| Default | 1120×790 | 1.25 / 125% | None |
| Maximized | 2048×1152 | 1.25 / 125% | None |
| Restored, then medium | 1024×768 | 1.25 / 125% | None |
| Minimum | 720×520 | 1.25 / 125% | None |

Window/client physical rectangles and DPI are in `*-request.json`; measured CSS
client rectangles are in the matching capture JSON. Screenshots are from the
actual `http://tauri.localhost/` WebView using CDP read-only capture/measurement.
Default and maximized sizes are measured, not inferred from configuration. Native
Settings/Appearance/Home navigation and Borealis persistence worked. Widgets
remain in normal vertical flow, with one column at minimum size. Launch and
blockers remain readable; the account dock stays within the minimum client.
Process 15392 exited normally with code 0 via a close message to its exact window.

**Unverified:** the visible enlarged character in a native Ready Home. The native
fixture is incomplete; its messages intentionally take priority over artwork.
Browser fixtures verify the populated crop, but that does not substitute for an
actual visible native character screenshot. This is the remaining acceptance
gap, distinct from the now-completed native medium/minimum sizing checks.

## H. Functional preservation

Correct account-keyed avatar selection and invalidation, validation and missing
skin behavior pass existing tests. No renderer, skin acquisition, authentication,
launch authority, registry, widget schema/data source or navigation changes.
Launch still calls the existing typed native command once under repeated clicks;
selector keyboard navigation and focus remain. Widget order/size/hide/add and
persistence still pass the regression suites.

## I. Accessibility and performance

No invisible rotation controls or Home pointer handlers remain. The image label
describes actual availability; canvas is decorative and not focusable. Existing
focus, status, errors, reduced motion/transparency and 200% text checks pass.
No new dependency, animation loop, acquisition, subscription or backdrop filter.
Canvas redraws remain bounded by texture/visibility changes. No performance
benchmark beyond functional/geometry checks is claimed.

## J. Preservation

All original tracked files, seven worktrees and protected local hashes remain.
No prior acceptance evidence was overwritten. Production Launcher 1.4.2, owner
instances/worlds/mods/configuration and published artifacts were not modified.
Owner-initiated sign-in occurred only in the isolated test app; no agent-authored
authentication or credential operation occurred, so authentication cannot be
described as untouched by the owner during this run. The temporary root remains
intact. No push, merge, tag, publish, deploy, release tooling/version change or
UI-3/UI-4 work.

## K. Remaining concerns

No blocking browser visual defect was observed. Native visible-character cropping
on a Ready instance still needs visual acceptance. Blocked Home intentionally
yields the artwork to functional messages. UI-2B's medium/minimum verification gap
is closed. No deferred Appearance workspace or interactive viewer feature was added.

## L. Recommendation

**PARTIAL — specific corrective work required:** confirm the visible cropped
character in an isolated, genuinely Ready native instance, then obtain owner
visual approval. Do not fabricate readiness or begin another UI phase.
