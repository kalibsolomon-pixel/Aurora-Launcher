# Final release polish acceptance — 2026-09-26

This supplements, and does not replace, the earlier phase and prepublication reports. The final release executable was built from `97220355dfe4cda5f0298fedd57a71bdc0dc7489`; subsequent changes are documentation only. No push or publication was performed. Discord profile/logo acceptance remains blocked by the missing legitimate Aurora-owned application registration.

## Git and preservation

Starting HEAD: `77c74661e1cbc029a66742e7523cc59c762d6d79`. Starting origin/main and merge-base: `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`, ahead 20 / behind 0. A final fetch returned the same remote commit. Four implementation commits were added without modifying earlier history:

- `cb0d65d`: visible guarded installed-mod trash action.
- `6aa5842`: matching frontend removal-blocker DTO.
- `93f4730`: persisted Home layout and isolated native Discord worker.
- `9722035`: widget renderers, editor, Settings and frontend coverage.

This acceptance, architecture and Discord setup documentation is the fifth new commit. Final ahead/behind is 25 / 0 when that commit is created. Its exact HEAD is reported in the final handoff because a commit cannot embed its own hash.

The baseline contains 161 tracked files and 607 protected untracked files (606 `.zcode*` files and one pre-existing diagnostic). Audits prove baseline tracked paths remain tracked and present, protected untracked hashes remain identical, critical documents/manifests/lockfiles/native configuration remain present, and no generated build/runtime/game/screenshot data is staged. No deletion, history rewrite, broad cleanup or cross-repository edit occurred.

## Installed Mods

Rows retain artwork, enable/disable switch and overflow, with an accessible compact trash action alongside them. Local/bootstrap removal names the exact file, requires confirmation, and uses the existing native removal command. Cancel does not mutate disk. Provider removal uses the existing native preview/fingerprint/apply lifecycle, including retained dependencies and provenance/ownership guards. The duplicate overflow removal action is removed; updates and dependency details remain there.

Inventory projects the same native dependency blocker used at mutation time. A blocked trash action is disabled with its reason visible. Native containment, expected current hash, ownership, rollback and dependency enforcement remain authoritative. Instance changes discard pending confirmations/previews.

Live acceptance used only a newly created disabled metadata-only `aurora-polish-disposable.jar.disabled`. Cancel preserved its SHA-256 `b60811bb95fb77e281cd972a981fda79ed751a04c9ccf9c4a0021d19c37967e1`; confirmation removed it and refreshed inventory. Existing mods were not toggled or removed. Native provider lifecycle scenarios are covered by the full deterministic suite, rather than a destructive live provider transaction against user content.

## Home widgets

The frontend catalog maps stable IDs to dedicated renderers; Rust owns validated layout persistence through narrow typed commands. Configuration schema 3 adds `homeWidgets` and non-secret Discord preferences. Schema 1/2 migration preserves selection and appearance, adds conservative defaults, and persists on the next ordinary save. Malformed/unsupported documents fail deliberately without reset or overwrite.

Defaults enable Instance Details and Content Summary, both Small; Session is optional and disabled. Instance Details uses installed native facts. Content Summary uses local native inventory and explicitly distinguishes pack counts from activation. Session describes only the selected instance's latest supervised process in this launcher session, including known start/exit values; it invents no durable play history.

Small occupies one column, Wide spans two, and Large spans two with additional minimum height. The narrow layout clamps to one column. Registered widgets declare allowed sizes. Customize Home exposes keyboard-operable move earlier/later, size and hide controls; normal Home has no editing chrome. Settings enables/disables each widget and resets the layout to the documented defaults. An empty enabled selection collapses the region. Hidden/unknown IDs retain their placement; unknown IDs have no renderer and never load code. New registry entries/renderers extend the same storage model. Skin/cape widgets remain deferred.

Live checks covered default Home, edit controls, Wide size, reordering, hiding Instance Details, enabling Session, real process state, full release-app restart persistence, responsive narrow rendering and reset to defaults. Deterministic coverage additionally covers empty/all-disabled layout, size bounds, unknown IDs, errors retaining prior state and store restart/reset behavior.

## Discord architecture, Settings and privacy

Research supports activity-only local Discord desktop RPC over IPC, without account OAuth or linking. References and exact registration steps are in `DISCORD_SETUP.md`. Aurora uses a small auditable native transport, not a downloaded helper or the archived RPC library. It reuses Tokio with its net feature; no new crate was added. Windows fixed named pipes and Unix standard local sockets have bounded frames/events and timeouts. READY plus nonce-matched command acknowledgements establish connection status; raw Discord responses never become diagnostics.

A legitimate public `AURORA_DISCORD_APPLICATION_ID` must be supplied at build time. The owner must name the application Aurora Client and upload the canonical Aurora mark with registered key `aurora-logo`. No borrowed/fabricated ID, logo key masquerading as an uploaded asset, bot token or client secret exists. This build shows Application setup required and disables Connect. Actual Discord profile/logo rendering was not tested or claimed.

With registration, Connect to Discord performs genuine local connection; Reconnect retries a dropped connection. Connection alone publishes no activity. There is no Link/Unlink because no account link exists. The master Rich Presence toggle persists, defaults off, and clears registered activity when disabled. Independent instance-name, Minecraft-version, platform/loader, Aurora-active and elapsed-time toggles all default off. The worker retries enabled connections on a bounded cadence, clears on orderly shutdown, and cannot block or fail Play. Not detected, closed and failed outcomes remain sanitized.

Generic In Launcher / Starting Minecraft / Playing Minecraft is derived from native supervisor state. Optional version/platform and instance name derive from the validated instance/plan; optional Aurora active denotes the verified launch-time bootstrap state, not continuous client health. Elapsed time derives from the supervised process start. These are allowlisted activity-only projections: session credentials, access/refresh tokens, launch arguments, paths and log bodies are absent from the activity type.

World/server are explicitly unavailable. No existing authoritative client activity bridge was found; process running is insufficient to infer menu/world/server. No gameplay log scraping, remote listener or client repository change was introduced. The architecture documents the future bounded/versioned local capability-scoped activity-only contract tied to the exact supervised child. World name and friendly server name must be separate opt-ins; raw server address requires separately clear disclosure and world paths are excluded. No ineffective world/server toggles are presented today.

## Tests and production acceptance

All final checks passed:

| Check | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| Frontend tests | 115 passed, 0 failed |
| Rust formatting check | passed |
| Rust all-target check | passed |
| Rust tests | 507 library + 6 integration passed; 16 library tests ignored; 0 failed |
| Frontend production build | passed through final Tauri build |
| `npm run tauri build` | passed; final optimized release and both bundles generated |
| Version contract | launcher/native/package/installers remain 1.0.0 |
| Real release executable boot | passed with existing selected instance, restored account and 3D player |

Final bundles are `src-tauri/target/release/bundle/msi/Aurora Launcher_1.0.0_x64_en-US.msi` and `src-tauri/target/release/bundle/nsis/Aurora Launcher_1.0.0_x64-setup.exe`. They were generated, not installed over the user's installation or published. An initial UI launcher lookup opened the older installed executable; it was closed, and acceptance was performed against the verified `C:\Dev\aurora-launcher\src-tauri\target\release\aurora-launcher.exe` process.

Final real game: Aurora Client 1.21.11, Fabric 0.19.5, Aurora 2.1.2, managed Java 21. Running disables duplicate Play and Session shows the same native process state. Game log evidence: 22:54:51 Initializing Aurora Client, 22:54:52 Aurora Client ready, 22:55:34 Stopping. The Aurora-customized menu was visually observed. Quit was clicked normally in that menu. Native supervisor reported exit code 0, Home returned Ready and Session showed exit 0.

User-data hashes were compared before and after removal and launch. Accounts, instance registry, existing mods and installed-state documents remain unchanged. Intentional configuration migration/preferences changed; the game normally rewrote `options.txt` after its initial accessibility prompt/normal quit. That game-owned write was preserved, not rolled back.

## Evidence

Evidence remains outside Git under `C:\Users\kalib\AppData\Local\Temp\aurora-final-polish-20260926-222449`. Logs include `frontend-check.txt`, `frontend-tests.txt`, `rust-check.txt`, `rust-tests.txt`, `production-build-final.txt`, baseline inventories/hashes, customized configuration and bounded `game-ready-normal-quit.txt` excerpts. No credentials or launch arguments were captured.

Useful screenshots under that root's `screenshots` directory:

- `03-home-editor.png`: editing controls beneath unchanged primary Home/player composition.
- `04-home-wide-editor.png`: Wide widget layout and controls.
- `05-home-reordered.png`: intermediate reorder inspection; definitive persisted order is shown in 08.
- `06-discord-settings.png`: missing setup, master/details off, world/server unavailable.
- `07-home-persisted-restart.png`: restarted release in narrow layout.
- `08-home-customized-restart.png`: persisted Wide Content Summary first, hidden Instance Details, enabled Session, full-width 3D player.
- `09-mods-trash-artwork-blockers.png`: switches, trash, overflow, artwork and native blockers.
- `10-mod-remove-confirmation.png`: exact disabled disposable file and permanence warning.
- `11-mod-remove-cancelled.png`: fixture retained after Cancel.
- `12-mod-removed.png`: refreshed empty matching inventory after native confirmation.
- `13-launch-starting.png`: launch request with duplicate action disabled; not proof of a Starting label.
- `14-home-running.png`: Running and session start, duplicate Play disabled.
- `15-minecraft-menu.png`: real Aurora game menu and Quit.
- `16-exit-zero-ready.png`: native exit 0, Ready and Session exit result.
- `17-home-reset-defaults.png`: restored default widget order/sizes and Ready after normal quit.

## Deferred and publication gate

No skin upload/cape/cosmetics mutation, widget marketplace, additional loader/provider, modpack, cloud sync, cross-process locking, durable crash journal or website work began. Authoritative world/server presence awaits a separately reviewed client bridge. Real Discord connection, profile/logo rendering, presence transition/clear and reconnect acceptance await the owner-provided legitimate application ID/assets. This is a local review handoff, not publication approval.
