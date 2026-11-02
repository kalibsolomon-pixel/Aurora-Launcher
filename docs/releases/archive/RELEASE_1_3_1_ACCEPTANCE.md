# Aurora Launcher 1.3.1 release acceptance record

Date: 2026-09-30. This record documents the publication and verification of
Aurora Launcher 1.3.1, a tightly scoped corrective release for the approved
post-1.3.0 Phase F corrections. The release source is
`a8a7260e159d2e2535c4922345f592a8ded5809d` on `main`; this documentation
commit sits above that frozen source and the `v1.3.1` tag remains attached to
exactly that commit.

## Release delta from 1.3.0

The correction history (fast-forwarded onto `main`, preserving independently
reviewable commits) plus the version commit:

- `86198d7` — perceptible Borealis internal motion (media regeneration, tested).
- `b6487a6` — Phase F frosted glass on the account dialog.
- `a5b2b76` — Borealis folds animated within the anchored silhouette (final
  approved media, 6,372,600 bytes).
- `087ae7e` — native window minimize awareness: event-driven, no polling;
  typed `window-minimized` signal pauses decorative playback while minimized.
- `a8a7260` — release metadata: version 1.3.1 synchronized across
  `package.json`, lockfiles and Cargo manifests, plus release notes.

No next-generation feature work (mod updates, modpacks, synchronization,
renderer changes, or loader support) entered this release. Aurora Client
remains 2.1.5 for new compatible instances; existing pins are untouched.

## Frozen media

`static/backgrounds/borealis-loop.mp4` hashed
`cb9a88c8a7dac6ca9c46ff5e296759f308013064c6969c75234e9e23d0145deb` before
release work, again before publication, and matched in the built frontend
output. The installed build serves the approved media (verified 20-second
loop, 1920×1080, `borealis-loop.mp4`).

## Verification (all on the release source)

- Frontend: 141 tests passed, 0 failed; svelte-check 0 errors / 0 warnings;
  production build passed.
- Rust: 566 unit tests passed, 0 failed, 21 intentionally ignored; 6 icon
  integration tests passed; `cargo fmt --check` and `cargo check --all-targets`
  passed.
- Release tooling: 14 tests passed, 0 failed; version contract 1.3.1.
- MSI payload integrity proven by non-mutating administrative extraction.

## Installer acceptance

- **1.3.0 → 1.3.1 upgrade (NSIS, silent, in place over a public-1.3.0
  baseline): PASS.** Registration and executable became 1.3.1; all seven
  launcher-state files (accounts, schema-6 config, gameplay history,
  instances, two skin-preset files) were re-hashed byte-identical; theme,
  background, custom accent, Borealis speed, Discord privacy settings,
  selected instance, accounts and pins all preserved; the pinned 2.1.2
  instance was not silently migrated to 2.1.5.
- **Uninstall (silent): PASS.** Registration, executable, uninstaller and
  owned shortcuts removed; the launcher-managed data root preserved
  byte-identical including two instances; no unrelated data touched; no
  WebView2 teardown lock remained this time.
- **Fresh install (NSIS, silent): PASS.** Registered Aurora Launcher 1.3.1
  (publisher `aurora`), booted with the account restored, Ready instance,
  Phase F Home, Borealis, and complete Settings.

## Live acceptance

- **Real Minecraft launch: PASS.** Ready → Starting → Running under managed
  Java (`java-runtime-delta/windows-x64-cb4394a…`), a fully loaded Minecraft
  1.21.11 main menu (MINECRAFT logo; Singleplayer, Multiplayer, Aurora,
  Options…, Quit Game; no loading overlay), a graceful window-close quit
  (WM_CLOSE — the normal application close path, not a process kill), a
  process-timed 427,351 ms session recorded with outcome `normal`, and the
  launcher returned to Ready with Play re-enabled. Duplicate launch was
  blocked while Running; the instances registry kept its pins. Honest
  limitation: synthetic clicks on the GLFW game window did not register
  (the established background-input limitation), so the quit was issued as a
  normal window close.
- **Accounts: PASS.** The account dialog opens with the approved frosted
  glass (independently image-verified: frosted material, translucent nested
  surfaces, dimmed diffused Borealis), shows the active account with avatar,
  lists stored accounts with Use/Check/Remove, a `Check account` refresh ran
  to completion with correct modal disabling and no error, and the dialog
  closes cleanly. No credential was logged or exposed at any point.
- **Borealis: PASS.** Playback verified at 0.5×, 1× and 1.5× with
  persistence; 20-second seamless loop; quantitative anchored-silhouette
  evidence (frame pairs seconds apart: macro-structure difference 0.4–0.7%
  of dynamic range, brightness-centroid drift ≤0.14%, ~10% of pixels changed
  internally).
- **Minimized performance (hard gate): PASS.** With 15-second process-tree
  samples (100% = one core; 7-process launcher/WebView2 tree): OLED/Simple
  0.94%; Borealis 0.5×/1×/1.5× 6.25/7.70/8.96%; account dialog 9.06%;
  minimized 0.00–0.10%; restored 4.16%. While minimized the video element is
  deterministically paused with `currentTime` and `totalVideoFrames` frozen
  across the sampling window (verified at 1× and at true 1.5×, including
  under a forced blur), and playback resumes from position after restore.
- **Discord: PASS.** The full privacy configuration survived the upgrade
  verbatim (including the server-sharing-gated address control) and the
  launcher ran normally alongside the running Discord client throughout,
  including the game session. Live presence rendering was not directly
  observed (the established owner-accepted outstanding item since 1.1.0).
- **History/activity: PASS.** The Playtime widget reflects the recorded
  session, Recent Worlds/Servers show their honest empty states, and an
  interrupted session reconciled to `interrupted` as designed. Bridge-gated
  identity remains covered by the automated suites (the live 2.1.2 pin has
  no bridge by design).
- **Live user validation:** during this pass the owner created a complete
  new Aurora Client 2.1.5 instance with a curated mod list through the
  1.3.1 pipeline (verified digests, Modrinth installs), demonstrating
  production 2.1.5 recognition and install on the released code.

## Publication

All gates passed. `v1.3.1: create` was confirmed before publication. The
hardened draft-first tooling created the draft with the exact target SHA (one
GitHub listing-lag rediscovery hiccup resolved through the designed
existing-draft path — nothing deleted or recreated, identical to the 1.2.0
and 1.3.0 precedents), uploaded the three approved assets exactly once,
verified them byte-exactly inside the draft, and published exactly once as
**release ID 399892506**
(https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/v1.3.1,
published 2026-09-30T09:28:21Z). The lightweight `v1.3.1` tag resolves to
exactly `a8a7260e159d2e2535c4922345f592a8ded5809d`; `target_commitish` is the
same full SHA; draft and prerelease are false.

Independent unauthenticated downloads of all three public assets re-hashed
byte-exact against the frozen local candidate, and a silent reinstall from
the publicly downloaded NSIS booted a verified 1.3.1 (Home, account, Borealis
playing, account dialog glass, minimize-paused/restore-resumed playback)
before a clean close. Historical releases were re-verified untouched:
Launcher v1.1.0 (2 assets), v1.2.0 (3 assets) and v1.3.0 (3 assets,
target `602718e…`) unchanged, and the Aurora Client 2.1.5 public release was
not modified by this launcher release.
