# Pre-release UI correction evidence

All inventories/accounts/artwork here are synthetic. Desktop captures use the
existing review shell in an isolated Tauri/WebView2 identifier, never owner state.
Raw Chromium traces and preservation inventories remain outside Git.

`r1.json` records three interleaved runs for each of five surfaces. The baseline
restores exactly the starting content background/backdrop declarations in the
same build; the candidate uses the implemented translucent unfiltered backing.
Each run has 100 illustrated entries and 60 wheel inputs (3000px down and back),
with live Borealis video. Frame callbacks, compositor CPU drawing work and
input-to-frame-swap are distinct measurements. Frame swap is **not** final physical
screen presentation, and DrawAndSwap is **not** hardware GPU utilization.

The measured rendering overhead is corrected. The severe owner-observed stutter
was not reproduced on this host; no higher FPS or eliminated dropped-frame claim
is supported by these runs. The report retains that release acceptance limitation.

Reproduce with `tools/visual/serve.mjs`, an externally isolated Tauri configuration
and WebView2's loopback debugging port. `profile-ui-corrections.mjs` uses
`UI_CDP_URL`, `UI_EVIDENCE_DIR`, `UI_SURFACES`, `UI_VARIANTS`, optional `UI_TRACE=1`
and `AURORA_PLAYWRIGHT_MODULE`. Analyze with
`python tools/visual/analyze-ui-corrections.py <evidence-directory> <trials.json>`.
The scripts neither acquire product artifacts nor invoke content mutations.
