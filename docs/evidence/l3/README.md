# L3 acceptance evidence

See [the L3 report](../../../LAUNCHER_REFINEMENT_L3.md) for the full implementation, test matrix, rollback limits and screenshot index.

These are actual isolated Tauri/WebView2 desktop captures. The original and refined Home widget images use matching 2000×1288 pixel viewports. Images 13/13b/17 use the physically resized native window. All artwork, player names and cape inventories are synthetic; none belongs to an owner account or private collection.

Images with the `FIXTURE ACCOUNTS · NATIVE LOCAL LIBRARY` label combine actual Rust local library commands with synthetic remote profile/cape responses. Empty-library and no-account fixture views are explicitly labeled visual fixtures. No screenshot of a fixture mutation proves live authenticated provider behavior.

Image 18 shows the optimized executable with the actual bundled production frontend and native backend, no fixture bridge, no connected account and a synthetic disposable local collection. It uses a separate external review identifier; the installed Launcher and production identifier were not changed.

The JSON results contain only sanitized counts, flags and synthetic test descriptions. Full filesystem inventories, hashes of personal files and diagnostic logs remain outside the repository. `verification-summary.json` records preservation counts and nonpersonal repository critical-file hashes. `image-manifest.json` records screenshot dimensions and checksums.
