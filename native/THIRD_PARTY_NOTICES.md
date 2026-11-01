# Third-party notices — Aurora Launcher native host

The native host (`native/`) adds these components to the launcher core,
which keeps its existing dependency notices.

* **Slint** (https://slint.dev) — used under the Slint Royalty-free License
  2.0. Attribution is provided by the `AboutSlint` widget in Settings → About.
  Slint's license notices in its source are unmodified.
* **winit, AccessKit, and Slint's transitive Rust crates** — MIT and/or
  Apache-2.0 licensed; see `Cargo.lock` for exact versions.
* **Material Design icons** (Google) — the icon outlines in
  `native/ui/icons.slint` are from Google's Material Design icon set, licensed
  under the Apache License 2.0 (the same family the WebView launcher ships
  through the `material-symbols` package).
  The searchable instance catalog in `src/symbols_data.rs` also derives from
  Google's Material Symbols Outlined variable font under Apache-2.0; its header
  records the source revision and `tools/generate_material_symbols.py` records
  the font axes and generation procedure.
  A copy of the license is in `assets/MaterialSymbols-LICENSE.txt`.
* **winresource** (build-time only) — MIT; embeds the confirmed native Aurora icon.
* **base64** and **windows** — MIT and/or Apache-2.0; versions already used by
  the launcher dependency tree, now direct native dependencies for artwork data
  decoding and the Windows PNG file chooser respectively.
* **minisign-verify** 0.2.5 — MIT; the native preview core uses it to verify
  authenticated update offers and installers. Private signing material is not
  included. See https://crates.io/crates/minisign-verify for its source/license.
