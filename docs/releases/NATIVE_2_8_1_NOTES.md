# Aurora Launcher 2.8.1

**Automatic upstream Mojang manifest sync on launch.** Fixes a launch failure
when Mojang updates a published Minecraft version manifest in place (for
example bumping `lwjgl-freetype` from `3.4.1` to `3.4.1-mojang-2.15.0.3` on
Minecraft `26.2`) after an instance was already installed:

- Detects when an intact instance matches its pinned Minecraft version and
  platform, but `installed-game.json` is missing a newly published required
  launch file from Mojang's live version manifest.
- Automatically runs a transactional incremental game sync under the instance
  operation lock during Play — reusing all verified cached artifacts and assets
  without touching `mods/`, `config/`, or `saves/`.
- Re-validates the refreshed launch snapshot (failing closed if the instance
  record or Aurora state changed concurrently) and persists the offline launch
  receipt only after the sync commits.

Update trust, application identity and managed-data locations remain unchanged.
Windows publisher signing remains unconfigured; signed update artifacts provide
release authentication.
