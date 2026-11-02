# Phase E1 acceptance

## Implemented

- Launcher-owned schema 1 `launcher/gameplay-history.json` records process-timed sessions, terminal outcome, activity mode and bounded recent visits. A missing file initializes the schema. Malformed and future schemas fail without rewrite. Writes are synchronized within the process, use a unique sibling temporary file, sync and rename. A crashed open session is reconciled conservatively at the last recorded duration. Closed sessions beyond the latest 256 compact into per-instance UTC day totals; detail retains at most 16 visits per session; daily archives retain ten years and at most 8192 buckets. No per-tick writes or external telemetry.
- World save identity is a validated single directory component derived by the client from the integrated server's storage root. Multiplayer identity is a native-validated canonical host/port derived from Minecraft's current `ServerData.ip`. Display fields stay separate. The client never sends absolute paths. The launcher query DTOs return bounded playtime and opaque recent-target IDs with display labels, never the stored target or a command.
- The authenticated bridge accepts explicit schema v2 with new `worldSaveId`/`serverTarget` fields and preserves v1 unchanged. V1 cannot supply quick-launch identities. Discord still projects only its existing display fields through independent privacy toggles. No visible Home widget, quick-launch command, Java argument or executable path was added.
- Minecraft 1.21.11 mapped client bytecode confirms `--quickPlaySingleplayer` passes a nonblank world ID to `WorldOpenFlows.openWorld` and `--quickPlayMultiplayer` parses a server address then invokes `ConnectScreen.startConnecting`. This is research for E2, not an E1 launch change.

## Verification

- Launcher: Rust format check and all-target check passed; 548 unit tests passed, 21 ignored by default (one interop test also passed when explicitly run), plus 6 icon tests passed. Frontend: 116 tests passed, Svelte/TypeScript 0 errors and 0 warnings, production Vite build passed. The Tauri release build produced local MSI and NSIS bundles. The built app reached a native window and logged backend readiness.
- Client: `compileClientJava check build` passed; 373 tests passed, zero failures/skips. V2 worker loopback and launcher receiver loopback tests cover the negotiated frame, identity separation and native validation. A separate opt-in interop test launched the compiled Java worker from the client test output against the Rust v2 receiver; it passed the authenticated world, menu and server transitions. V1 tests remain passing.
- No widget catalog or Home component was changed. Normal Play still uses the existing resolve/validate/LaunchSpec/spawn path. Discord projection has a regression test proving validated history targets do not serialize even when display privacy toggles are enabled.

## Compatibility and limits

The current reviewed v2.1.3 client artifact remains pinned to schema v1. The new client code can speak v1 or v2, but an unreviewed artifact hash is deliberately denied an activity bridge by the production launcher. Normal Play and process-only Discord fallback continue. A later reviewed client release must pin its artifact hash before v2 identity collection is active in production. The Java/Rust interop check uses a test-only harness, not a real Minecraft v2 session. Current-session duration is checkpointed on transitions and finalized on process exit; an abrupt launcher crash discards time since the last checkpoint. A future Quick Launch action must look up opaque history IDs, revalidate a ready instance and world containment/server target, and reuse the existing launch pipeline.

Phase E2 may add Playtime, Recent Worlds and Recent Servers widgets plus validated Quick Launch. E2 is not started here.
