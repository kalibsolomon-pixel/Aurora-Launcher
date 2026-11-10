# Aurora Launcher

A Windows launcher for Minecraft: Java Edition, built with Rust and Slint.
Keep separate instances for different Minecraft versions, loaders, and modpacks.
Sign in with Microsoft, choose an instance, and play.

**[Download for Windows x64](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/latest)**
 · [All releases](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases)
 · [Documentation](docs/README.md)
 · [Report an issue](https://github.com/kalibsolomon-pixel/Aurora-Launcher/issues)

## Get started

1. Download the installer from the latest release and run it.
2. Open **Aurora Launcher** from the Start menu.
3. Sign in with a Microsoft account that owns Minecraft: Java Edition.
4. Create an instance or choose an existing one.

The application executable is `aurora-launcher.exe`. New installers use
`Aurora.Launcher_<version>_x64-setup.exe`.

To upgrade from version 2.1.0 or earlier, run the 2.1.1 or later installer once
to adopt the new executable and installer names. Your existing instances and
application data stay in place. Later releases use the signed updater.

Updates are cryptographically signed. Windows publisher signing is currently
unavailable, so Windows may show an unknown-publisher warning. Release pages
include update signatures and SHA-256 hashes.

## What you can do

- Keep Minecraft instances separate, with their own mods, packs, and settings.
- Install exact Minecraft and loader versions, with compatible Aurora Client integration where available.
- Find mods, resource packs, and shaders on Modrinth, or import local content.
- Use managed Java runtimes or select a validated Java executable for an instance.
- Manage skins and capes, revisit saved destinations with Quick Play, and arrange Home widgets.
- Sync selected settings and files between compatible instances, with backups and conflict review.

Modrinth modpacks for Fabric, Forge, and NeoForge are available from version
2.1.1, including compatible additions to existing instances. CurseForge and Quilt
modpacks are outside this release's scope. See the
[2.1.1 release notes](docs/releases/NATIVE_2_1_1_NOTES.md) for loader and update limits.

Worlds stay local. Syncing does not automatically merge conflicting server edits.
The archived WebView application keeps its separate installation and update
authority. Moving data between the applications requires a backup and an explicit
migration.

## Build from source

Use stable Rust and the Microsoft C++ build tools on Windows x64. WebView2 is
not required. Node.js and npm are needed for release tooling; NSIS 3.11 builds
the Windows installer.

```sh
cargo run --manifest-path native/Cargo.toml --bin aurora-launcher
cargo build --manifest-path native/Cargo.toml --release --locked
```

The release executable is `native/target/release/aurora-launcher.exe`.
The Rust application is in `native/`; shared launcher logic is in `core/`.

```sh
cargo test --manifest-path core/Cargo.toml --locked
cargo test --manifest-path native/Cargo.toml --locked
npm ci
npm test
```

## Project documentation

- [Documentation index](docs/README.md)
- [Architecture](ARCHITECTURE.md)
- [Release and signing procedure](RELEASING.md)
- [Naming and upgrade compatibility](docs/releases/BRANDING_CONTRACT.md)
- [Contributor and repository boundaries](AGENTS.md)
- [MIT license](LICENSE)
