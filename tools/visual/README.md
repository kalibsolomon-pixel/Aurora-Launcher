# Phase F review harness

Run `node tools/visual/serve.mjs` from the repository root. The standalone Vite server listens only on `127.0.0.1:1422` and mounts the actual Shell, Home, Settings, Instances and individual instance components. It is not imported by the production app or production build.

Only this server aliases the application invoke boundary to synthetic DTOs. No fixture Play action launches a process, and no fixture account action mutates real launcher state. Layout saves and browser appearance saves use tab session storage. When hosted in Tauri, appearance writes call the actual Rust command to verify its derived palette and persistence in the explicitly isolated review identifier; the returned state is also saved to session storage. Native window APIs remain real. The review label distinguishes fixtures from production data; the block player is the existing no-skin fallback.

Query parameters: `theme=oled|midnight|aurora-dark`, `background=simple|borealis`, `page=settings|instances`, `tab=overview|settings|mods|resourcePacks|shaders`, `instances=0..4` (seeded instance count for the Instances-page hierarchy fixtures; default 2), `skin=1` (render the fixture player skin on Home), `speed=0..100`, `time=seconds`, `reduced=1`, `edit=1`, `empty=1`. Overrides are review-only. This is a focused visual harness, not a complete backend emulator; unsupported actions report a fixture error. Clear the tab's session storage to reset its layout.

For native review, provide a temporary CLI Tauri config with a distinct identifier (for example `com.aurora.launcher.phasef-review`), an empty `build.beforeDevCommand`, and `build.devUrl` pointing to this server. Include the main window dimensions in that override. Never alter the production identifier for a review. The actual-backend smoke uses the regular port 1420 frontend with the same isolated identifier.

`node tools/visual/benchmark-player.mjs` measures software rasterization after warm-up. It excludes native canvas upload and is not a browser frame-time benchmark. Acceptance measurements and visual evidence are in `PHASE_F_ACCEPTANCE.md` and `docs/phase-f/`.
