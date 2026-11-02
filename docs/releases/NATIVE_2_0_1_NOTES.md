# Aurora Launcher Native 2.0.1 — Modpack Reliability and Download UI Fixes

## Fixed

- Prevented false-positive modpack integrity failures caused by game-modified configuration files.
- Improved recovery from interrupted modpack installation states.
- Prevented damaged modpack status from unnecessarily blocking Java configuration.
- Improved error messages identifying missing or mismatched managed files.
- Protected modpack-owned files from unintended synchronization changes.
- Moved download notifications to the bottom-right so they no longer obscure Discover's Show More (Load More) button, including at minimum window size.

## Preserved

- Managed artifact hash verification.
- User configuration and worlds.
- Existing native installer and updater security.
- Native resource-efficiency design.

Windows x64, per-user NSIS installation. Existing native-preview package and update
identities are unchanged. Updater signatures use the established production key;
Windows Authenticode publisher signing remains unconfigured, as in 2.0.0.

After installing, owners of affected packs can check:
Repair → Ready → Launch Minecraft → Exit → Check.
The instance should remain Ready. This release does not automatically repair or
modify an owner's instance.
