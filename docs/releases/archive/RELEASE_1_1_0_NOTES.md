# Aurora Launcher 1.1.0 — release notes

New compatible Aurora instances use the verified Aurora Client 2.1.3 release
with Minecraft 1.21.11, Fabric Loader 0.19.5, Fabric API 0.141.6+1.21.11,
and managed Java 21. Existing instances remain pinned.

- Improved Home, account presentation, player preview, and instance navigation.
- Customizable Home widgets and persisted layout preferences.
- Clearer installed-mod controls, guarded removal, and provider ownership details.
- Improved Modrinth filtering, dependency checks, and provider content lifecycle.
- Optional Aurora installation and genuine Vanilla execution alongside Fabric.
- Expanded compatibility validation and clearer launch-failure diagnostics.
- Optional Discord Rich Presence using the Aurora Client application and logo.
- Authenticated Aurora Client 2.1.3 gameplay activity, with world, server, and
  server-address details controlled by separate privacy opt-ins.

Discord presence and all extra details default off. Existing instances remain
pinned; installing this launcher does not silently upgrade their Aurora release.
Compatibility checks cannot guarantee every cross-mod interaction or resolve
every dependency conflict. Live Discord gameplay rendering acceptance remains
outstanding; no unsupported loader or Vulkan compatibility is claimed.
