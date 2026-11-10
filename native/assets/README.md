# Aurora Launcher assets

`aurora.svg` is copied byte for byte from the owner-confirmed
`C:\Users\kalib\Downloads\aurora-refined.svg` (owner-approved option B).
It retains the 640×512 viewBox and original Aurora gradient colors, fills the
space between the ribbons, and joins their left tips into one point. The thin
highlight strokes and angular fold overlay are removed for calmer bands.

`native/tools/generate-icons.mjs` produces the transparent 216×173 sidebar mark
(`aurora-mark-216.png`) and the 1024 px application icon master (`icon.png`) with
Sharp/libvips. The app icon places the ribbon on the existing dark-gradient
squircle with transparent outer bounds. It also generates small PNG frames and
the Windows ICO from the master. ICO order remains 40, 16, 20, 24, 32, 48, 64,
128, 256. `native/build.rs` embeds this ICO in the native executable, the Slint
window uses `icon-256.png`, and native packaging consumes the same assets.

The retained WebView host's old artwork and package outputs are preserved.
They are not release artifacts for the native host and are not installed over it.
