# Aurora Client activity bridge acceptance

Acceptance date: 2026-09-27, Windows x64. Nothing pushed or published. Aurora Client was inspected read-only, without a client build or source change.

## Source and artifact contract

- Launcher starting HEAD: `01a2be65fe3b3809ba873a15b9ba6ef9f9a0dea2`, 29 ahead / 0 behind origin/main.
- Origin/main and merge-base: `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`; fetched again and unchanged.
- Client inspected HEAD: `a1f0ab6d35fab2427a82b06be6128d7295b630d8`. Actual Java bridge classes and `LAUNCHER_ACTIVITY_PROTOCOL.md` agree on protocol v1.
- Reviewed bridge-enabled `aurora-2.1.2.jar`: 2,467,058 bytes, SHA-256 `eb2b06bc3955881ee9ff0dc561c25ced617de822a276c61fa3a9f79343602777`.
- Existing published 2.1.2: 2,450,086 bytes, SHA-256 `55ac97f7494daa3866bb3b4aa8d23e49b240fe5ced00fbf1742f7214fc77c52a`; it contains no bridge. Release metadata and the registered instance's artifact were not replaced. Identical version labels do not establish identical behavior.
- Production eligibility requires the reviewed digest, active deeply verified bootstrap content and Minecraft 1.21.11. Old/missing/disabled/unreviewed client content receives no bootstrap; generic process presence remains usable. No guessed production release infrastructure or silent upgrade was added.

## Native boundary and security

Each eligible launch generates a UUID session and 32 OS-random bytes (64 lowercase hexadecimal capability). An exclusive `127.0.0.1:0` listener uses Windows SO_EXCLUSIVEADDRUSE. Four child-only environment values hand off endpoint/session/capability/protocol. Every launch removes ambient bridge variables first. No capability is sent to Svelte, commands/events, arguments, config, Discord, logs or diagnostics. Session/snapshot debug output is redacted; owned capability/redaction buffers are zeroized. Normal supervisor redaction includes the capability, verified using a fixture that deliberately prints it on both child streams.

The receiver permits eight sequential attempts, one pending handshake, a 120-second startup window, a 1024-byte LF hello and two-second total hello/ack budget. Schema/session and fixed-length constant-time capability authentication precede acceptance. The exact accepted frame is its only write. The listener closes after authentication. No game commands, filesystem operations, reconnect protocol or heartbeat are added.

Activity is UTF-8 JSON plus LF, bounded to 4096 bytes before parse. CRLF/BOM, invalid UTF-8, duplicate/unknown fields, explicit null strings, unknown state/schema, wrong session, invalid/state-inappropriate fields and invalid strings fail. Names allow 128 code points; address 255. Controls, Unicode format characters and line separators are excluded, with trimmed text required. First snapshot is sequence 1/MAIN_MENU; later positive signed-64-bit sequences strictly increase and may skip values. Frames have a two-second total deadline after the first byte; authenticated idle does not expire.

Whole snapshots replace old identity. EOF/protocol failure/cancellation invalidate activity permanently. Exact-child exit disposes the receiver before output drains and terminal callbacks, preventing stale updates from resurrecting it. Failed listener setup does not fail Play; failed spawn drops its listener. Existing preparation reservation, final content/registry validation, Starting/Running exclusion and exact child supervision remain authoritative.

Gameplay lives only in private native handles. A bounded capacity-one wake coalesces notifications to the existing Discord actor. Discord setup/connection is independent; the bridge operates when Discord is unavailable. No log scraping, client source import, frontend socket, arbitrary endpoint or general RPC exists.

## Privacy and configuration

Master off projects null. Master alone exposes existing generic process-known presence and logo. Show World permits an authenticated human-readable singleplayer name; Show Server permits the distinct multiplayer display name. Show Server Address additionally requires Show Server. Missing names never fall back to addresses. While address consent is off, display names containing the supplied address or its host are suppressed after sanitization and case normalization, including host:port and bracketed IPv6 forms. Filtering happens before Discord serialization; world/server identity never survives a whole-snapshot transition.

Launcher configuration schema 4 requires all nine privacy booleans. Explicit schema-3 migration preserves all six previous preferences, selection, appearance and widgets, introducing the three gameplay preferences as false. Schemas 1/2 retain their existing explicit migration semantics. Unknown/malformed documents fail without overwriting them. Parameterized migration tests exercise every old preference combination and idempotence. Projection tests exercise every master/world/server/address combination across all three gameplay states, missing identity and names containing addresses.

Settings shows the three controls and disables address sharing unless server sharing is enabled. Connect/Reconnect retains real native connection semantics. Application setup required remains visible, with Connect disabled when no legitimate application ID was compiled in. No account Unlink fiction was added.

## Verification

- Final ordinary Rust suite: **543 passed, 20 ignored, 0 failed** (537 library plus six icon integration tests). The ignored tests are explicit fixture/manual acceptance entry points, not silently counted as passing.
- Separately invoked actual-Java interoperability: **1 passed, 0 failed**. Managed Java 21 loaded the actual reviewed JAR's bridge worker through a same-package temporary harness; the supervisor observed all three states, world/server identity, exact acknowledgement, normal exit 0 and activity cleanup.
- Separately invoked real Minecraft acceptance: **1 passed, 0 failed**. Details below.
- Frontend: **116 passed, 0 failed**. Svelte/TypeScript: **0 errors, 0 warnings**.
- Rust formatting and all-target check passed. Production frontend build passed. Version contract remains **1.0.0**.
- Final `npm run tauri build` passed: optimized native compilation completed in 4m 38s and both MSI/NSIS bundles succeeded. Executable: 10,164,224 bytes; MSI: 5,771,264 bytes; NSIS installer: 4,211,896 bytes. All use launcher version 1.0.0. Bundles were built, not installed or published.
- The exact repository release executable booted successfully against retained application data, with Home Ready, active published Aurora 2.1.2, Minecraft 1.21.11/Fabric 0.19.5 and the existing content inventory. The older installed launcher executable was explicitly excluded from this check.
- Final registered-instance production Play passed with the real restored account and validated managed Java. Minecraft reached the Aurora main menu; its normal Quit returned Home to Ready with **Last game exited with code 0**. No bridge was expected from the older published client. Missing Discord registration did not affect launch or exit. Settings confirmed all three new preferences off and address disabled, with pre-existing preferences preserved.

Existing deterministic suites still exercise final polish/widgets/account presentation, compatibility predicates/nested capabilities, ownership/dependency/removal protections, launch preparation races, configuration-change rejection, native diagnostics and exact process supervision. These passing suites are regression evidence; destructive/user-content UI workflows were not repeated against the user's instance.

## Live bridge acceptance

The live game fixture uses a disposable managed root beneath the acceptance TEMP directory. Only verified game/runtime copies, the reviewed client JAR, Fabric API and a temporary acceptance driver were placed there. No user saves/config/mods/accounts were copied or changed. The driver was compiled/remapped outside both repositories against existing read-only Minecraft/Fabric caches; client sources/build were never modified or rebuilt. A dedicated Minecraft server bound only to loopback in its own disposable root. Offline synthetic identity exists exclusively in the ignored test fixture, never the production launch path.

The actual reviewed client JAR authenticated through the actual launcher receiver under managed Java 21 and the real structured launch/supervisor boundary. The native observer verified:

1. MAIN_MENU.
2. SINGLEPLAYER with authoritative fixture world name, including while paused.
3. MAIN_MENU after leaving the fixture world.
4. MULTIPLAYER with separate fixture server display name and loopback address, including while paused.
5. MAIN_MENU after disconnect.
6. Normal client stop, exact-child exit 0, non-running state and cleared native gameplay handle.

The final test passed in 47.30 seconds. Earlier temporary-driver runs failed from incomplete remapping and stalled integrated-server teardown. Remapping included the actual Minecraft classpath; final fixture shutdown requests the integrated server's normal halt before world teardown. These were corrected harness failures and are not concealed as passing runs. The original known-good registered instance was never changed for these attempts.

This proves the actual client/native protocol and real gameplay transitions. It does **not** prove registered production Play with the unpublished bridge JAR; its production release metadata has deliberately not been changed. Registered Play with the existing published JAR is a separate fallback/regression check.

## Discord live acceptance and owner setup

**External blocker: no Aurora-owned Discord application ID is available in the build environment/repository.** Desktop Discord being installed/running cannot satisfy application registration. No borrowed ID, token, secret or invented asset was used. The renderer requests the established `aurora-logo` key; its remote registration/rendering cannot be verified without owner setup.

Real Discord generic presence, world/server/address visibility, master clear and reconnect are therefore **not live accepted**. Deterministic projection and fake/duplex IPC tests passed. [DISCORD_SETUP.md](DISCORD_SETUP.md) supplies the legitimate owner application/asset/build steps and the exact remaining live privacy matrix. Connection acknowledgement alone will not establish remote rendering.

## Evidence and repository safety

Audit logs, hash inventories, fixture harnesses and screenshots remain outside Git at `C:\Users\kalib\AppData\Local\Temp\aurora-bridge-audit-df564935-7bf6-46d1-9f16-91def5a7c7c3`. Screenshots: `screenshots/discord-settings.png`, `screenshots/production-game-main-menu.jpg`, and `screenshots/production-ready-exit-zero.png`. Settings/Ready captures exclude account/sidebar data; no capability, private world or server appears. No live Discord screenshot is claimed.

The starting baseline contains 181 tracked files and 607 protected untracked diagnostics. Final tracked count is 183: the activity bridge module and this acceptance document are the only additions. All baseline tracked files remain present; all 607 diagnostics remain byte-identical and untracked. All 374 client tracked files remain byte-identical and the client working tree stays clean. No deletion, missing critical manifest/lockfile/document, generated game/runtime data staging or cleanup command occurred. The pre-launch 239-entry user-data hash baseline was unchanged after isolated acceptance. The authorized final production game itself saved `options.txt` and initialized/updated `packetfixer.properties` and `fabric/indigo-renderer.properties`; the launcher integration did not edit those files. Existing saves, mod JARs, packs, registry, accounts and launcher preferences remain unchanged. Game logs naturally changed during the launch.

Functional commit: `1458b4e8b0568d89ca2d0125cdce457a3b8b1f82` (authenticated native activity, privacy migration/UI and tests). A separate documentation commit records architecture, setup and acceptance. Final branch is 31 ahead / 0 behind the unchanged origin/main; tracked working files are clean, with the original 607 untracked diagnostics preserved. Nothing pushed.

## Remaining qualifications

Client-side historical acceptance records 369 Java/26 Python checks, but native Windows initially failed 17 source-shape assertions because LF/CRLF differed. Its final source assertions used an external LF-normalized mirror while compiled classes came from original inputs. This is a qualification of client evidence, not an unqualified native Windows pass. No client suite was rerun or rewritten here.

Future reviewed release wiring, legitimate Discord application/logo setup and real rendered Discord privacy/reconnect acceptance remain outstanding. No push, publication, client release, website work, telemetry or remote control was performed.
