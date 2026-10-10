# Pre-release UI correction evidence

All inventories/accounts/artwork here are synthetic. Desktop captures use the
existing review shell in an isolated Tauri/WebView2 identifier, never owner state.
Raw Chromium traces and preservation inventories remain outside Git.

`r1.json` records three interleaved runs for each of five surfaces at commit A.
Its baseline restores exactly the starting content background/backdrop
declarations in the same A build; the candidate uses an unfiltered backing.
Later revisions add workspace geometry and a shared glass layer; reproduce the
original controlled experiment from A, not by relabeling a final-source run.
Each run has 100 illustrated entries and 60 wheel inputs (3000px down and back),
with live Borealis video. Frame callbacks, compositor CPU drawing work and
input-to-frame-swap are distinct measurements. Frame swap is **not** final physical
screen presentation, and DrawAndSwap is **not** hardware GPU utilization.

Per-result filters are removed; the restored shared glass adds the measured cost
documented in `glass-match.json`. The severe owner-observed stutter was not
reproduced on this host; no higher FPS or eliminated dropped-frame claim is
supported by these runs. The report retains that release acceptance limitation.

Reproduce with `tools/visual/serve.mjs`, an externally isolated Tauri configuration
and WebView2's loopback debugging port. `profile-ui-corrections.mjs` uses
`UI_CDP_URL`, `UI_EVIDENCE_DIR`, `UI_SURFACES`, `UI_VARIANTS`, optional `UI_TRACE=1`
and `AURORA_PLAYWRIGHT_MODULE`. Analyze with
`python tools/visual/analyze-ui-corrections.py <evidence-directory> <trials.json>`.
For the final glass comparison, set
`UI_VARIANTS=previous-backing,shared-glass` on the final application source.
The scripts neither acquire product artifacts nor invoke content mutations.

`historical.json` is the safe archive comparison from `8df387b` before L1.
`final.json` checks all five views at commit C. `glass-match.json` compares the
previous unfiltered backing with the final established shared glass; geometry
changed, so use `r1.json` for the controlled blur attribution. `artwork-cache.json`
separates real native cold acquisition from cache reuse in a disposable root.
The latter uses 20 public projects behind 100 synthetic rows, not an owner inventory.
It is an opt-in live diagnostic, not a deterministic Internet-dependent unit test.
Run `profile-artwork-cache.mjs` only with `UI_MANAGED_ROOT` matching the freshly
created diagnostic root of the isolated debug app; it refuses a populated cache
and never deletes one. `UI_EVIDENCE_DIR` must already exist.

`ui-corrections.test.mjs` covers warning accessibility, advisory versus blocking
feedback, stable scrolling/artwork, shared glass equality, reduced transparency,
720×520/900×930/1600×1000 layouts, and the standalone modpack ancestor.
Set `UI_CDP_URL` for native window resizing or omit it for headless Chromium.
Existing L1/L3 and correction harnesses provide the broader regression coverage.

`before/` and `after/` contain 14 corresponding actual WebView2 captures per revision,
plus a final standalone modpack glass comparison in `after/`,
with viewport receipts. The before DOM was captured before R2/R3; only the R1
content declarations were temporarily restored to the exact initial declarations.
`capture-ui-corrections.mjs` reproduces the current-source views using an explicit
`UI_CAPTURE_DIR`; capture a baseline from its own source revision, not CSS alone.
All before images and all 15 refreshed after images were visually inspected,
including full-size warning/narrow views. Earlier C captures remain in commit D.
The final shared panel uses the existing group material; its restored translucency
does not inherit the discarded 82% backing's white-underlay contrast bound.
