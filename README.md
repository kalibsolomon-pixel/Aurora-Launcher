# Aurora Launcher 2.0.0 — Native Rust Minecraft Launcher

Aurora Launcher is a Windows x64 launcher for Minecraft: Java Edition, with a
native Rust/Slint interface and isolated Minecraft instances. This major release
moves the interface away from WebView while retaining the verified Rust backend.

## Download and install

Native 2.0.0 is published through the separate **native-preview** channel as
`native-v2.0.0`. Get the [Windows x64 installer](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.0.0/Aurora.Native.Preview_2.0.0_x64-setup.exe)
or review its signatures and hashes on the [native release page](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/native-v2.0.0).
Run `Aurora.Native.Preview_2.0.0_x64-setup.exe`, open Aurora Launcher Native
Preview, sign in with Microsoft, and create or select an instance. Minecraft:
Java Edition ownership is required.

[WebView 1.5.1](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/v1.5.1)
remains the stable fallback. Native has separate installation, data, credentials,
and update authority; it does not automatically replace or migrate WebView
installations. Keep a backup before manually moving data between versions.
Updater signatures authenticate updates; Windows Authenticode publisher signing
is currently unavailable, so Windows can show an unknown-publisher warning.
Native automatic update discovery is awaiting its protected authority setup;
the signed manual installer is available now.

## Features

- Isolated Minecraft instances, exact loader selection, and optional compatible Aurora Client integration.
- Modrinth discovery and content management, local modpack import, installation, and reviewed updates.
- Managed Java runtimes and explicitly selected, validated custom Java executables.
- Resource-pack, shader, mod, skin, and cape management.
- Shared runtime/assets and coordinated verified downloads to avoid unnecessary repeated work.
- Optional cross-instance synchronization with backups, conflict review, and per-instance overrides.
- Responsive Home widgets, playback controls, and refreshed native Aurora icons.

Synchronization supports selected settings and files, plus safe server-list
additions. World synchronization and automatic merging of edited server entries
are unsupported. Performance depends on the system and workload; prior native
measurements and their limits are in the [benchmark record](docs/native-ui/BENCHMARKS.md).

## Documentation

Start with the [documentation index](docs/README.md),
[architecture](ARCHITECTURE.md), and [release instructions](RELEASING.md).
The [Native 2.0.0 release record](docs/releases/NATIVE_2_0_0_RELEASE.md) records
publication, signing, artifact hashes, validation, and remaining gates.
Historical development and acceptance reports remain in the organized docs tree.

## Development

Native Windows builds require stable Rust and the Microsoft C++ build tools.
The native interface does not require WebView2. Node/npm are used for release
tooling and for the retained WebView host.

```sh
cargo run --manifest-path native/Cargo.toml
cargo test --manifest-path native/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features --features native-preview
cargo build --manifest-path native/Cargo.toml --locked --release
```

Use `npm ci`, `npm run check`, and `npm run build` for the separate WebView host.
See [AGENTS.md](AGENTS.md) for repository and security boundaries.
