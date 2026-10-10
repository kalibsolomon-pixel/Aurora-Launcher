# Aurora Launcher 1.4.2 — release notes

- Improved installed and browsed artwork reliability, including bounded WebP and GIF support with safe fallbacks.
- Reduced redundant healthy startup readiness checks.
- Refined browsing controls, responsive content panels, shared glass surfaces and selected navigation states.
- Placed installed-mod warnings beside version labels with accessible, grouped details.
- Made instance renaming and configuration editing easier to find, with protection against stale edits.
- Added durable Recent Servers history and favorites.
- Added a Skins & Capes library with search, favorites, previews and account-aware apply/equip actions.

New compatible Aurora instances select Aurora Client 3.0.0 for Minecraft 1.21.11. Existing instance pins remain unchanged until an explicit update.

Windows x64 NSIS installers use Aurora's existing updater signing key. Windows Authenticode publisher signing is not configured; Windows may show an unknown-publisher warning. Launcher 1.4.0 requires a manual installer bootstrap; Launcher 1.4.1 uses the signed update path.

Recent Servers history and the skin library migrate known schema-1 documents to schema 2 on their next successful edit. Older launchers cannot read these newer documents; preserve a backup before reverting to an older launcher. Uninstall preserves launcher data by default; leave **Delete app data** unchecked to retain instances and personal content.
