# Native-only stable Launcher

The owner retired the Tauri/WebView interface and selected native Rust/Slint as
the only supported Launcher application. GitHub release `native-v2.0.0`, release
ID `431029795`, is now **stable and Latest**, with `prerelease: false`.
Its immutable source tag, installer, hashes, signatures and signed discovery
bytes were not changed. The existing installer was already entirely native.

## Active source and release policy

- `native/` is the only UI. The shared Rust launcher library moved from
  `src-tauri/` to `core/`, retaining its existing Rust library/API identity.
- The Svelte source, Vite/SvelteKit configuration, Tauri host, IPC wrappers,
  WebView updater, capabilities and bundler configuration were removed.
  Both active Rust lockfiles are free of Tauri, Wry, WebView2 and WebKit crates.
- Native commands, core tests, runtime/install/authentication/synchronization
  behavior and native updater remain Rust-owned. The update facade now delegates
  directly to the existing native implementation and retains check deduplication.
- Only native candidate/release workflows remain active. Future releases are
  stable/latest; their signature/inventory/source/ref protections are retained.
  The old UI's workflows/tools/assets remain historical records under docs.
- Node is release tooling only. The existing `@tauri-apps/cli` signer remains for
  the existing encrypted minisign signing identity; it is not an application UI
  dependency and does not build or ship WebView content.
- Existing native-preview package, data, credential, channel and installer names
  remain compatibility contracts. The name does not make GitHub 2.0.0 a prerelease.
  Existing WebView installations, user content, release artifacts and historical
  WebView authority are preserved. No automatic migration or uninstall occurred.

The tracked-file audit accounts for all 3,321 baseline paths: 299 explicit moves,
151 authorized legacy UI removals, and no unexplained missing files. Local build
outputs and schemas in the retired checkout location were preserved and ignored.
No build/game/runtime data or private credentials are staged. Active Markdown
links follow moved files; historical UI links target the immutable release source.

## Verification

| Check | Result |
| --- | --- |
| Core/native Rust formatting | Passed |
| Core/native all-target checking | Passed |
| Core regression tests | 1,109 passed; 31 existing ignored |
| Native tests | 52 passed plus 5 picker integration tests |
| Active release/signature/authority/policy tests | 8 passed |
| Workflow validation | Both native workflows pass actionlint 1.7.12 |
| Optimized native build and NSIS package | Optimized build passed; final NSIS package identity/hash check recorded with local evidence |
| Real native boot | Disposable empty managed root; real Home observed, then normal window close and clean shutdown |
| Stable public release | Native 2.0.0 is Latest; existing immutable assets and native signed authority preserved |

The runtime smoke used the new debug executable and the existing diagnostic-only
data-root override, never owner accounts/instances. Prior gameplay and signed
update/restart acceptance were not repeated or claimed as new production results.

## Website

[Website PR 3](https://github.com/kalibsolomon-pixel/Aurora-Website/pull/3), merged
as `47a89550b58ea40d296f76bc3782bd67d2dd456b`, makes the verified native installer
the primary download and labels WebView 1.5.1 historical. All ten website checks
and both hosted CI checks pass. Existing styles and deployment guards remain.
The local page was visually inspected. Public website hosting still lacks the
owner's Cloudflare account/project/credential; GitHub distribution is live.
