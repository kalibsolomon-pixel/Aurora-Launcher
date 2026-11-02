# NATIVE initiative — acceptance report

This is the historical NATIVE-6 report. Current continuation implementation,
Windows acceptance and remaining limits are recorded in
[docs/native-ui/NATIVE_8_ACCEPTANCE.md](NATIVE_8_ACCEPTANCE.md); NATIVE-7 records remain linked
there. Historical resource numbers below retain their original methodology.

**Classification: PARTIAL — WORKING NATIVE IMPLEMENTATION**

A native Rust/Slint Aurora Launcher runs the core workflows end to end over the
same Rust core as the production WebView launcher — create an instance, install
Minecraft + Fabric + Aurora Client 3.0.0, install managed Java, sign in with
Microsoft in the system browser, launch, reach the Aurora main menu, enter a
world, exit cleanly and return to Ready — while using roughly a quarter of the
WebView launcher's memory and no idle CPU. It is not full parity: several
WebView features have no native UI yet, and native self-update is blocked
pending an owner decision on update authority.

Date: 2026-10-31. Owner visual acceptance has **not** happened.

## 1. Identity

| Item | Value |
| --- | --- |
| Start | `9b99db3ac226854c8ae96a86eed4c8d3084defa7` on `codex/spxctra-ui-refinement` (latest dev line incl. the Spxctra refinement; `v1.5.1` is released from `origin/main`, which this branch does not contain) |
| Worktree / branch | `C:\Dev\aurora-launcher-native`, `codex/native-ui-migration` |
| Commits | `713ec04` host-neutral services · `0eb08ea` NATIVE-0 prototypes and measurements · `b7674fb` native Slint host · this report (final commit) |
| Selected framework | Slint 1.18.1, winit backend, **software renderer**, Royalty-free License 2.0 with `AboutSlint` attribution |
| Native binary | `native/target/release/aurora-native.exe`, 23.1 MB (Tauri: 24.5 MB) |

## 2. Milestones

| Milestone | Status | Evidence |
| --- | --- | --- |
| NATIVE-0 architecture & framework selection | **Complete** | [BENCHMARKS](BENCHMARKS.md), [FRAMEWORK_DECISION](FRAMEWORK_DECISION.md) |
| NATIVE-1 Rust service boundary | **Complete** | `host.rs`, `application_tauri.rs`; core builds and tests with and without Tauri |
| NATIVE-2 native shell & design system | **Complete** | [DESIGN_SYSTEM](DESIGN_SYSTEM.md); idle rendering measured at 0 % CPU |
| NATIVE-3 main launcher functionality | **Complete** for Home, instances, settings, install/readiness, launch | §5 |
| NATIVE-4 content, accounts, appearance | **Partial** — mods/packs/shaders, Discover, accounts, themes/accents, skins/capes present; modpacks, content updates, skin import missing | [PARITY_MATRIX](PARITY_MATRIX.md) |
| NATIVE-5 updater, platform, data compatibility | **Partial** — data compatible by construction; updater blocked by design; packaging not done; Linux not built | §7, §8 |
| NATIVE-6 full acceptance | **Not reached** — measurements and core workflows done; parity incomplete | this report |

## 3. Architecture work

* **Host boundary.** Every launcher operation (120 commands) moved from
  `tauri::AppHandle` to `host::Host` (data directory, version, events, task
  spawning, open folder / https URL) via a recorded mechanical transform; no
  command logic changed. Tauri commands are generated thin wrappers with
  identical names, argument keys, sync/async nature, return types, DTOs, error
  codes and events. Details: [ARCHITECTURE](ARCHITECTURE.md).
* **Feature gating.** `tauri`, `tauri-build`, opener and updater plugins are
  optional behind default `tauri-host`. Host dispatch is dynamic so tests and
  the native binary never link the WebView runtime (an enum version briefly
  pulled `comctl32!TaskDialogIndirect` into the test binary and broke it at
  load; found by diffing import tables and fixed before commit).
* **Native host** (`native/`): Slint UI with one global per domain, Rust
  controllers per domain, a typed view of the same serialized DTO contract as
  `src/lib/backend.ts`, stale-result discarding, coalesced readiness checks.

## 4. Native screens and workflows

Home · Instances (list, create, workspace with Settings/Mods/Resource
packs/Shaders, health check, repair, rename, delete with exact-name
confirmation) · Discover (Modrinth mods/resource packs/shaders) · Skins
(current skin, library, capes) · Settings (theme, accent presets, custom accent,
reduce motion, account, update status, Aurora Client update review/apply,
shortcuts, launcher folder, About with Slint attribution) · Accounts dialog ·
notifications. Screenshots: [`docs/native-ui/screens/`](screens).

## 5. Real installation acceptance (disposable root, native debug build)

Driven entirely through Windows UI Automation against
`AURORA_DIAGNOSTIC_DATA_ROOT` (never the owner's data root).

| Step | Result | Evidence |
| --- | --- | --- |
| 1. Fresh disposable instance creation | Pass — Minecraft 1.21.11 + Fabric 0.19.5 + Aurora 3.0.0 (defaults from the newest Aurora release) | `dev-create-progress.png`, `dev-after-create.png` |
| 2. Aurora Client 3.0.0 installation | Pass — 4677 files downloaded and verified by the existing pipeline | same |
| 3. Managed Java 21 | Pass — installed from Home's "Install Java" blocker, 402 files verified | `dev-java-done.png` |
| 4. Readiness | Pass — Rust blockers rendered, then **Ready** | `dev-home-ready.png`, `home.png` |
| 5. Authenticated launch | Pass — `javaw.exe` was the exact child of the native launcher | `dev-home-running.png` |
| 6. Main menu | Pass — Aurora title screen ("Aurora Settings") | `dev-game-title.png` |
| 7. Gameplay entry | Pass — new singleplayer world loaded with Aurora HUD | `dev-game-world.png` |
| 8. Normal exit | Pass — window close, exit code 0, world saved | `dev-home-after-exit.png` |
| 9. Return to Ready | Pass after fix — a readiness/exit race left "Running"; fixed (child snapshot wins, bounded re-check) | `home.png` |
| 10. Instance data preserved | Pass — `saves/New World`, `options.txt` present after exit | — |

## 6. Performance (measured; release builds; same host as §BENCHMARKS)

Tauri = `9b99db3` built with `tauri build --no-bundle` (production frontend),
isolated identifier, empty root. Native = final release build, empty root.
Medians of 3 interleaved runs ([`evidence/native6`](evidence/native6)).

| Metric | Tauri baseline | Native result | Difference |
| --- | ---: | ---: | ---: |
| Cold startup (first launch after build; not post-reboot) | 48 ms window / 1451 ms quiescent | 534 ms window / 1361 ms quiescent | quiescent −6 % |
| Warm startup (first window / quiescent) | 23.4 ms / 1615 ms | 25.3 ms / 874 ms | quiescent −46 % |
| Idle working set (process tree) | 407.2 MiB | 106.8 MiB | −300 MiB (−74 %) |
| Idle private memory (process tree) | 213.0 MiB | 85.7 MiB | −127 MiB (−60 %) |
| Total process-tree memory incl. GPU dedicated | 213.0 + 73.6 MiB | 85.7 + 0 MiB | −201 MiB |
| Processes | 7 (1 host + 6 WebView2) | 1 | −6 |
| Idle CPU (% of one core, 20 s) | 0.20–0.28 % | 0.00 % | qualified: both low; native measured zero |
| GPU activity | 0.00 % utilization; 73.6 MiB dedicated, 11.7 MiB shared | 0.00 %; 0 MiB | Qualified (counter resolution) |
| Minimized overhead (CPU) | 0 % (one run 13.7 %) | 0 % | Qualified |
| Navigation (UIA-driven, 120–150 switches) | 8.8 s CPU, working set 398 → 510 MiB, settled 472 MiB / 246 MiB private | 0.5 s CPU, 106 → 115 MiB, settled 87 MiB private | ~17× less CPU per switch |
| Navigation latency (UIA invoke round trip) | NOT MEASURED (WebView invoke returns before render) | p50 1.1 ms, p95 1.9 ms (dispatch only) | — |
| Active content browsing | NOT MEASURED | 92.7 MiB private with 20 live Modrinth results | — |
| Active install (debug build, during 4677-file download) | NOT MEASURED | 150 MiB WS / 106 MiB private | — |
| Memory after 120 navigations + 20 dialogs + Discover (populated root) | — | 89.8 → 92.8 MiB private; idle CPU back to 0 % | +3 MiB |

The 150 MiB target is met (≈107 MiB working set, ≈86 MiB private, no GPU
memory). Startup to quiescence is ~0.9 s warm. The NATIVE-0 prototypes show
the GPU renderer would have cost more memory than the WebView on this host.

## 7. Security and authentication

* Unchanged: OAuth PKCE S256 public client, system browser, loopback
  callback, Windows Credential Manager for the refresh credential, memory-only
  downstream tokens, artifact verification, path containment, process
  supervision by exact child handle. The native UI only ever sees
  `AccountSummary`; no password field exists.
* `host::open_url` now refuses anything but `https://` in both hosts
  (defense in depth for the authorization URL; unit-tested).
* All existing security tests pass in both feature sets (§9).
* **Credential note (owner action).** During testing the sign-in button opened
  Microsoft in the system browser, whose existing session completed sign-in for
  **Spxcterr** without interaction. Credential Manager targets are global per
  account (`…/account/<uuid>`), so this replaced the stored refresh credential
  the production launcher uses for that account with a new valid one. Do not
  sign out of that account inside the disposable root; that would delete the
  shared entry. No token was viewed, logged or exported.
* **Updater.** Native self-update is deliberately disabled
  (`updater_unconfigured`); the signed artifact is the WebView NSIS installer.
  Migration plan requiring owner approval:
  [UPDATER_ASSESSMENT](UPDATER_ASSESSMENT.md).

## 8. Platform and data compatibility

* **Windows:** working; 125 % scaling and two monitors correct; OS title bar.
* **Linux:** not built or run. The core builds without Tauri; the native host
  uses winit (X11/Wayland) and the software renderer; `host` has XDG and
  `xdg-open` paths. Credential storage still fails deliberately on Linux
  (pre-existing).
* **Data:** same identifier `com.aurora.launcher`, same directory, same
  schemas, through the same code. No migration. Release native builds read the
  production root unless built with `disposable-data-root`.
* **Packaging:** not done (no installer, no signing for the native exe).

## 9. Tests executed

| Command | Result |
| --- | --- |
| `cargo test` (src-tauri, Tauri host) | 1007 passed, 0 failed, 31 ignored; 6 integration passed |
| `cargo test --no-default-features` (core without Tauri) | 1001 passed, 0 failed, 31 ignored; 6 integration passed |
| `cargo check --all-targets` both feature sets | pass |
| `cargo fmt --check` (src-tauri, native) | pass |
| `cargo test` (native) | 5 passed |
| `cargo build --release` (native, Tauri baseline) | pass |
| UI Automation acceptance (§5), navigation/dialog scenarios (§6) | pass |
| `npm run check` (svelte-check) | 0 errors, 0 warnings, 278 files |
| `npx tauri build --no-bundle` after the extraction (isolated identifier) + real boot | pass — Home, accounts, widgets, playtime, recent worlds/servers rendered from the backend over the generated wrappers (`screens/tauri-post-extraction-home.png`) |
| `npm test` (frontend unit tests) | **not completed** — the run stalled with four live workers for 20 min (the recorded baseline run takes ~3 min) and was stopped; frontend source is unchanged by this initiative, so this needs a separate look |
| `npm run tauri build` with installer bundling | not run (no packaging change) |

## 10. Regressions found and fixed during the initiative

1. Test binary failed to load (`STATUS_ENTRYPOINT_NOT_FOUND`) after the first
   host design → dynamic dispatch.
2. Readiness was re-checked on every Home navigation; each check deep-hashes
   installed files → only on input changes, single in flight.
3. Exit/readiness race left Home on "Running" → child snapshot precedence and
   bounded re-check.
4. One shutdown hang observed (window closed, event loop alive); not reproduced
   after explicit UI-state release and bounded shutdown ordering.
5. Stale Java progress line, avatar initials, indeterminate progress fill,
   button stretching, row alignment.

## 11. Remaining limitations (not done)

Modpacks (browse/install/merge/update) · content update checks and bulk
updates · Discover categories, sorting, project details, version choice,
artwork · instance icons · loader version pinning · Aurora enable/disable
transition · Home widgets (playtime, recent worlds/servers, quick play) ·
Discord settings · skin import and 3D preview · Borealis background · launcher
self-update · packaging · Linux run · Narrator, high-contrast and 200 % text
testing · owner visual review.

## 12. Preservation audit

* Main checkout `C:\Dev\aurora-launcher`: HEAD `9b99db3`, `git status`,
  tracked-file list and SHA-256 of all seven pre-existing modified/untracked
  files are identical to the pre-work snapshot.
* Worktree commits contain no deletions or renames; `native/target` and
  prototype targets are ignored; no build output is committed.
* No push, merge, tag, release, workflow dispatch, update-authority change,
  website deployment, production launcher replacement or Aurora Client change.
* Processes were only ever closed or terminated by exact PID of processes this
  work started.
* Test data lives in the session scratchpad (`native-root-1`, `native-root-empty`)
  and `%LOCALAPPDATA%\com.aurora.launcher.native-baseline` (Tauri baseline); none
  is in the repository.

## 13. Recommended next steps

1. Owner visual review of the native screens; decide on the Spxctra direction.
2. Run the bundled installer build of the WebView launcher once before the next release.
3. Port the remaining workflows in parity-matrix order (modpacks and content
   updates first), then skin import with a native file dialog.
4. Decide the update-authority migration (package the native exe in the signed
   NSIS installer) — owner approval required.
5. Build and measure on Linux and a low-end Windows machine before shipping.
6. Consider scoping Credential Manager targets by data root so disposable roots
   cannot touch production credentials (core change, separate review).

## 14. Aurora Client

**AURORA CLIENT HANDOFF: NOT YET REQUIRED.** One observation for a future
Client task: at a small window size (≈860×480) Aurora's HUD lines
(Sprint/Sneak over FPS/Time) overlapped (`dev-game-world.png`). It is an
investigation topic, not a verified defect; no Client change was made.
