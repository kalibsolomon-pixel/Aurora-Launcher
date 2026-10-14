# UI-3.1 screenshot review

Start with the [acceptance report](../../UI_3_1_MODPACK_LAYOUT_ACCEPTANCE.md).

- [Matched Modpacks before/after](modpacks-before-after.png): all five required
  sizes, identical fixtures, backgrounds and viewport dimensions.
- [Approved Mods beside corrected Modpacks](mods-modpacks-rows.png): original
  CSS pixel scale, showing the shared row geometry and distinct pack action.
- [Medium Modpacks results](after/modpacks-results-1024x768.png).
- [Minimum Modpacks results](after/modpacks-results-706x512.png).
- [Exact review at medium size](states/review-1024x768.png).
- [Optional review at minimum size and 200% text](states/review-minimum-200.png).

The 15 matched originals per side are retained in `before/` and `after/`. Three
additional after captures scroll the first Modpack row into view. Five review
captures cover all sizes. Browser images use the actual components with clearly
labeled deterministic provider fixtures, not native forwarding or owner content.
Ten approved Mods/Resource Pack PNGs remain byte-identical between sides.

Actual compiled native application, real Rust/provider results, fresh empty data:

- [Default 1120×790](native/modpacks-default.png).
- [Enforced 706×512 minimum, controls](native/enforced-minimum.png).
- [Minimum, scrolled results and review actions](native/minimum-results.png).
- [Exact-version details at minimum](native/minimum-details.png).

Measurement JSON is beside each native PNG. `intermediate-722x512` is an
intermediate frame size, not the enforced minimum. Two right-edge drag attempts
scrolled content without resizing; only measured dimensions are acceptance.
A later expansion attempt was rejected before input because its destination was
outside the window. Native medium-width coverage is supplemented by the browser
matrix. No real modpack installation or account action occurred.
