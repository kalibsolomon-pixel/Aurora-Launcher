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
* **winresource** (build-time only) — MIT; embeds the existing Aurora icon.
