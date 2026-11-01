# Native preview distribution contract — NATIVE-9

This is a local implementation, not permission to publish or activate an
authority. The stable WebView installation and update authority are preserved.
The current evidence and remaining production gates are in
[NATIVE_9_ACCEPTANCE.md](../../NATIVE_9_ACCEPTANCE.md).

## Preview isolation

| Identity | WebView stable | Native preview |
| --- | --- | --- |
| Executable | `aurora-launcher.exe` | `aurora-native.exe` |
| Per-user install directory | `%LOCALAPPDATA%/Aurora Launcher` | `%LOCALAPPDATA%/Aurora Launcher Native Preview` |
| Application-data directory | `%LOCALAPPDATA%/com.aurora.launcher` | `%LOCALAPPDATA%/com.aurora.launcher.native-preview` |
| Credential target prefix | `com.aurora.launcher/account/` | `com.aurora.launcher.native-preview/account/` |
| Shortcut name | Aurora Launcher | Aurora Launcher Native Preview |
| HKCU uninstall identity | Existing Tauri identity | `com.aurora.launcher.native-preview` |
| Discovery | Existing `launcher-update-authority/launcher-update.json` | Proposed `launcher-native-update-authority/native-update.json` |
| Release artifact namespace | Existing `v<version>` / WebView NSIS name | Proposed `native-v<version>` / `Aurora.Native.Preview_<version>_x64-setup.exe` |

The explicit `native-preview` core feature selects these data/credential/shortcut
identities and the native update engine. Combining it with `tauri-host` or
`private-candidate` is a compile error. The ordinary Tauri build keeps its exact
identity and updater. Preview version ordering is independent; a higher WebView
version never becomes a native offer and neither host silently migrates to the
other. The current source version remains 1.4.2; no production version/tag was
created by this work.

NATIVE-6/7/8 executables used the production credential namespace. Their warning
still applies: a disposable data root alone does not isolate those executables'
Credential Manager operations. NATIVE-9 does not copy or redeem their credentials.
Signing in to the preview creates a separate credential. Removing it removes
only the preview account entry; it is not a Microsoft-wide revocation.

## Signed native offers

`updates/native.rs` requires **both** the existing owner-supplied
`AURORA_UPDATER_PUBKEY` and the exact compile-time
`AURORA_NATIVE_UPDATE_AUTHORITY` equal to its proposed fixed HTTPS authority.
No runtime/environment/frontend endpoint or key override exists. Ordinary local
builds are unconfigured. No authority ref was created by this implementation.
Enabling it is a separate owner-reviewed build/release operation.

The proposed authority serves a bounded JSON envelope with two fields:
`payload` (base64 of the original JSON bytes) and `signature` (the existing Tauri
base64 minisign signature format). The compiled key verifies the **entire payload
before its fields become authority**. This adds authentication of discovery
fields; it does not weaken the stable stream's installer verification.

The signed schema-1 payload must bind product `com.aurora.launcher.native-preview`,
channel `native-preview`, runtime `slint`, Windows/x86_64, `nsis-per-user`, exact
numeric version, minimum compatible installed version, exact source SHA,
the fixed repository/tag/native filename, expected size/SHA-256 and a separate
installer signature. Unknown/missing fields, other identities, source URLs,
architecture, package types and incompatible minimum versions fail. Offers
must be strictly newer to download/install. Equality/build metadata cannot
authorize an upgrade. The prehashed installer signature's authenticated filename
must match the versioned native filename; an optional authenticated version
must also match. Private signing keys never enter runtime code.

Discovery refuses redirects and is bounded at 64 KiB. Installer transfers have
a 256 MiB bound, exact byte count and SHA-256, and stream signature verification.
Only bounded HTTPS redirects from the fixed GitHub native release path to
`release-assets.githubusercontent.com` are admitted. Error messages omit raw
response bodies and CDN query credentials.

## Download, handoff and recovery

The UI first reviews, then downloads/verifies, then explicitly chooses **Exit and
install**. Real byte progress comes from the transfer. Active games and visible
native installation/sign-in jobs must finish first. The core independently checks
registered game process state. No automatic installation or polling is introduced.

Downloads use a unique operation directory under `cache/native-updates/` and an
untrusted `.part` file. Hash/signature verification precedes rename to a verified
installer. Only exact operation-created staging is cleaned on failure; verified
staging is retained as process-local pending state. A restart loses that pending
authority and requires a new verified download. No cache garbage collection is
introduced.

Before handoff the core fetches/authenticates the offer again and compares every
field, including same-version drift, then rehashes/reverifies the retained file.
It permits automatic installation only from the fixed installed native preview
location. Structured process arguments hand off the exact live launcher PID to
NSIS. The installer validates that PID as a positive integer, opens that process
once and waits up to 30 seconds for its handle; it never finds/kills a process by
name or overwrites a running executable. The native UI exits only after installer
spawn succeeds, and nothing else normally holds the launcher's handle, so the
launcher can exit before NSIS opens it. NSIS therefore treats `OpenProcess`
failing with `ERROR_INVALID_PARAMETER` (no process has that PID) as "already
exited" and continues; any other open failure aborts. The sharing-violation probe
on the installed executable still runs afterwards and refuses while it is in use.
Residual risk: if Windows reuses the PID within that window, NSIS waits on the
unrelated process for at most 30 seconds and aborts, unchanged, unless it exits. NSIS can restart the
native executable after successful `/RESTART` installation.

The old executable is retained until activation succeeds. Rename failure attempts
restoration. Interrupted `.new`/`.previous` evidence is a deliberate conflict,
not permission to wipe the installation. A failed preflight preserves installed
bytes; the user can relaunch the old installation and arrange a reviewed manual
repair. The implementation does not claim a durable multi-file installer journal,
universal power-loss rollback, post-exit UI error reporting, or completed end-to-end
signed self-update acceptance. Those remain release gates in the acceptance report.

## Standalone Windows packaging

`native/tools/package-windows.ps1` invokes NSIS directly; Tauri is not used for
packaging. It refuses artifact overwrite, acceptance-feature distribution,
wrong native identity and version metadata. The installer requests `user`
execution, validates Windows x64, fixes its preview install path (ignores `/D`),
guards redirected ancestors and recognized package ownership, refuses downgrades
and foreign shortcut slots, and serializes its own installers. It creates a Start
Menu shortcut, offers an optional desktop shortcut, writes an HKCU Add/Remove
Programs entry and ships application/license notices. Uninstall removes only fixed
owned package files and target-proven shortcuts. It has no delete-data option,
wildcard deletion, recursive removal or process-name termination.

Disposable installer builds require an explicit `aurora-native9-*` temp root and
random acceptance identity. Their install location, shortcuts and registry entry
are separate from both real installations. Acceptance installers are clearly
named and are never production artifacts. Current packages are **not Windows
Authenticode publisher signed**. Updater minisign signatures authenticate update
bytes; they do not supply Windows publisher reputation or replace Authenticode.

## Local preparation and owner gates

1. Build and verify the ordinary native release. Package with
   `native/tools/package-windows.ps1 -OutputDirectory <new-output-directory>`.
2. If Authenticode is later configured, apply and verify it **before** updater
   signing and final hash recording. No certificate is configured here.
3. In the existing protected owner signing environment, sign the exact native
   installer using the existing identity. Never place private materials here.
4. `tools/release/native-update.mjs payload` prepares the exact payload from the
   installer, its signature, public-key file, version, minimum version and source
   SHA. Sign the resulting `native-update-payload.json` through the owner process,
   then `native-update.mjs envelope` verifies that signature and wraps its original
   bytes. Preparation does not contact or write GitHub.
5. Independently accept those exact signed bytes in a disposable Windows
   environment, including the full updater handoff and restart. The test-only
   fixture key/harmless payloads cannot substitute for this.
6. Separately approve native authority initialization/protection, signing build
   configuration, immutable native release publication and discovery advancement
   last. Reuse the existing authority protection principles from
   `UPDATE_AUTHORITY.md`; do not reuse or modify the stable ref. Native publication
   automation/authority advancement has not been implemented or dispatched here.

The manual `native-preview-candidate.yml` only builds a private CI artifact. It
contains no production secrets, signing/authority enablement, Git writes, release
creation or publication. It has not been run in this assignment. Its packaging
compiler provisioning still needs CI validation before an owner relies on it.

## Data compatibility and rollback

No persisted registry, accounts, configuration, icons, widgets, provider, pack,
recovery, runtime or asset-identity schema changes in NATIVE-9. Local imports
remain user-managed files without invented provider provenance. The native
preview reads the existing formats through the shared core. Tests operate only
on synthetic disposable copies. It does not automatically open, move, convert or
share the production data root.

For a future accepted stable replacement, use a separately reviewed migration:
stop both hosts, preserve a complete backup, test a disposable data copy with the
candidate, and decide explicitly which stable identity/data/credential scope is
retained. Do not copy token files or Credential Manager entries. The preview can
coexist indefinitely while the stable installation and its data remain intact.
Rollback during preview is to close/uninstall the preview and resume stable with
its untouched original data. Copying a test fixture back is not an approved
production migration. This evidence does not authorize an automatic host switch.
