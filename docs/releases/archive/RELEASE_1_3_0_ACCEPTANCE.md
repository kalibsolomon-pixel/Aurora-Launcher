# Aurora Launcher 1.3.0 release acceptance record — continuation

Date: 2026-09-29. This record completes the interrupted 1.3.0 release
acceptance for the frozen candidate
`602718efe7ece2ce94a3c8611e7af8fd14a5ae57` (Phase F shell/appearance, Home
and Settings redesign, Borealis background and frosted glass, plus the
candidate release preparation). No source change was made during this
continuation; no new candidate was created.

## State reestablishment

Local HEAD and `origin/main` both resolved to the frozen candidate, 0
ahead/0 behind, tracked tree clean, all 609 protected pre-existing local
diagnostic files preserved. No `v1.3.0` tag, release, or draft existed;
public production remained Launcher 1.2.0. The frozen artifacts were
independently re-hashed byte-exact: NSIS 8,944,748 bytes
`7bc024f94f979e2ce2b44d0afc79e86e304bf9f27a13d2d2b3c30211c467aa4f`,
MSI 10,493,952 bytes
`4b058b3517ed0e0429e589bc3ade76d661d02a44fc2dda59d958174a66da7119`,
`release-assets.json` 585 bytes
`bbc31fffe10203cf61723089f2a44e8a6f7c0d0caa2e56420370faf9253c6444`
(BOM-less, exact source SHA). Inherited verified evidence (frontend 136/0,
Rust 564/0/21 ignored, icon 6/0, release tooling 14/0, svelte-check 0/0,
fmt/check/builds, the 1.2.0→1.3.0 NSIS upgrade with schema-4→6 migration
preserving theme/accent/widgets/Discord/selection, and Play observed to
Running with managed Java) was retained: the candidate source and artifacts
remained byte-identical, and Phase F touched no launch, bridge, history,
auth, Discord, distribution, or instance code (`git diff --name-only` over
`ec30f10..602718e`).

## Gate results (this continuation)

- **Real Minecraft acceptance — PASSED.** Observed Ready → Starting →
  Running → a real Minecraft 1.21.11 window → the title menu fully loaded
  and usable (MINECRAFT logo; Singleplayer, Multiplayer, Aurora, Options…,
  Quit Game; no loading overlay) under managed Java
  (`runtimes/java-runtime-delta/windows-x64-cb4394a…/bin/javaw.exe`). The
  game process exited cleanly and the launcher's supervision recorded
  `outcome: normal` with a 244,909 ms process-timed session; the launcher
  returned to **Ready** with Play re-enabled, no stuck Starting/Running, no
  duplicate processes, no repair, no instance mutation (instances registry
  and installed-game manifest untouched), pin still Aurora 2.1.2, and the
  E1 crashed-open-session from the interruption reconciled to
  `interrupted` as designed. Honest limitation: two dispatched Quit Game
  clicks at the corrected button coordinates produced no visible reaction
  while the game was backgrounded behind the owner's browser; the game
  exited cleanly a few minutes later (a delayed effect of those clicks or
  an owner click — not a process kill, which was never used).
- **Live account acceptance — PASSED with caveat.** The signed-in account
  loads at boot and renders with name and avatar; profile/player preview
  loads; session restoration across boots introduced no auth error. The
  owner switched the active account live during the session (MyRevenant →
  Spxcterr), demonstrating account selection works; that selection was
  preserved untouched afterward. The account dropdown itself could not be
  expanded by synthetic background clicks (the same background-input
  limitation as the game window) — chip state, avatar, and live switching
  are the evidence. No credential was logged or exposed; no sign-out.
- **Bridge/history/Quick Launch — verified to the extent the live instance
  permits.** This instance is pinned to Aurora Client 2.1.2, which by
  design receives no activity bridge (only the exact 2.1.5 digest does),
  so live MAIN_MENU/SINGLEPLAYER/MULTIPLAYER identity could not be
  exercised; no such claim is made. Verified live: history remains valid
  (session opened, process-timed, closed `normal`; the interrupted prior
  session reconciled), and the Playtime widget reflects recorded time.
  Recent Worlds/Servers show their honest empty state (no bridge targets
  exist), and Quick Launch cannot be exercised without bridge identity;
  both remain covered by the automated E1/E2 suites, and the frontend
  still gains no trusted world/server launch authority (unchanged code,
  passing suites).
- **Live Discord — documented limitation, consistent with established
  policy.** The Discord client was running during the entire real
  Minecraft session; the game launched and exited cleanly alongside it
  (non-fatal coexistence proven). The master toggle and every detail
  control — including the address control gated behind server sharing —
  were preserved verbatim through the schema-6 migration (verified in the
  persisted configuration; nothing was weakened for testing). Presence
  text during play and clearing after exit were not directly observed
  (reading the owner's Discord UI hit the same background-input
  limitation); per the established release record, live gameplay presence
  rendering has remained an owner-accepted outstanding item since 1.1.0
  and is not a publication blocker.
- **Production performance — PASSED.** Installed candidate, launcher/WebView
  tree (7 processes), ~15 s summed CPU deltas (100% = one core; 28 logical
  cores): Borealis speed 50 Home **0.10%**; Borealis speed 100 Settings
  **0.00%**; OLED + Simple **0.00%**; minimized **0.00%** (motion pause
  observed live). Working set 523.5–530.3 MiB, private 419.7–452.0 MiB.
  GPU: Windows process-attributed engine counters, mean **0.000%** over
  ten one-second samples at Borealis default. No player-renderer idle
  loop is observable at idle (0.10% Home idle includes the preview). All
  results are at or below the Phase F development comparison points.
- **Fresh install — PASSED.** After the uninstall below, a silent fresh
  install of the exact candidate (re-hashed EXACT before use) registered
  Aurora Launcher 1.3.0 (publisher aurora) with shortcuts, booted the
  Phase F Home with custom window controls, icon navigation, Borealis
  background (aurora-gradient pixels confirmed), frosted surfaces, Play
  readiness, Settings with independent theme/background/accent groups,
  and the preserved valid appearance state.
- **Uninstall — PASSED.** Silent uninstall removed registration, Start
  and desktop shortcuts, executable and uninstaller; the launcher-managed
  data root was preserved byte-identical (accounts, schema-6 config,
  gameplay history, instances); no unrelated data was touched and no
  `.minecraft` exists on this machine. `aurora_launcher_lib.dll` and the
  internal uninstall link copy remained initially due to WebView2
  teardown locking seconds after app close — verified transient
  (immediately openable exclusively), the established 1.2.0 precedent.
- **Restore + final smoke — PASSED.** Boot, account chip, Home with
  Ready instance and 2.1.2 pin, instance selector menu, Play readiness,
  Borealis, Settings load, Home↔Settings navigation, and all window
  controls exercised; no release-blocking visual or functional defect.

## Publication

All gates passed; publication used the hardened draft-first tooling.
Collision check returned `v1.3.0: create`. The draft was created with the
exact target SHA (one listing-lag rediscovery hiccup resolved through the
designed existing-draft path, identical to the 1.2.0 precedent — nothing
deleted or recreated), the three approved assets uploaded once and
verified byte-exactly inside the draft, and the existing verified draft
published exactly once as **release ID 398887565**
(https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/v1.3.0,
published 2026-09-29T06:53:10Z). The lightweight `v1.3.0` tag was created
by publication at exactly `602718efe7ece2ce94a3c8611e7af8fd14a5ae57` and
verified independently; `target_commitish` is the same full SHA; draft and
prerelease are false; GitHub's advertised digests equal the approved
values. Independent unauthenticated downloads of all three assets re-hashed
byte-exact, the manifest agrees with the downloaded installers under the
documented space-to-dot name normalization, and a silent reinstall from
the publicly downloaded NSIS booted a verified 1.3.0 (Ready, account, pin)
before a clean close.

Historical releases were re-verified untouched: Launcher v1.1.0 (2 assets)
and v1.2.0 (3 assets) unchanged; Aurora Client 2.1.5 public release
unchanged (2,468,545 bytes, `fdc344e2…`); v2.1.4 was never referenced. No
force push; screenshots stayed local under
`%TEMP%/aurora-1_3_0-visual/`; no installer was committed; the 609
protected diagnostics remain; tracked deletions: none.
