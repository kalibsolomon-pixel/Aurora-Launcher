# Aurora Launcher 1.4.1 — release notes

A corrective release that restores launcher artwork, improves reliability, and refines the interface.

- Restored project artwork for Mods, Resource Packs and Shaders, with safe fallback images.
- Restored Recent Servers icons with reliable fallbacks.
- Improved validation and handling of launcher artwork and server icons.
- Refined the Home instance selector and Instances spacing.
- Reorganized General settings around Windows desktop integration, including Start menu status and desktop shortcut controls.
- Discord Rich Presence now connects automatically while enabled and reconnects on its own after interruptions.
- Accounts now open in a side drawer, and the account card sits at the bottom of the sidebar.
- Instance creation shows current Aurora compatibility information instead of outdated hard-coded text.
- Interface polish, including consistent "Minecraft Account" capitalization and removal of the standalone sidebar version label.

Production-client correction — October 7, 2026: the current production target is Aurora Client 3.0.0 for Minecraft 1.21.11. Corrected Launcher source selects it for new compatible Aurora instances. Existing instance pins are preserved; nothing updates automatically.

The previously published 1.4.1 installer is unchanged by this source correction. After creating a compatible instance with that installer, use Check for Updates and explicitly apply the Aurora Client 3.0.0 update before playing. See `CLIENT_3_0_0_PRODUCTION_CORRECTION.md` for verification and publication limits.

Existing Aurora Launcher 1.4.0 installations require manual installation of 1.4.1. Launcher updates use signed installers verified by Aurora.
