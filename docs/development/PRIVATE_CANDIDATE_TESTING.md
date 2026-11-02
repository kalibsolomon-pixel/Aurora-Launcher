# Private Windows candidate

Build committed source with `powershell -ExecutionPolicy Bypass -File tools/release/private-candidate.ps1`.
This uses the normal Tauri NSIS workflow and never runs publication tooling.
The base production version stays 1.4.2. The private overlay identifies
`1.4.2-private.20261019.3`, product **Aurora Launcher Private Test Candidate**,
identifier `com.aurora.launcher.private-candidate`. It requires the paired Cargo
feature; ordinary production configuration and identity are unchanged.

Install the private setup executable. Its default per-user installation folder,
Start menu entry, optional Desktop shortcut, WebView profile and managed data are
separate. Data defaults to `%LOCALAPPDATA%\com.aurora.launcher.private-candidate`.
The installed executable is `aurora-launcher-private-candidate.exe`, so NSIS's
process-name check cannot target the production `aurora-launcher.exe`.
Secure refresh credentials use `com.aurora.launcher.private-candidate/account/…`
in Windows Credential Manager. Sign in independently through the system browser;
never copy the production account store or tokens. The portable executable uses
the same candidate data and requires installed WebView2; it is not data-portable.

For path acceptance only, launch the private executable in a process with
`AURORA_PRIVATE_DATA_ROOT` set to a new absolute disposable folder. The production
executable ignores that variable. Use a short ASCII-only path for ordinary tests.
The packaged Java 21 launcher has demonstrated DLL/path failures in long and
Japanese-character paths; Aurora reports them and blocks Play. Reinstallation at
the same failing location does not resolve Java's own path limitation.

Launcher self-updates are intentionally unconfigured in this private package,
including when a production key exists in the build environment. Client discovery
retains its existing published authority and requires explicit application. No
unpublished Client is advertised. The embedded catalog offers published Client
3.0.0; a newer local JAR with the same version is not that release artifact.

Owner checklist:

- Start the private candidate and confirm its title and prerelease version in About.
- Sign in through the legitimate Microsoft browser flow with an entitled account.
- Create/select an instance; install Minecraft 1.21.11, Fabric and Aurora as desired.
- Run Check managed Java; confirm Overview and Home agree on its status.
- Launch Minecraft, reach its main menu, quit normally and confirm return to Ready.
- Browse Modrinth, review compatible content, install a test mod, disable/enable it.
- Rename/select the instance. Resize to a narrow window and inspect dialogs.
- Change an appearance/widget setting; close and reopen and verify persistence.
- Inspect error details and Updates; no launcher self-update should be offered.

Remove the application using its separate Windows installed-app entry. Preserve
the candidate data folder for inspection; no instance/user content needs deletion.
Sign out/remove test accounts through the candidate before uninstalling if you
want its secure credentials removed. Removing the portable executable alone does
not remove candidate data or credentials. Never delete production folders.

Artifacts are unsigned locally; this is not public release approval. The report
`LAUNCHER_JAVA_VALIDATION_AND_PRIVATE_RC_REPORT.md` records exact artifacts, hashes,
source identity, native acceptance and remaining gates. Public signing, release
tags, manifests, download pages and coordinated Client publication remain owner
decisions after independent testing.
