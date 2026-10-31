# Native Aurora identity

`aurora.svg` is copied byte for byte from the owner-confirmed
`C:\Users\kalib\Downloads\aurora.svg` (NATIVE-7, 2026-10-31). Its 640×512
viewBox and ribbon paths are unchanged. The sidebar uses this vector directly.

`tools/generate-icons.mjs` produces the transparent 1024 px master, small PNG
frames, and Windows ICO from that source with Sharp/libvips. It adds transparent
padding only. ICO order remains 40, 16, 20, 24, 32, 48, 64, 128, 256; every frame
is generated from the master. `native/build.rs` embeds this ICO in the native
executable, and the Slint window uses `icon-256.png`. Future native packaging
should consume this same directory. No native installer exists yet.

The retained WebView host's old artwork and package outputs are preserved.
They are not release artifacts for the native host and are not installed over it.
