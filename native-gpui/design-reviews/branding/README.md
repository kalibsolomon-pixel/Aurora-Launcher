# Aurora branding

The sidebar embeds the original full-color Aurora SVG and caches the image rather
than displaying the placeholder sparkle tile. The logo keeps its 640:512 aspect
ratio. Header dragging and the existing Aurora / Launcher text are preserved.

The owner's Downloads files `aurora-refined.svg` and `aurora-app-icon.png` match
`native/assets/aurora.svg` and `native/assets/icon.png` byte for byte. Existing
shared assets are reused without duplicating or changing their artwork. The
Windows build embeds the shared multi-resolution ICO derived from the icon PNG,
using resource ID 1, which GPUI loads for its native window class. The Windows-only
build dependency uses the same winresource crate as the supported host.

Validation: native debug build and Windows boot; all 385 tests passed; formatting
and whitespace checks passed. The executable's icon group was extracted and all
nine resolutions compared pixel for pixel with the shared ICO. The extracted
256px icon is [shown here](embedded-icon.png). [Default](home.png) and
[minimum-size](home-minimum.png) native captures verify the logo's placement.
The SVG renders through GPUI's full-color image decoder, preserving its gradients.
No performance measurements or new accessibility compliance claims are made.
Earlier review evidence remains unchanged.
