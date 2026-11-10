# Aurora Launcher Native 2.1.1 — Modrinth Modpacks and Native Interface Improvements

Modrinth modpacks can create a new instance or add compatible content to an existing instance using Fabric, Forge, or NeoForge. Review now identifies the Minecraft and loader versions, destination, component limits, preserved configuration files, and installation progress. Failed new-instance installs can resume using the same instance instead of creating a duplicate.

This release also includes the reviewed native interface, artwork caching, Borealis rendering and shutdown fixes, gameplay-history coordination, and Forge/legacy Minecraft compatibility improvements.

Known limits:

- CurseForge and Quilt are outside this release's scope.
- NeoForge modpacks require Aurora's supported 26.x generation; 1.20.x and 1.21.x NeoForge packs are refused.
- Forge and NeoForge modpack updates are not supported. Fabric updates retain their existing behavior.
- New instances allow at most 512 recognized components; additions to an existing instance allow at most 64. Existing differing user configuration files are preserved.
- Older launchers may refuse schema-3 modpack state. Preserve compatible backups before a manual downgrade; update authority is never rolled back.

The owner-attended Fabric/Forge/NeoForge Modrinth modpack matrix was **waived by the owner as a release gate**. It was not run or passed. No live modpack installation or gameplay validation is claimed for that matrix. Automated validation is recorded separately against the exact release source.

The package identity remains `com.aurora.launcher.native-preview` and the update channel remains `native-preview`. The established `Aurora.Native.Preview_2.1.1_x64-setup.exe` installer naming protocol is retained for compatibility with published native clients. Native and historical WebView release/update authorities remain separate.

Production updater signatures authenticate release content. Windows Authenticode publisher signing remains unconfigured; updater signatures do not establish Authenticode publisher identity.
