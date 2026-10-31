# Updater integration assessment

## Current authority (unchanged)

* Manifest: `https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-update-authority/launcher-update.json`
  (compiled constant, no runtime override).
* Trust: minisign-style signature over the full installer, verified against
  `AURORA_UPDATER_PUBKEY` compiled into release builds; builds without the key
  report "unconfigured".
* Artifact: the signed NSIS installer
  `…/releases/download/v<version>/Aurora.Launcher_<version>_x64-setup.exe`,
  pinned to the repository and version, redirects only to GitHub's asset CDN.
* Install: `tauri-plugin-updater` runs the verified installer and the
  application exits.

## Native host behavior

The native host compiles the core without `tauri-plugin-updater`. Every
launcher self-update operation (`check`, `download`, `install`) returns
`updater_unconfigured` with the message "Launcher self-update is not
available in the native preview build. Update through the published
installer." Settings shows it as the launcher update status. Aurora Client
update discovery and installation are unaffected (host-neutral).

This is deliberate:

1. The only signed artifact the authority publishes is the WebView build's
   installer. A native build that installed it would silently replace itself
   with the WebView launcher.
2. Shipping a native artifact through the same channel requires changing the
   update-authority manifest and release pipeline — explicitly outside this
   initiative without owner approval.
3. A second, unsigned updater must not exist.

## Recommended migration (requires owner approval)

1. Package the native executable in the existing NSIS installer (same
   install location, same shortcuts hooks, same identifier). The installer is
   what the authority signs, so the verification model is unchanged.
2. Implement native download/verify with the `minisign-verify` crate (the
   verifier `tauri-plugin-updater` uses) against the same compiled public
   key, preserving the same manifest URL, artifact pinning, redirect policy,
   two-step download/install, same-version drift re-check and
   "verified bytes only" rule already encoded in `updates::launcher`.
3. Launch the verified installer with structured arguments and exit, as the
   Tauri plugin does today.
4. Publish one release that contains the native build through the existing
   gated release process; earlier WebView builds update to it normally.

Until then the native host is a side-by-side preview distributed manually.
