# Phase K evidence index (2026-10-02)

All live evidence was produced from the real Aurora UI
(`npm run tauri dev`, debug build) against the disposable diagnostic root
`C:\Users\kalib\AppData\Local\Temp\aurora-phase-k-root` (verified absolute
and confirmed by the backend's own startup line before any mutation;
production launcher data and `.minecraft` were never touched). UI driving
used DOM events over the devtools port of the real app window; every
screenshot below is the genuine rendered application.

## Implementation acceptance (first pass)

| File | Kind | What it proves |
| --- | --- | --- |
| research-notes.md | RESEARCH | Authoritative NeoForge model (verified against live maven artifacts + a real launcher-style boot) |
| probe-boot-log.txt | LIVE (research probe) | The launch model Aurora implements boots MC 26.2 + NeoForge 26.2.0.88 out-of-app; NOT Aurora launch acceptance |
| probe-boot-screen.png | LIVE (research probe) | Minecraft title window "Minecraft NeoForge 26.2" from the probe boot |
| 00-create-form.png | LIVE | Creation form with Vanilla/Fabric/NeoForge platform options |
| 01-loader-selection-neoforge.png | LIVE | NeoForge selected as the platform |
| 02-neoforge-version-selection | DOM CAPTURE (transcript) | Exact-version picker populated with 89 versions for MC 26.2, newest stable first, betas marked unstable |
| 03-instances-all-ready.png | LIVE | Three NeoForge instances Ready: 26.2 pinned 26.2.0.88, 26.2 automatic, 26.3 pinned beta 26.3.0.41-beta |
| 03-install-progress | NOT CAPTURED LIVE | Warm installs complete in ~4 s and the one cold install raced a dev-server reload; phases proven by live DOM captures, emitted events, and deterministic tests (not artificially recreated) |
| 04-instance-ready.png | LIVE | First NeoForge instance Ready after full validation |
| 05-installed-neoforge-mod.png | LIVE | AppleSkin (NeoForge build for MC 26.2) installed from Modrinth with provider provenance |
| 09-restart-persistence.png | LIVE | After a full app restart: instance Ready, exact versions persist, mod persists, deep validation passes, no re-downloads |
| 10-incompatible-content-rejected.png | LIVE | MC 1.21.11 + NeoForge creation fails truthfully; no instance created |
| 11-launch-auth-blocker.png | LIVE | Historical: Play readiness before sign-in — runtime green, launch blocked only by `authentication_required` |

## Final launch acceptance (completion pass, authenticated)

Authentication was completed through Aurora's normal Microsoft flow in the
isolated diagnostic environment (owner-interacted browser sign-in); the
account record stores only the profile UUID and name, and the refresh
credential lives exclusively in the OS-backed Credential Manager. No token
was copied, injected, printed, or captured.

| File | Kind | What it proves |
| --- | --- | --- |
| 12-authenticated-ready.png | LIVE | Account "Spxcterr" signed in; content/Java-25 readiness green; authentication blocker gone; instance Ready |
| 13-neoforge-running.png | LIVE | Ready → Starting → Running; supervised managed-runtime javaw.exe; readiness shows "Process 19252 · started 10:11:21 AM" |
| 14-neoforge-main-menu.png | LIVE | Genuine Minecraft 26.2 main menu with the NeoForge badge, Singleplayer/Multiplayer/Options/Quit buttons |
| 15-neoforge-normal-exit-ready.png | LIVE | After normal Quit: exit code 0 in Aurora's supervised log, Play available again, no orphan process |
| 16-neoforge-restart-after-launch.png | LIVE | After a full launcher restart: account still signed in, instance Ready, deep validation passed, AppleSkin present, no reinstall/redownload |
| 17-duplicate-launch-refused.png | LIVE | While Running: no Play control exists (replaced by the Running indicator); readiness reports the supervised process; exactly one Java process |
| aurora-launch-1790950281-0.log | LIVE (runtime log) | First launch: redacted supervised log, "Setting user: Spxcterr" (session injection), "Exit code: 0" |
| aurora-launch-1790950794-1.log | LIVE (runtime log) | Second-launch sanity: "Exit code: 0" |
| latest.log (in-instance) | LIVE (game log) | "Found mod file neoforge-26.2.0.88-universal.jar", "Found mod file appleskin-neoforge-mc26.2-3.0.10.jar", mod list "NeoForge 26.2.0.88 (neoforge)" + "AppleSkin 3.0.10+mc26.2 (appleskin)" |

Secret scans of both supervised launch logs and the game log found zero
matches for token patterns (`access_token`, `Bearer`, `Xbl3.0`, `XSTS`,
`refresh_token`).
