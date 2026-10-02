# Phase K evidence index (2026-10-02)

All live evidence was produced from the real Aurora UI
(`npm run tauri dev`, debug build) against the disposable diagnostic root
`C:\Users\kalib\AppData\Local\Temp\aurora-phase-k-root` (verified absolute
before any mutation; production launcher data was never touched). UI driving
used DOM events over the devtools port of the real app window; every
screenshot below is the genuine rendered application.

| File | Kind | What it proves |
| --- | --- | --- |
| research-notes.md | RESEARCH | Authoritative NeoForge model (verified against live maven artifacts + a real launcher-style boot) |
| probe-boot-log.txt | LIVE (research probe) | Minecraft 26.2 + NeoForge 26.2.0.8x boots with the classpath/natives/libraryDirectory model Aurora implements; "NeoForge mod loading, version 26.2.0.88, for MC 26.2"; game layer = minecraft-client-patched jar |
| probe-boot-screen.png | LIVE (research probe) | Minecraft title window "Minecraft NeoForge 26.2" from the probe boot |
| 00-create-form.png | LIVE | Creation form with Vanilla/Fabric/NeoForge platform options |
| 01-loader-selection-neoforge.png | LIVE | NeoForge selected as the platform |
| 02-neoforge-version-selection | DOM CAPTURE (transcript) | The exact-version picker populated with 89 versions for MC 26.2, newest stable first (26.2.0.88), betas marked "(unstable)" — captured as DOM state during the live run; see final report §Version discovery |
| 03-instances-all-ready.png | LIVE | Three NeoForge instances Ready: 26.2 pinned 26.2.0.88, 26.2 automatic, and 26.3 pinned beta 26.3.0.41-beta |
| 03-install-progress | NOT CAPTURED LIVE | Warm installs complete in ~4 s and the one cold install raced a dev-server reload; progress phases are proven by the live DOM captures ("Validating…"), the emitted instance-progress events, and the deterministic install tests |
| 04-instance-ready.png | LIVE | First NeoForge instance Ready after full validation |
| 05-installed-neoforge-mod.png | LIVE | AppleSkin (NeoForge build for MC 26.2) installed from Modrinth with provider provenance |
| 09-restart-persistence.png | LIVE | After a full app restart against the same root: instance Ready, exact MC/NeoForge versions persist, installed mod persists (shown as NeoForge metadata), deep validation passes, no re-downloads |
| 10-incompatible-content-rejected.png | LIVE | MC 1.21.11 + NeoForge creation fails truthfully ("no mod loader version could be selected… no compatible version for this Minecraft version") with no instance created |
| 11-launch-auth-blocker.png | LIVE | Play readiness: managed java-runtime-epsilon/Java 25 green; launch blocked only by `authentication_required` (no account in the isolated diagnostic environment) |

Additional live DOM evidence retained in the acceptance transcript:
the first install's failing metadata parse (later fixed: unknown installer
document fields), the successful retry through
Installing→Validating→Ready, and the AppleSkin details view selecting the
`appleskin-neoforge-mc26.2-3.0.10.jar` file for the NeoForge instance.
