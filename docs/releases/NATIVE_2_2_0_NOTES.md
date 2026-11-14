# Aurora Launcher 2.2.0

Aurora Launcher now uses the GPUI interface for the installed application and
signed updates. Existing instances, accounts, credentials, settings, installation
paths and updater trust stay in place. The native update manifest keeps its
legacy schema marker so existing native clients can receive this renderer change. Settings supports downloading a verified
Launcher update and exiting to install it.

Aurora Client discovery and update checks use the official Client GitHub releases.
Client 3.4.0 is available for Minecraft 26.1.2, 26.2 and 26.3, each with its matching
JAR. Minecraft 1.21.11 keeps Client 3.2.1 as the legacy option. Updates verify the
published SHA-256 and JAR metadata and preserve the instance's exact Minecraft,
Loader, Java and Fabric API environment. Client updates remain explicit reviewed
transactions with backups and rollback.

The polished interface includes selected-instance Content navigation, aligned
content toggles and deletion actions, twenty-item Discover pages, a single install
progress indicator, clearer cape previews and the Aurora logo and application icon.

The executable remains `aurora-launcher.exe`; the installer is
`Aurora.Launcher_2.2.0_x64-setup.exe`. Builds through 2.1.0 still require one manual
installer upgrade for the historical filename transition. Historical WebView data
and its update authority remain separate. Windows publisher signing remains
unconfigured; updater signatures verify release authenticity independently.
