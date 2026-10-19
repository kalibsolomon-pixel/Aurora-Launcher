# Aurora Launcher — Premium Design audit and refinement report

**Status: implemented, natively validated on Windows, committed locally. Not pushed, merged or published.**
Owner visual approval is not claimed. No real authenticated Minecraft launch was performed (see N).

## A. Executive summary

The launcher had already been through Phase F, UI-1…UI-5 and several correction passes, so this
initiative consolidated rather than redesigned. A full native audit — fixture-driven *and* a real
Rust backend in an isolated data root — found that the remaining problems were mostly **not on the
ideal populated screens** but in transitions, first run, install and error states:

- raw internal identifiers shown to users (`installingGame`, runtime and launch phases);
- a "busy… wait, then retry" recovery warning and a Retry button shown *during the launcher's own
  instance creation*;
- a five-line raw Java error containing a private filesystem path;
- a workspace header saying **Ready** while Play was blocked;
- blocked Home launch states conveyed only by a coloured dot;
- an unframed first-run Home, misaligned widget empty states, Launch truncating a 14-character name;
- duplicated controls (two Rename buttons), an orphaned tool row, inconsistent dialogs (one with a
  full-window blur and an undefined `--shadow-lg` token), and decorative extra outlines.

All of these are fixed; shared interaction feedback was consolidated; performance is unchanged
within measurement noise. 293/293 frontend tests and 999/999 Rust tests pass; `npm run check`,
`npm run build` and `cargo fmt --check` are clean (details in K).

## B. Premium Design skill application

The installed `anthropic-skills:premium-design` skill was loaded in full before the audit. It
directly drove these decisions:

| Skill rule | Applied as |
| --- | --- |
| Specify exact transition properties, never `all` | `.btn`, rail, icon buttons, account chip, settings nav now list `background-color`/`color`/`border-color`/`transform` |
| Pressables scale ~0.97 on `:active` | Buttons, dialog buttons, icon buttons, account chip (0.97); rail items (0.96). Replaced the previous `translateY(1px)` |
| Tooltips: delay first, instant afterwards; 125–200 ms; origin-aware | Rail tooltips fade/slide 125 ms ease-out from their left edge; instant while moving along the rail |
| Modals 200–500 ms, centered origin, never from `scale(0)` | Shared dialog entrance: `opacity 0 + scale(.97)` → 1 in 180 ms ease-out, centered |
| Reduced motion = fewer/gentler, not zero | Movement removed; dialogs keep a 120 ms fade |
| Never animate high-frequency keyboard actions | Keyboard category/tab navigation remains unanimated |
| Only animate transform/opacity | No new layout-property animations; no new blur surfaces |

**Reconciliation with owner decisions** (owner decisions win; underlying usability principle still applied):

| Skill/generic suggestion | Owner decision found | Resolution |
| --- | --- | --- |
| State readiness in text | `df5d9da` "Simplify Home launch card and inline readiness dots"; test asserts no visible `Ready` text | Ready stays the quiet dot. Only *blocking or delaying* states (Needs attention, Checking…) get a text line — color is never the sole signal for a problem |
| Outlined destructive buttons for clarity | Owner prefers neutral buttons without decorative outlines (Client initiative) | Danger buttons lose the outline and gain a soft error fill |
| Remove accent from selection | `c0c9342` owner-requested page-wide selection accent | Accent fill + bar kept; only the redundant third treatment (outline) removed from Settings categories |
| Fewer tab levels in Mods | UI-3/3.1 owner-approved Discover geometry | Left unchanged |
| Rail glass and Borealis accent glass | Owner-requested glass matching (pre-release corrections) | Left unchanged |
| Larger Home hero / denser composition | UI-2B / UI-2B.1 hero and character crop | Composition kept; only the Launch control width (320→400 px, same 80 px height) changed so names fit |

## C. Interface inventory

Audited from source and natively unless marked *source only*.

| Area | Surfaces and states |
| --- | --- |
| Shell | Custom 40 px title bar + window controls, 96 px icon rail with tooltips, account chip (signed in / signed out / reauthentication), Borealis video + Simple/OLED backgrounds, scroll containment, minimum 720×520 |
| Home | Selected-instance hero, Launch + instance listbox, readiness dot, blockers/feedback, player preview (skin / no skin / signed out), update notice, other-instance-running notice, empty (no instances / none selected), loading, state error |
| Home widgets | Instance Details, Content Summary, Session, Playtime, Recent Worlds, Recent Servers, Skin Library, Cape Selector; edit mode, add tray, drag/arrow reorder, sizes, empty states |
| Instances | Existing-first and creation-first layouts, rows (Ready/Selected/Installing/Recovery/Damaged/Needs install), Modrinth modpacks browser, creation form (platform, loader, Aurora Client, snapshots), live install progress |
| Instance workspace | Header (breadcrumb, badge, Rename/Open folder/Play), Overview readiness rows, Mods (Installed/Browse, recognition, updates, rows, removal confirmation), Resource Packs, Shaders, Settings (General/Performance/Java/Display, Maintenance, Delete) |
| Dialogs | Account drawer, Rename, Delete (name confirmation), widget reset, Appearance import/rename/apply/cape/remove |
| Appearance | Skin/cape library, 3D stage, account actions (fixture accounts) |
| Settings | Appearance, Home & Widgets, Launcher, Minecraft & Java, Updates, Discord & privacy, Accessibility, About |
| Developer | Development-only pipeline page (*source only*; stripped from production) |

## D. Design-system refinements

- **Danger button** (`app.css`): transparent border + `--color-error-soft` fill; stronger fill on hover.
- **Interaction motion** (`phase-f.css`): exact property lists, 0.97 press scale, rail tooltip timing,
  reduced-motion coverage for every new transition.
- **Shared modal** `.aurora-dialog` (`phase-f.css`): the UI-4 Appearance dialog material (panel
  gradient, edge, 22 px radius, `--f-dialog-dim` backdrop, shadow) now used by Rename, Delete,
  widget reset and Appearance dialogs. Removes a full-window `backdrop-filter` on the Rename
  backdrop and an undefined `--shadow-lg`.
- **Widget empty voice** `.widget-empty`: one size/colour/rhythm for all widget empty messages.
- **Content tools row** `.content-tools`: recognition + update check share one row for Mods and packs.
- **Presentation helpers** (`instanceStatus.ts`): `instanceProgressLabel`, `runtimeProgressLabel`,
  `launchProgressLabel`, `workspaceBadge`, and an optional `technical` field on status decisions.
  Counts are native counts verbatim — no percentages or estimates are derived.

No new framework, dependency, token family or styling layer was introduced.

## E. Screen-by-screen changes

| Surface | Problem found | Change |
| --- | --- | --- |
| Home – readiness | Blocked/checking states only visible as a dot colour | Text line for Needs attention / Checking…; Ready unchanged |
| Home – blockers | Each blocker followed by its own "Technical details"; no next step | One list; each known code offers its existing command (Install Java, Accounts, Open instance); one technical disclosure; unknown codes offer only Check again |
| Home – Launch | "Premium Revi…" truncated at 1120 px | Control 400 px wide, same height |
| Home – no skin | Tiny floating sentence | Quiet mark + text; signed-out adds a Sign in action (not nested inside the image role) |
| Home – first run | Unframed text above card widgets | Empty states use the launch panel, so first run matches daily composition |
| Home – edit bar | Opaque surface unlike every other panel | Panel material, control radius, shadow |
| Widgets | Empty messages at different sizes/offsets; Playtime range tabs with no data | Shared empty style; range hidden until any playtime exists |
| Rail account chip | "Not signed in / Sign in to play Minecraft" wrapped to 5 lines | Rail shows identity only; reauthentication keeps a warning line; full state stays in the accessible name and drawer |
| Instances – rows | Actions wrapped below on most rows but not the selected one; three equal-weight buttons | Aligned trailing action cluster; Validate is a quiet button |
| Instances – creation | Raw `installingGame` badge and status; header 25 px off the centered column | Human progress text with item counts; header aligned to the column |
| Instances – own install | "Installation busy… then retry" + Retry while creating | Live creation progress wins over its own held lock; Retry hidden while creating |
| Workspace header | "Ready" while Play was blocked | "Needs attention" when Rust's play decision for this exact instance blocks launch while content is fine |
| Overview – Java | Raw error with private path in red over 5 lines; raw runtime/launch phases | One-sentence summary + Technical details disclosure; human phase labels |
| Instance Settings | Duplicate Rename; Maintenance mixed buttons and disclosure triangles in one wrapping strip; description squeezed to ~90 px at 1600 px; `vanilla` lowercase | Rename only in header; labeled Maintenance rows; readable measure; "Vanilla" |
| Mods / packs | "Recognize local files" orphaned on its own row | Shares a row with the update check |
| Account drawer | Remove looked different in selected vs. other rows | Consistent soft-fill danger buttons |
| Settings | Category labels unreadable over bright aurora bands; triple selection treatment | Quiet unfiltered backing on Borealis; accent fill + bar without outline |
| Dialogs | Two visual systems, full-window blur, undefined token, unlabeled Delete dialog | One shared dialog; Delete has `aria-labelledby` |

## F. Navigation and usability

Task paths were shortened without new navigation: a blocked Launch now resolves from Home in one
click (Install Java / Accounts / Open instance), and a signed-out user can sign in from the skin
area. Rename has one location. No destinations, routes or tab structures were added or removed;
the no-Accounts-tab model and bottom-left account entry are preserved.

## G. Responsive behavior

Native WebView2 at 125% Windows scaling (DPR 1.25) with exact CSS viewports **720×520 (minimum),
1120×760 (default) and 1600×1000**, 16 surfaces each, before and after: **0 of 96 captures had
horizontal overflow**. The Instances list keeps its action cluster on one line at the default size;
Maintenance rows wrap actions below rather than squeezing text (fixed in the report commit after the final 1600 px capture showed the squeeze; the final `after-*` set includes it). *Not validated:* 100%/150%/200%
scaling, physical 1440p/4K monitors, maximized-window snapping; browser-only checks are not
counted as acceptance.

## H. Accessibility

- Problem states are stated in text, not colour alone (Home, header badge).
- Placeholder action is no longer inside a `role="img"` container.
- Delete dialog labelled; dialogs keep native focus containment and Escape.
- Keyboard pass (native): title-bar controls, rail items and account chip all report
  `:focus-visible` with a 2 px accent outline. One sample of the dev-only Developer rail item did
  not report focus-visible; not investigated further (stripped from production builds).
- Reduced motion: every new transition/animation is covered; dialogs fade only.

## I. Performance

Method: same native debug WebView2 window (1120×760, DPR 1.25, 144 Hz display), same fixture data,
baseline source (`ce77839`) vs. final source served alternately on the same port; 3 runs each;
5 s idle `requestAnimationFrame` sampling, DevTools `Performance.getMetrics`, 32 rail-navigation
cycles (click → two frames), 10 account-drawer opens. Raw JSON: `docs/premium-design/performance/`.

| Metric (median of 3) | Before | After |
| --- | --- | --- |
| Home idle frame p95 / max | 7.1 / 7.2 ms | 7.1 / 7.2 ms |
| Frames > 33 ms (any page) | 0 | 0 |
| Idle layouts / style recalcs over 5 s (Home) | 0 / 0 | 0 / 0 |
| Idle task time over 5 s (Home) | 47.9 ms | 49.7 ms |
| Rail navigation p50 / p95 | 13.5 / 13.7 ms | 13.5 / 13.7 ms |
| Account drawer open p50 | 3.7 ms | 3.8 ms |
| Home DOM nodes | 873 | 897 |
| JS heap (all pages) | ~13.9 MB | ~16.3 MB |

Interpretation: frame pacing, idle work and responsiveness are unchanged within noise. Home has
24 more DOM nodes (placeholder mark, readiness line). The JS heap is ~2.5 MB higher **on every
page, including pages with identical node counts**, which points to the two separate Vite dev
servers' module graphs rather than per-page UI cost — but this was not proven, and production-build
heap, cold-start-to-usable and process CPU/RSS were **not** measured. No improvement is claimed.

## J. Native screenshots

- `docs/premium-design/native/before-<size>-<surface>.png` and `after-<size>-<surface>.png`:
  48 + 48 matched full-resolution WebView2 captures (16 surfaces × 3 sizes), plus `*-metrics.txt`
  (viewport, DPR, overflow) and `after-1120x760-keyboard-focus.png`.
- `docs/premium-design/native-real/`: real Rust backend, isolated data root — first run before/after,
  creation form, live install (3 s / 15 s / 45 s), install complete, Needs attention before/after,
  Java install/failure before/after, Overview, Rename and Delete dialogs before/after.

Fixture captures use the review harness (synthetic DTOs, labelled "Visual fixtures"); real-backend
captures are actual native behaviour. Mockups were not used as evidence.

## K. Functional regression testing

| Check | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| Frontend suite (serial, as UI-4/UI-5) | **293/293** pass (`checks/frontend-tests-final.txt`) |
| New `premiumDesign.test.ts` | 9 tests / 14 behaviours: readiness wording, signed-out action, rail identity, single Rename + Maintenance, motion/danger rules, progress labels and own-lock precedence, Java summary + header badge, first-run panel + Playtime, blocker actions |
| `npm run build` | exit 0 (`checks/npm-build.txt`) |
| `cargo fmt --check` | exit 0 |
| `cargo test` | **999 passed, 0 failed** (first attempt could not relink while the review app held the debug exe; re-run after closing it) |
| Real backend, isolated root | Fresh config created; Vanilla 1.21.11 instance created and installed to **Ready**; selection; Needs attention with real blockers; managed Java install executed; Rename/Delete dialogs opened and cancelled (never confirmed) |

**Not performed:** authenticated launch and Ready → Starting → Running → Quit → Ready; Modrinth
mod install/enable/delete on a real instance; widget persistence across restart on the real
backend. Prior phases' acceptance of those flows is unaffected by the code paths changed here, but
it was not re-run.

**Observed backend behaviour (unchanged, for the owner):** in the isolated root (a ~250-character
path) the managed Java `java -version` diagnostic failed to load `jvm.dll`, so Overview reports
the runtime as Damaged, while play readiness accepted the runtime (only the account blocker
remained). This is most likely the unusually long test path, but it means the readiness runtime
check and the explicit diagnostic can disagree. It was not investigated further.

## L. Compatibility

Windows 11 (WebView2) validated natively. **Linux not validated** — no Linux environment was
used; no platform-specific code was added (CSS and Svelte only, `:has()` already in use).

## M. Repository state

- Branch `codex/launcher-premium-design`, created from `codex/native-resolution-readiness`.
- Starting HEAD `ce77839726a009025e94bf25f8526fe4a153a26a`; commits `0d54cde`, `dd09dd0`,
  `1f63874`, plus the report commit (resolve with `git log -1 --format=%H -- LAUNCHER_PREMIUM_DESIGN_AUDIT_REPORT.md`).
- Changed: 23 files under `src/` (CSS/Svelte/TS only, 1 new test file), this report, `docs/premium-design/`.
  No Rust, DTO, command, error code, config schema, identifier, manifest, dependency, lockfile,
  release or update-authority change.
- Protected untracked files (`.zcode-icon-diag.py`, `.zcodeignore`, the Phase D evidence text
  file, two `__pycache__` folders) unchanged; SHA-256 baselines in `docs/premium-design/baseline.txt`.
  No tracked deletions or renames. Seven pre-existing worktrees untouched.
- All native runs used isolated identifiers (`com.aurora.launcher.premium-design-review`,
  `…-isolated`) and `AURORA_DIAGNOSTIC_DATA_ROOT`/WebView2 folders in the session scratchpad.
  No owner instance, account, credential, configuration or `.minecraft` was read or modified.
- Nothing pushed, merged, tagged or published.

## N. Remaining limitations

- No authenticated launch / running-state acceptance (needs owner go-ahead to use a real account).
- Linux, other DPI scales and physical high-resolution displays unverified.
- Appearance (UI-4) and Discover (UI-3) interiors were inspected but intentionally left unchanged.
- Production-build performance and startup not measured; dev-server heap difference unexplained.
- Historical Playwright harnesses (`tools/visual/ui*.test.mjs`) were not run (Playwright isn't
  installed here); `ui2b*` geometry checks may expect the former 320 px Launch width.
- Readiness-vs-diagnostic Java disagreement (K) left for a backend-owned investigation.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
