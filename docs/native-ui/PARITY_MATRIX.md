# Functional parity matrix (native host vs WebView host)

**NATIVE-9:** current additions, the complete surface audit and explicit remaining
release blockers are in [NATIVE_9_ACCEPTANCE](../../NATIVE_9_ACCEPTANCE.md).
Historical "Done" rows never imply owner-attended account or installer acceptance.

**NATIVE-8 continuation:** the current status of shared installation/downloads,
native cards/navigation/icons, widgets/media, custom accent, Discord, chrome and
Borealis is recorded in [NATIVE_8_ACCEPTANCE](../../NATIVE_8_ACCEPTANCE.md).
Its evidence and explicit limitations supersede the corresponding historical
rows below. Native packaging/updater authority remains separately gated.

**Historical NATIVE-6 baseline.** NATIVE-7 supersedes the rows for single-click
input, Home selection, selection styling, Discover/project artwork/details,
modpack install/merge, 3D skins/capes, PNG import, accounts and native branding.
The current functional/verification table is in
[NATIVE_7_UI_COMPLETION_ACCEPTANCE](../../NATIVE_7_UI_COMPLETION_ACCEPTANCE.md).
The original measurements and acceptance evidence below are preserved.

Status values: **Done** (implemented against the authoritative backend and
exercised), **Done (unexercised)** (implemented and compiled, not driven end
to end in this session), **Partial**, **Missing** (backend exists, no native
UI yet), **Blocked** (needs an owner decision or external dependency).

Acceptance runs used a disposable data root (`AURORA_DIAGNOSTIC_DATA_ROOT`)
and were driven through Windows UI Automation (`native/tools/uia.ps1`);
screenshots are in [`screens/`](screens/).

## Home and launch

| Workflow | WebView behavior | Native status | Test performed | Result / evidence |
| --- | --- | --- | --- | --- |
| Selected-instance display | hero with name, version, loader, Aurora | Done | fresh root → created instance shown | `screens/dev-home-ready.png` |
| Instance switching | picker on Home | Done (unexercised with >1 instance) | — | tiles render when 2+ instances |
| Play readiness | Rust `get_play_readiness`, blockers | Done | missing Java + missing account blockers shown with actions | `dev-home-ready.png` |
| Blocker actions | Sign in / Install Java / open instance | Done | "Install Java" installed and verified Java 21 (402 files) | `dev-java-done.png` |
| Play | Rust-validated launch | Done | authenticated launch of Aurora Client 3.0.0 on Minecraft 1.21.11 | `dev-home-starting.png`, `dev-game-title.png` |
| Launch progress | `launch-progress` phases | Done | phases rendered | `dev-home-starting.png` |
| Running state | `launch-state` | Done | "Running" while `javaw.exe` child of the native launcher ran | `dev-home-running.png` |
| Exit handling | exit code / failure diagnostic | Done | normal window close → exit code 0 shown, world saved | `dev-home-after-exit.png` |
| Recent activity widgets (playtime, worlds, servers, quick play) | Home widgets | Missing | — | backend commands exist (`get_playtime_summary`, `get_recent_worlds`, …) |
| Error reporting | inline + toasts | Done | sign-in cancel / failures surface | — |

## Instances

| Workflow | Native status | Test | Result |
| --- | --- | --- | --- |
| Create (Minecraft version, platform, Aurora) | Done — platforms from `platformCapabilities`, default version from the newest Aurora release | created Fabric + Aurora 3.0.0 instance with real downloads (4677 files) | `dev-create-progress.png`, `dev-after-create.png` |
| Loader version pinning | Missing (Automatic only) | — | WebView offers "choose version" |
| Snapshot versions | Done | — | checkbox reloads list |
| Select | Done | auto-selected first instance | — |
| Rename | Done (unexercised) | — | expected-name concurrency check in Rust |
| Configure memory / JVM args / window size | Done (unexercised) | — | sends `expectedConfiguration`, preserves unknown fields (unit-tested) |
| Performance modes | Partial | — | memory advice shown; mode picker missing |
| Validate installation | Done (unexercised) | — | read-only `validate_instance` + runtime status |
| Repair / retry install | Done (unexercised) | — | `retry_instance_install` or `install_instance_configuration` |
| Recovery states | Partial | — | status labels mirror `instanceStatus.ts`; recovery detail text not shown |
| Delete instance (exact-name confirmation) | Done (unexercised) | — | — |
| Instance icons | Missing | — | letter glyphs only |
| Aurora enable/disable transition | Missing | — | `preview/apply_aurora_transition` exist |
| Open instance folder | Done (unexercised) | — | `ShellExecuteW` |

## Minecraft installation

| Workflow | Native status | Evidence |
| --- | --- | --- |
| Version selection | Done | Mojang manifest list loaded |
| Fabric | Done | Fabric 0.19.5 installed |
| Forge / NeoForge | Done for creation with automatic loader (unexercised) | platform list from Rust |
| Download, integrity, safe extraction, dependency resolution | Done (same core) | 4677 files verified |
| Progress | Done | `instance-progress` + `install-progress` |
| Cancellation | Missing (same as WebView: no cancel for installs) | — |
| Failure recovery | Done (core) / Partial (UI) | failed creation leaves retryable record; Repair button |

## Mods, packs, shaders, Discover

| Workflow | Native status | Notes |
| --- | --- | --- |
| Installed mods list, enable/disable, remove with confirmation | Done — list exercised (Aurora Client, Fabric API); toggle/remove not exercised | uses `canToggle`/`canRemove`; `screens/workspace-mods.png` |
| Resource packs / shaders list, toggle, remove | Done (unexercised) | shader activation shown as "Selected in game" |
| Filter installed content | Done | — |
| Discover: search, kinds, install into chosen instance | Done — browsing exercised with live Modrinth results; install not exercised | `screens/discover.png` |
| Categories, sorting, project details, version choice | Missing | — |
| Project artwork | Missing | letter/placeholder glyphs |
| Modpacks (browse, install, merge, update) | Missing | backend commands exist |
| Content updates (check, apply, bulk, policy) | Missing | backend commands exist |
| Conflict detection / Fabric aliases | Done (core) | enforced in Rust on install and readiness |

## Accounts and appearance

| Workflow | Native status | Evidence |
| --- | --- | --- |
| Microsoft OAuth (system browser, PKCE) | Done | sign-in completed via the system browser's existing Microsoft session |
| Account list / select / sign out | Done (select/sign-out unexercised — see credential note) | `screens/accounts-dialog.png` |
| Session refresh, entitlement | Done (core) | Play restored the session and launched |
| Auth progress phases | Done | `auth-progress` |
| Account head avatar | Done | — |
| Themes (Aurora Dark, Midnight, OLED) | Done (unexercised) | persisted via `set_appearance` |
| Accent presets | Done (unexercised) | Rust-derived palettes |
| Custom accent color | Done (unexercised) | `#rrggbb`, validated and derived in Rust |
| Borealis background | Missing (preference preserved, not rendered) | — |
| Reduce motion | Done | maps to `auroraMotionSpeed = 0` |
| Current skin, skin library, capes | Done — load exercised against the real account; Use/Wear/Save/Remove not exercised (they change the live Minecraft profile) | `screens/skins.png` |
| Skin import from file, 3D character preview, favorites editing | Missing | needs a native file dialog (e.g. `rfd`) and a skin renderer |

## Settings, desktop, updates

| Workflow | Native status | Notes |
| --- | --- | --- |
| Desktop / Start menu shortcuts | Done (unexercised) | same `shortcuts` module |
| Open launcher folder | Done (unexercised) | — |
| Aurora Client update discovery | Done | startup check + "Check now" |
| Aurora Client update review and apply | Done (unexercised) | fingerprinted preview → confirmation → `apply_client_update`; this branch's Client manifest endpoint answers HTTP 404 (fixed on `origin/main` 1.5.1), so no update could be offered |
| Launcher self-update | Blocked (by design) | see [`UPDATER_ASSESSMENT.md`](UPDATER_ASSESSMENT.md) |
| Discord settings | Missing (integration initializes) | — |
| About / version / data root / Slint attribution | Done | — |
| Accessibility preferences | Partial | reduce motion only |
