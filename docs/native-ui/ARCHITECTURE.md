# Native host architecture (NATIVE-1 / NATIVE-2)

## Layers

```text
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ WebView host (production)    │   │ Native host (preview)        │
│ src-tauri/src/main.rs + lib  │   │ native/ (crate aurora-native)│
│ Svelte UI ── Tauri IPC       │   │ Slint UI ── controllers      │
│ application_tauri.rs (thin)  │   │ native/src/api.rs (DTO view) │
└──────────────┬───────────────┘   └──────────────┬───────────────┘
               │  Host::from_tauri                │  Host::native
               ▼                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│ Application services: src-tauri/src/application.rs (host-neutral)│
│ 120 operations, DTOs, CommandError — identical code for both     │
├─────────────────────────────────────────────────────────────────┤
│ Launcher core domains (unchanged): instances, install, minecraft,│
│ fabric, forge, neoforge, runtime, auth, launch, modrinth, packs, │
│ cache/integrity/downloads, updates, appearance, cosmetics, …     │
├─────────────────────────────────────────────────────────────────┤
│ Platform integration: paths, credentials (Windows Credential     │
│ Manager), shortcuts, process supervision, host::open_path/url    │
└─────────────────────────────────────────────────────────────────┘
```

## The `Host` boundary (`src-tauri/src/host.rs`)

Before this initiative every command took `tauri::AppHandle` and used it for
four things only. `Host` provides exactly those, for either host:

| Need | Tauri host | Native host |
| --- | --- | --- |
| Local data directory | `app.path().app_local_data_dir()` | `SHGetKnownFolderPath(FOLDERID_LocalAppData)` / XDG + `com.aurora.launcher` (same location) |
| Version | `package_info().version` | `CARGO_PKG_VERSION` of the core (== package.json, enforced by `verify:version`) |
| Events | `Emitter::emit` to the webview (unchanged names and payloads) | serialized payload delivered to the Slint event loop |
| Background tasks | `tauri::async_runtime::spawn` | the native host's Tokio runtime |
| Open folder / browser | `tauri-plugin-opener` (unchanged) | `ShellExecuteW("open")` / `xdg-open`, structured, no shell |

`ManagedPaths` validation is unchanged and still applies to whatever the host
reports. `open_url` now refuses anything but `https://` in both hosts (it is
only used for the Microsoft authorization URL).

### Extraction method

The 120 `#[tauri::command]` functions in `application.rs` were made
host-neutral by a one-time scripted transform (recorded in the commit):
`AppHandle` → `Host`, `tauri::async_runtime::spawn_blocking` →
`tokio::task::spawn_blocking` (the same Tokio runtime under Tauri), two
`app.path()/package_info()` calls → `Host` methods, and the opener calls →
`host::open_*`. No command body logic changed.
`src-tauri/src/application_tauri.rs` was generated from the original
signatures: each wrapper keeps the command name, parameter names (the
frontend's argument keys), sync/async nature and return type, and calls the
identical function. `lib.rs` registers those wrappers, so the WebView IPC
contract (command names, DTOs, error codes, events) is unchanged.

### Feature gating

`tauri`, `tauri-build`, `tauri-plugin-opener` and `tauri-plugin-updater` are
optional behind the default `tauri-host` feature. With
`default-features = false` the crate builds the full launcher core without
Tauri or WebView2. Tauri-only modules (`window_activity`, the research
`performance_*` commands, the official updater wrapper) are gated. The
`windows` crate's `std` feature, previously enabled implicitly by Tauri
through feature unification, is now declared explicitly.

### Updater

`updates::launcher` keeps the official Tauri updater for the WebView host.
In the native host every launcher self-update operation reports
`updater_unconfigured` with an explanation: the published signed update is
the WebView NSIS installer, and moving the production update authority to a
native artifact requires a separate owner-approved migration (see
[`UPDATER_ASSESSMENT.md`](UPDATER_ASSESSMENT.md)). Aurora Client update
discovery and installation are host-neutral and work in both hosts.

## Native host (`native/`)

* `src/main.rs` — entry, worker-mode check (`run_artwork_decoder_if_requested`
  re-executes this binary for isolated image decoding), renderer selection,
  Tokio runtime (2 workers, ≤ 8 blocking threads), `Host::native`, Discord
  initialization, startup sequence.
* `src/api.rs` — the native view of the application services: builds requests
  from the documented camelCase JSON contract and reads responses into mirror
  types. DTO fields stay private in the core; the native host depends only
  on the same serialized contract as `src/lib/backend.ts`. Synchronous,
  filesystem-bound operations run on `spawn_blocking`, never on the UI thread.
* `src/ui.rs` — UI-thread state (`AppState`), async dispatch
  (`spawn(work, done)` → `invoke_from_event_loop`), continuation parking,
  notifications.
* `src/{home,instances,discover,accounts,settings}.rs` — one controller per
  domain, each bound to one Slint global.
* `src/events.rs` — routes host events (`install-progress`,
  `instance-progress`, `runtime-progress`, `auth-progress`, `launch-progress`,
  `launch-state`, `update-status`) to controllers.
* `src/theme.rs` — applies the persisted appearance: surface palettes mirror
  `src/app.css`; accents come from `appearance::AccentPalette`.
* `ui/` — `theme.slint` (tokens), `icons.slint`, `components.slint` (shared
  controls), `state.slint` (the UI state contract: structs and one global per
  domain), `pages/*.slint`, `app.slint` (shell, dialogs, notifications).

### Authority rules carried over

* The native UI never decides readiness: Home presents `get_play_readiness`
  exactly like `home.ts`, and Play re-validates in Rust.
* Stale async results are discarded by serial counters (readiness, content
  lists, Discover searches), mirroring the Svelte store.
* Content mutations apply only the authoritative inventory returned by Rust;
  failures re-read the list (no optimistic state).
* Configuration edits round-trip unknown configuration fields (`#[serde(flatten)]`)
  and send the base configuration as `expectedConfiguration`, so concurrent
  edits are rejected by the existing optimistic-concurrency check.
* Passwords never enter the app; sign-in opens the system browser; tokens
  never reach the native UI (only `AccountSummary`).

### Data compatibility

Both hosts use the same identifier (`com.aurora.launcher`), the same
platform data directory and the same persisted schemas, through the same
code. No migration exists or is needed. Development and acceptance builds
use `AURORA_DIAGNOSTIC_DATA_ROOT` (debug builds, or release builds with the
`disposable-data-root` feature only).

**Shared Windows credential entries.** Credential Manager targets are
`…/account/<profile-uuid>` and are not scoped by data root. A sign-in in a
disposable root therefore rewrites the same entry the production launcher
uses for that account (with a fresh, valid refresh credential), and signing
out in a disposable root would delete it. This is pre-existing behavior of
the shared core, not new to the native host; acceptance runs must not sign
out of an account that is also used in production.

### Rendering and idle behavior

* Software renderer by default (see [`BENCHMARKS.md`](BENCHMARKS.md)); a
  static window does not redraw.
* Transitions are ≤150 ms on control state changes only and are disabled by
  **Reduce motion** (persisted as `auroraMotionSpeed = 0`, the existing
  preference). The only continuous animation is the busy `Spinner`, which
  exists only while an operation is running and is static under reduced
  motion.
* Lists are plain models rebuilt on change; images (account heads) are
  decoded once and cached per account.
