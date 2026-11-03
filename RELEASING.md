# Releasing Aurora Launcher

Aurora Launcher is a native Rust/Slint application. Native 2.0.1 is the current
stable/latest GitHub release. The Svelte/Tauri application and WebView release
workflows are retired; old releases and their authority remain historical records.

## Build and validate

```sh
npm ci
npm run verify:version
npm test
cargo fmt --manifest-path core/Cargo.toml --all -- --check
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo check --manifest-path core/Cargo.toml --all-targets --locked
cargo test --manifest-path core/Cargo.toml --locked
cargo test --manifest-path native/Cargo.toml --locked
cargo build --manifest-path native/Cargo.toml --release --locked
```

`native/Cargo.toml`, `core/Cargo.toml`, their lockfiles and the release-tool npm
metadata share the native version. Never rebuild or replace published immutable
native artifacts; a future binary release requires a new owner-selected version.

## Protected publication

Use `.github/workflows/native-release.yml` from reviewed current main, with its
exact source SHA, numeric version, supported minimum installed native version and
the exact observed native authority parent. Signing, publication and authority
advancement retain their existing protected environments and review requirements.
Future native releases are stable GitHub releases and become Latest. Tags remain
`native-v<version>`; installers remain `Aurora.Native.Preview_<version>_x64-setup.exe`.

The existing package/data/credential/channel identifiers containing `native-preview`
are compatibility contracts, not a prerelease designation. Do not rename them or
automatically migrate existing WebView data. The native updater accepts only its
existing signed package identity. Never advance the historical WebView authority.

Node is release tooling only. The retained `@tauri-apps/cli` dependency signs the
existing encrypted minisign key format; it does not build or ship a WebView UI.
No new signing key or authority is introduced. Windows Authenticode remains
unconfigured; updater signatures do not provide Windows publisher identity.

Verify the exact signed installer/envelope, hash inventory, source/tag, immutable
public bytes and native discovery before reporting success. Preserve explicit
installer acceptance and describe any update/restart verification limits.

See [protected native procedure](docs/native-ui/PROTECTED_NATIVE_RELEASE.md),
[2.0.1 release record](docs/releases/NATIVE_2_0_1_RELEASE.md), and
[historical release instructions](docs/releases/archive/RELEASING_PRE_NATIVE_ONLY.md).
