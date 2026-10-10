# Aurora Launcher — pre-release UI corrections

## Owner acceptance update — October 10, 2026

The owner manually reviewed the latest prototype, including scrolling, reports
that everything seems fine, and explicitly requested release. This resolves the
subjective R1 scrolling review gate for final application source
`db078f20d8d2eaa0c632b276e68c63179872acde` and evidence commit
`0e9eb4f473f61a99ccb256f3a0a6c38e051a26f9`. The owner selected Launcher 1.4.2
for the next controlled release.

This acceptance does not replicate the previously reported severe symptom or
validate physical frame-presentation/GPU measurements that were not captured.
The historical diagnostic findings and limits below remain unchanged. The
previous hold-publication recommendation is superseded only for subjective R1
acceptance and release initiation; signed-installer acceptance, exact-source
verification and separately protected publication/authority gates remain required.
See `RELEASE_1_4_2_ACCEPTANCE.md` for current release status.

## Historical executive verdict before owner review: BLOCKED

R2 and R3 are implemented and locally verified. R1 isolates a real backdrop
compositing cost and removes per-result filters, but the owner's severe scrolling
regression did **not** reproduce on this host, including the historical source.
The evidence does not establish that the reported regression is resolved.
L1–L3 remain locally complete; this correction phase is **not release accepted**.
Do not publish on the strength of these visual changes or compositor measurements.

No push, publication, installer execution, production replacement, release,
account sign-in, game launch, or owner-instance mutation was performed.

## Source identities and scope

- Branch: `codex/client-3-production-authority`.
- Starting HEAD: `3aa0ade370667dadadf6de72c5003fee986fa957`.
- Historical comparison: `8df387bd682d7ab6c41206affcf68ba38c51dd0d`, before L1.
- Final application source: `db078f20d8d2eaa0c632b276e68c63179872acde`.
- Earlier material/performance source: `5afbe22e7d52bcdd8559357928881ab8e93271b4`.
  Final captures and `glass-match.json` include the owner-requested glass matching
  follow-up and the tooltip teardown correction.
- The evidence commit containing this report changes only documentation and review
  evidence. Resolve its exact identity with
  `git log -1 --format=%H -- LAUNCHER_PRERELEASE_UI_CORRECTIONS.md`.

Application changes are limited to the installed-mod warning presentation,
shared warning icon/tooltip, workspace content materials and selection/alignment,
and a presentation class on the standalone modpack group. Native Rust,
compatibility rules, command DTOs, account boundaries, artwork acquisition,
dependencies, lockfiles, version 1.4.1 and production identifier are unchanged.

## R1 diagnosis and historical comparison

The starting CSS applies `backdrop-filter: blur(28px) saturate(135%)
brightness(.92)` to the installed list and every browsing result over live
Borealis video. These growing filtered surfaces increase compositor CPU
submission work. Restoring the exact initial content declarations and then
removing only that filter in the same native review build isolates this cost.
The measured result justified the small material correction before R2/R3 work.

This filter already exists in
`c83a9e6c2a324c91a88b8afd5d4b42ccaed0da9a`, before L1–L3. The archived pre-L1
components also scroll without the severe symptom under this fixture. Therefore
the owner regression's origin is **undetermined**; attributing it to L1, L2 or L3
would be unsupported. No demonstrably smooth owner/hardware baseline was available.
The historical source was extracted outside the active checkout, with a separate
loopback review server and a review-only invocation hook. No historical checkout
or production application was replaced. Historical components differ in ancillary
DOM/artwork counts, so they are contextual evidence, not a controlled CSS A/B.

There is no new virtualization, scroll subscription, sorting pipeline, image
cache, rendering dependency or native acquisition change. Keyed rows and fixed
artwork dimensions already exist. Closed tooltips perform no scroll geometry
work; only an open tooltip tracks its position.

## Measurement method and results

Actual Windows Tauri/WebView2 debug review, isolated identifier
`com.aurora.launcher.ui-corrections-review`; 1600×1000 logical viewport, DPR 1.25,
same live Borealis loop, 100 illustrated rows/results, 60 wheel inputs per run
(3000px down and back). Three interleaved starting/R1 trials per each of five
surfaces, then three historical and three C trials per surface.
There are 8,343 callback-interval samples in the controlled A/B, 4,209 historical,
and 4,152 at C. These are short repeated diagnostics, not endurance testing.
The host reports a Radeon RX 9070 XT, driver 32.0.31041.1004 and 2560×1440/240Hz
display mode; callback cadence must not be equated with that display mode.

All table cells below are milliseconds. Each cell summarizes the median of the
three per-trial statistics, rather than hiding tails in average FPS.

| Surface | Starting callback median / p95 / p99 | R1 callback median / p95 / p99 | C callback median / p95 / p99 |
| --- | --- | --- | --- |
| Installed mods | 6.9 / 7.2 / 7.2 | 6.9 / 7.2 / 7.3 | 6.9 / 7.2 / 7.2 |
| Browse mods | 6.9 / 7.2 / 7.3 | 6.9 / 7.2 / 7.2 | 6.9 / 7.2 / 7.2 |
| Resource packs | 6.9 / 7.2 / 7.3 | 6.9 / 7.2 / 7.2 | 6.9 / 7.2 / 7.2 |
| Shaders | 6.9 / 7.1 / 7.2 | 6.9 / 7.2 / 7.3 | 6.9 / 7.2 / 7.3 |
| Modpacks | 6.9 / 7.2 / 7.2 | 6.9 / 7.2 / 7.3 | 6.9 / 7.2 / 7.2 |

Historical callback medians are also 6.9ms (one individual run is 7.0ms), with
p95 7.1–7.2ms and p99 7.1–7.3ms. No callback over 25ms, long main-thread task,
or Chromium-marked janky scroll frame was found in the valid controlled trials.
This supports **no FPS/stutter improvement claim**, because the baseline already
meets the observed cadence here. rAF is a callback, not proof of physical delivery.

| Surface | Historical draw median | Starting draw median / p95 / p99 | R1 draw median / p95 / p99 | C draw median / p95 / p99 |
| --- | --- | --- | --- | --- |
| Installed mods | .304 | .307 / .413 / .662 | .250 / .355 / .646 | .247 / .329 / .631 |
| Browse mods | 1.128 | .862 / 1.081 / 1.289 | .282 / .416 / .479 | .248 / .322 / .420 |
| Resource packs | 1.128 | .884 / 1.121 / 1.297 | .398 / .628 / .672 | .246 / .364 / .591 |
| Shaders | 1.501 | .839 / 1.131 / 1.918 | .269 / .416 / .540 | .247 / .451 / 1.928 |
| Modpacks | 1.043 | .786 / 1.121 / 1.337 | .246 / .358 / .530 | .253 / .385 / .889 |

`Display::DrawAndSwap` measures compositor **CPU submission work**, not elapsed
GPU rendering time or GPU utilization. The controlled median reduction is about
19% installed and 55–69% across browsing surfaces. The C medians remain low after
the initial shared backing; shader p99 remains noisy (1.928ms), so there is no
claim that every tail improves. C geometry changes make the original R1 A/B the proper
causal comparison. No hardware GPU timer or final physical presentation trace
was collected.

| Surface | Starting input-to-frame-swap p95 | R1 p95 | C p95 |
| --- | --- | --- | --- |
| Installed mods | 14.077 | 12.389 | 12.653 |
| Browse mods | 13.587 | 11.651 | 11.910 |
| Resource packs | 14.119 | 12.376 | 11.996 |
| Shaders | 13.837 | 13.174 | 13.559 |
| Modpacks | 12.994 | 13.007 | 12.550 |

Frame swap excludes final screen presentation. Modpack latency did not improve
consistently in the controlled A/B; do not market the compositor percentages as
input-latency or FPS percentages. Individual trials and sample counts are retained
in `docs/evidence/ui-corrections/{r1,historical,final}.json`.

The owner's subsequent glass feedback is measured in `glass-match.json`: three
interleaved runs for each of the same five surfaces, comparing the previous
unfiltered backing with the final single glass background layer. Geometry,
inventory, live video, viewport, DPR and wheel sequence are held constant.
There are 4,073 previous-backing and 4,063 shared-glass callback samples.

| Surface | Previous draw median / p95 / p99 | Shared glass draw median / p95 / p99 | Previous / shared input-to-swap p95 | Shared callback median / p95 / p99 |
| --- | --- | --- | --- | --- |
| Installed mods | .274 / .373 / .447 | .345 / .457 / .624 | 12.655 / 13.839 | 6.9 / 7.1 / 7.2 |
| Browse mods | .278 / .421 / .497 | .347 / .476 / .639 | 12.752 / 13.340 | 6.9 / 7.2 / 7.2 |
| Resource packs | .273 / .402 / .471 | .347 / .485 / .551 | 12.949 / 13.619 | 6.9 / 7.1 / 7.2 |
| Shaders | .280 / .419 / .496 | .338 / .458 / .514 | 13.049 / 13.630 | 6.9 / 7.2 / 7.3 |
| Modpacks | .275 / .356 / .426 | .343 / .447 / .541 | 12.760 / 12.536 | 6.9 / 7.2 / 7.3 |

Restoring the established frost adds .058–.074ms of median compositor CPU work
over the unfiltered backing. Input-to-swap p95 is higher in four views and lower
in modpacks; this is a visual consistency tradeoff, not a further performance
improvement. All 30 runs have no long task, callback over 25ms or Chromium-marked
janky scroll frame. One previous-backing installed trial records 44 settling
mutations; the other 29 runs record zero. The final glass has no per-result
filters. These short diagnostics still do not reproduce or prove resolution of
the owner's severe symptom. The original R1 percentages are not final-glass
improvement percentages, and physical presentation/GPU timing remain unmeasured.

## Artwork, cold/warm caches and rendering attribution

Separate real-native acquisition diagnostic at C (before the glass matching
follow-up): a fresh disposable managed root,
20 public Modrinth projects from the existing L1 audit behind 100 synthetic rows,
with initial resolution overlapping wheel scrolling. No product installation or
owner inventory was involved. Cold means Aurora's artwork disk cache was absent;
it does not mean upstream CDN/OS network caches were flushed.

| C candidate scenario | Acquisition median / p95 | Callback median / p95 / p99 | Samples | >25ms / long tasks |
| --- | --- | --- | --- | --- |
| Cold Aurora artwork cache | 474.6 / 754.1 | 6.9 / 7.1 / 7.2 | 279 | 0 / 0 |
| Warm Aurora artwork cache | 47.7 / 108.2 | 6.9 / 7.2 / 7.3 | 286 | 0 / 0 |

All 100 initial resolutions returned available; 20 canonical cache objects were
created, and warm object SHA-256s remained byte-identical. All visible images
decoded. Total decoded images at the sampling endpoint were 96 cold / 100 warm;
offscreen images retain the existing lazy-loading behavior. The synthetic aliases
deliberately issue 100 initial calls against 20 real identities; they are not a
model of a user's identity inventory. No extra artwork resolutions occurred
during the settled fixture scroll/filter checks. Cache reuse is established by
the existing Rust read-cache path and unchanged objects, not a packet capture.
This real cold/warm check covers the C installed-list fixture, before the later
glass matching change; the native acquisition/cache implementation is unchanged. Browsing
comparisons use settled deterministic artwork; starting/historical cold networking
and cold live browsing in all four categories were not measured.

The controlled A/B retained identical node counts: 2,731 installed, 1,976 for
mods/resource packs/shaders, 1,776 modpacks. It recorded zero scroll DOM mutations
and no long tasks. C installed DOM is 3,206 because each warning has its
accessible popup content; C browse counts are unchanged. One C installed
run recorded 44 settling mutations, the other 14 C runs zero. No claim of zero
updates during initial artwork acquisition is made.

Representative mods traces show 16 lazy image decodes before and after R1
(0.603ms versus 0.538ms in ImageFrameGenerator), 14 at C (0.369ms), and comparable
style/layout lifecycle CPU totals (0.800 / 0.826 / 0.674ms). This does not suggest
repeated whole-list image decoding as the bottleneck. Video decoding continues.
Removing backdrop filters changes raster behavior too: sampled raster operations
increase from 36 to 240. Thus this is **not** a measured reduction in every CPU or
GPU category. Renderer heap/layout counters are retained in the receipts; these
short runs do not establish memory-leak or garbage-collection endurance results.
`render-attribution.json` records the selected event counts/durations. Raw traces
and invalid/interrupted exploratory runs remain outside Git and are not accepted
measurement evidence.

## R2 warning presentation

Collapsed rows now place one amber triangle immediately after the version,
using Aurora's icon and shared `.f-tooltip` material. Every native warning is
preserved in a grouped list, with full wrapping, accessible label/description,
hover/focus access, click/tap pinning, outside dismissal and Escape. A native
top-layer popover prevents row clipping; geometry clamps to the viewport.
Escape keeps keyboard focus on the trigger. Only one warning popup is active.
Long titles can wrap; no fixed clipping height is introduced. Expanded details
retain their full diagnostics. Equal-length collapsed fixture rows have equal
heights regardless of warning count.

Native warning codes/messages are consumed unchanged. Synthetic blocked-toggle
feedback remains an action-level alert and leaves the switch enabled; native
dependency enforcement remains covered by the unchanged Rust test suite.

During shutdown after the first evidence commit, the review log exposed a delayed
popover toggle event after workspace navigation had cleared the bound popup.
A pinned-popup → Settings regression test reproduced the null-binding error.
The handler now reads the event's stable current target. The same check passes in
both headless Chromium and actual native WebView2, including zero page errors.
The full frontend suite, type check, frontend build and optimized native smoke
were refreshed for this focused follow-up; Rust sources are unchanged.

## R3 readability, selection and responsive alignment

The owner subsequently supplied a screenshot of the standalone modpack panel
and requested that the new panels match the established blurred UI. The first
correction's dark, unfiltered 82% backing did not match the existing groups.
The workspace now uses exactly the same `--f-panel`, `--f-blur`, `--f-edge`,
`--f-shadow` and radius as existing panels. The standalone modpack group inherits
its ordinary group material again, through the same background layer. Native
computed-style checks compare the background, filter, border, shadow and radius
directly against existing groups.

One enclosing panel background layer supplies the frost; installed lists and
individual browser results remain unfiltered, with a light secondary fill. This
preserves removal of per-result filters while restoring familiar diffusion.
The layer avoids creating a new fixed-position containing block, so browse notices
stay anchored to the viewport while scrolling (verified in all three installable
browser categories using a synthetic unavailable response). The
additional shared filter is measured separately in `glass-match.json` below.
Reduced transparency and unsupported filters retain solid fallbacks.

The previous 4.5:1 fully-white-underlay bound applied to the discarded 82% backing
and is **not** a claim about this restored glass material. Readability was visually
checked on the actual bright/dark Borealis captures; this does not certify
contrast against arbitrary backgrounds. Existing glass tokens were retained as
the owner requested, rather than darkening all established panels.

Selected instance/view/category controls share a turquoise token, tinted fill
and 3px inset marker, with existing ARIA state and font weight. This changes no
persisted accent preference; global customizable accents still govern the shell.
Shared 24px padding (16px at narrow widths), wrapping headings/header actions,
and 40px view/category controls keep content aligned and usable.
Native tests pass at 1600×1000, 900×930 and the minimum 720×520. No horizontal
overflow or clipped tooltip was found. No per-result blur or row animation is added.

## Visual and actual desktop acceptance

Actual isolated WebView2 captures, not browser mockups:
`docs/evidence/ui-corrections/before/` and `after/`: 14 corresponding views, plus
one final standalone comparison matching the owner screenshot. Final after images
replace the earlier C captures, which remain recoverable from commit D.
`captures.json` records viewport/DPR/scroll position with no horizontal overflow.

| Capture name | State |
| --- | --- |
| 01-installed-top | Collapsed installed rows, mixed warning counts |
| 02-installed-middle | Same 2200px scroll offset; denser corrected rows |
| 03-warning | Single warning |
| 04-multiple-warnings | Multiple warnings with long wrapping text |
| 05-browse-mods | 100 illustrated results |
| 06-browse-resourcepacks | 100 illustrated results |
| 07-browse-shaders | 100 illustrated results |
| 08-browse-modpacks | 100 illustrated results |
| 09-navigation-bright | Representative video position 3s |
| 10-navigation-dark | Representative video position 12s |
| 11-installed-narrow | 900×930 installed content |
| 12-browse-narrow | 900×930 browsing content |
| 13-settings | Instance settings regression |
| 14-cosmetics | L3 Skins & Capes regression |
| 15-standalone-glass-match | Final modpack panel beside existing instance/create panels |

All original before images and all 15 refreshed after images were visually
inspected; warning and narrow views were additionally examined at full size. No
overlap, clipped text, broken artwork, horizontal overflow or inconsistent
collapsed warning density was found. The initial pasted request referred to two
Installed Mods images that were not attached. The later supplied standalone
modpack screenshot was inspected and directly informed the material correction.
Native viewport receipts retain any one-pixel Windows scaling roundoff.

Native desktop input was exercised through the Windows computer-use skill:
wheel scrolling, keyboard return to the top, warning click and Escape. Snapshots
showed stable artwork and the contained grouped warning. Native CDP checks also
verified focus retention, hover/click dismissal, filtering/sorting, scroll
position, resize behavior, and action-level errors. Instrumentation and snapshots
do not substitute for the owner's subjective smoothness acceptance on the failing
setup. The severe symptom remains unverified.

The final optimized unbundled executable was copied into the disposable review
directory and booted with the actual production frontend at `http://tauri.localhost`,
not the fixture server. Its built-in identifier is
`com.aurora.launcher.ui-corrections-optimized-review`; the Rust status confirms
the separate managed root, backend ready, version 1.4.1 and empty registry with no
selected instance. Actual native inspection showed the empty Home and signed-out
state, with zero page errors. Receipt and image: `optimized-smoke.json` / `.png`.
WebView2 reports 154.0.4258.62. Executable SHA-256:
`aec77011ada83f47775bb032d998d4b7cfebbf38f6192423090887b57c0a93b9`.
No installer/bundle was built or executed. The build emits the existing
STATIC_VCRUNTIME deprecation warning. This empty-state smoke establishes startup
and real backend isolation; the content stress acceptance uses the debug fixture
window, not this empty optimized registry.

## Automated verification

| Check | Result |
| --- | --- |
| `npm run check` | Pass, zero errors/warnings |
| Complete frontend suite, serial | 265 passed, 16 suites, zero failed/skipped |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Pass |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | Pass; four pre-existing test-helper warnings |
| Complete Rust suite | 854 library + 6 icon integration passed; zero failures; **29 ignored, not claimed passed** |
| Production frontend build | Pass, static output generated |
| Isolated optimized native build, `--no-bundle` | Pass, optimized release profile; final glass source |
| New UI correction native tests | Pass: single/grouped/long warnings, accessibility/dismissal, pinned-popup navigation teardown, warning persistence, action feedback, scroll/artwork reuse, viewport-anchored browse notices, 100 rows/four browsers, shared glass equality, minimum/narrow/wide layouts, reduced transparency, standalone modpack ancestor |
| Existing L1 visual integration | Pass for all four content kinds, retry/fallback, filters/actions and responsive long metadata |
| Existing L3 visual integration | Pass at 1600×1030, 1120×760, 860×800, 720×520; cape confirmation, account isolation, widgets and dialog focus |
| Existing correction harness | Pass: selector/accounts, sidebar containment, artwork DTO/fallback and settings capability gate |

The full Rust suite includes Modrinth, installed-mod/dependency and artwork cache,
format/normalization/security tests. Live ignored tests were not enabled: this
request forbids product installation and release, and the deterministic suite is
the regression authority. Real artwork-only acquisition was separately exercised.
An initial frontend run was interrupted after fixture startup timeouts; complete
serial runs subsequently passed. The first teardown-follow-up frontend run also
hit fixture startup timeouts while the review server was active; a full serial
retry with the review server stopped passed all 265 tests. The final glass source
also passes the complete 265-test suite, type check and frontend production build.
A first Rust attempt was blocked by the running review executable's Windows file
lock; after closing that exact review app, the complete retry passed. Incomplete
attempts are not counted as passes. Rust and the existing L1/L3 integration logs
are retained from C; Rust and those other page implementations are unchanged.

## Preservation and local commits

Initial status, HEAD/history, tracked set, manifests/docs/lockfiles and private
hash inventories were recorded outside Git. Baseline: 753 tracked files, 15,144
pre-existing untracked files, 175 protected ignored files, and 27,273 owner files.
Checks before A, B and C found no missing file and zero protected changes. Only
the four scoped pre-existing application files changed. Review apps/caches use
new isolated identifiers/disposable roots; `.minecraft` and owner content are
untouched. Raw inventories, credentials, private addresses and account details
are not committed. No broad cleanup was run.

Final pre-evidence-commit verification rehashed the complete baseline: zero
missing files, zero changed protected/untracked/owner files, and exactly the four
scoped tracked application modifications. `receipts.json` contains sanitized
counts/check results; private path/hash inventories remain outside Git.

| Commit | Purpose |
| --- | --- |
| `e89181dc25e5b43f879d06cbd85088b052886729` | A — controlled scrolling material correction and profiling evidence |
| `4210369eaa458911cb1575792f41362448054326` | B — compact metadata warnings and accessibility tests |
| `5afbe22e7d52bcdd8559357928881ab8e93271b4` | C — workspace readability/alignment/selection and visual tests |
| `79597ef22d9471ca474a5487d3f46f306beed225` | D — sanitized acceptance evidence, capture/cache tooling and report |
| `4ab53f1c7bd26550f65de678a4b4e2261ed8a8fe` | E — fix the reproduced delayed warning-popover teardown error and add navigation regression |
| `db078f20d8d2eaa0c632b276e68c63179872acde` | F — match the owner-referenced established glass while retaining unfiltered rows |
| Report's containing commit (lookup above) | G — refresh final style measurements, captures and acceptance receipts |

The suggested A→B→C→D order was followed. C builds on A's shared content material;
the warning presentation is a separate change. Review-tool extensions in D do not
alter the production bundle. E fixes the reproducible teardown defect found while
closing the review after D.
F applies the owner's later screenshot feedback, and G records the final evidence.

## Rollback and release recommendation

Keep these commits local. For a full rollback, use ordinary reviewed `git revert`
commits in G→F→E→D→C→B→A order; obtain G from the report-path log above. For a targeted
warning rollback, revert E then B and inspect any test expectations. For a targeted
material rollback, inspect the shared CSS changes in A/C/F together and run the
visual/type checks. Do not reset the checkout, clean untracked files or remove
instance/user data. Reverts affect source only; they do not touch owner caches or
installed applications.

Release recommendation: **hold publication**. Obtain a reproducible failing
scroll scenario on the owner's affected setup (window scale, real inventory and
background settings), then compare this candidate with the same failing baseline
and capture physical presentation/GPU evidence if needed. The remaining issue is
R1 acceptance, not missing permission to push. No publication is authorized.
