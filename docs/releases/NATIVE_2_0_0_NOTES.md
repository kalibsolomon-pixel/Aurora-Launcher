# Aurora Launcher Native 2.0.0

The first major native Rust/Slint Aurora Launcher release for Windows x64.

- Native interface with lower UI overhead in prior controlled measurements;
  results vary with hardware, content and workload.
- Improved instance preparation, shared runtime/assets, verified downloads,
  and coordinated artifact acquisition.
- Refined Modrinth modpack installation and reviewed updates, exact loader
  selection, and validated per-instance custom Java.
- Optional cross-instance settings/files synchronization with conflict review,
  backups, per-instance overrides, and safe server-list additions.
- Responsive Home widget content and the owner-approved Aurora icon improvements.
- Native NSIS installation and independently signed native update discovery.

World synchronization and automatic merging of edited server entries are not
supported. Native uses the established `native-preview` channel and separate
installation/data/credential identity `com.aurora.launcher.native-preview`.
Existing WebView 1.5.1 installations and update authority are preserved; there is
no automatic migration or replacement.

Installer: `Aurora.Native.Preview_2.0.0_x64-setup.exe` (Windows x64, per user).
The release includes detached installer and manifest signatures plus the exact
artifact inventory. Verify the installer SHA-256 against that inventory before
opening it. Updater cryptographic signing is separate from Windows Authenticode;
no Windows publisher certificate is configured, so unknown-publisher warnings
may appear under the owner's accepted distribution policy.
