# Native UI-2 acceptance

**PARTIAL — IMPLEMENTATION INCOMPLETE**

## Repository and preservation

UI-2 starts at approved UI-1 commit `bc355b5e9617d22df96c587e3d46c272d7606aa0`. The approved branch `codex/native-ui-1` was clean in `C:\Dev\aurora-launcher-ui1`. The expected checkout `C:\Dev\aurora-launcher-native` was on `codex/native-2.0.1-release-record` at `8dcb308059ce25946270013727ab7d08f29212d8` with 13 unstaged release-related modifications. UI-2 uses `C:\Dev\aurora-launcher-ui2` on `codex/native-ui-2`. The worktree inventory included the old WebView checkout, the UI-1 reference and baseline, and several historical linked worktrees. Final implementation SHA is `d3a60ba233cc6598399120d895842d799f05d136`. The UI-2 baseline had 3,206 tracked paths and an empty index and worktree diff. The original tracked-file list was recorded in the Windows temporary directory as `aurora-ui2-tracked-bc355b5.txt`.

Critical starting SHA-256 digests

| File | SHA-256 |
| --- | --- |
| AGENTS.md | ffc19bbeb935553a8a90c7195fc4dd4eba0db7a66ab44c9fb944116239589e76 |
| ARCHITECTURE.md | 782423ef4dbfb45a95894a13d1d11e4bf0f0fc9e803471900c1e6d91b3d015e8 |
| RELEASING.md | aea1b3506464ae1eeee8775eab1aa4a7079e1c147bd35cf8d7e13ce0f68b6e81 |
| core/Cargo.toml | 9c059cd307555aa2cd4f7a7e665f29d44365c5a2949464b3573f79acd6d4466f |
| core/Cargo.lock | 731a034e1bd3cd097fe9659caa0c903ff4f5c7214d32fa0e0e7959807f5e91f4 |
| native/Cargo.toml | 0b0eb934209fd02b37990c413d962371cff629decb5ebc46cb6821a7daa726c5 |
| native/Cargo.lock | 47f6d5aa584063a4db33f3466d6e6969e23976a6c17e6558b658d4a0a698b162 |

## Pandora source findings

Pandora's upstream HEAD was `2d83d6f22c43e804d9b544e8dc98d49d4874e3af`. Its root MIT license credits Moulberry. UI-2 references the implementation and uses Aurora code. [Pinned Pandora source](https://github.com/Moulberry/PandoraLauncher/tree/2d83d6f22c43e804d9b544e8dc98d49d4874e3af)

- **Worlds.** `instance.rs` reads gzip NBT from each save's `level.dat`. `Data.LastPlayed` drives ordering and `Data.LevelName` supplies the title, with the folder as fallback. The world cache uses filesystem change notifications in `backend_filesystem.rs`, including `level.dat` and `icon.png`. This is saved-world discovery and a Minecraft-recorded time. I found no independently persisted observed world-entry history or active-world identity in the inspected backend, log stream, launch state, or schemas. The Quick Play page passes the folder name as the singleplayer target. [World loading and parsing](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/instance.rs) [Quick Play page](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/frontend/src/pages/instance/quickplay_subpage.rs)
- **Servers.** The same instance module reads the `servers` NBT list from `servers.dat`, skips hidden entries, and reads the name, IP, and saved favicon. `server_list_pinger.rs` enriches status and icon data, with the saved icon as fallback. Pings measure reachability. I found no dedicated recent-server or favorite history derived from actual joins. The saved list survives via Minecraft's file, and Pandora can pass an address from it as a multiplayer Quick Play target. [Saved list and world summary code](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/instance.rs) [Status pinger](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/server_list_pinger.rs)
- **Process and launch.** `log_reader.rs` consumes supervised streams, parses Log4j framing, sanitizes output, and sends log-display messages. Its inspected callers route the stream to the frontend. `launch/mod.rs` selects the argument groups using Minecraft version metadata and launch features, expands typed singleplayer/multiplayer/Realms targets, and gives the client a Quick Play log path. I found no reader for that log in Pandora's gameplay path. `quickplay_presets.rs` constructs vanilla or Fabric presets and their mod selections. `backend_handler.rs` dispatches discovery and launch requests, `backend.rs` owns the pinger, and instance persistence records process-level play statistics. These saved-list and launch paths use standard game files and arguments without a required gameplay-reporting mod. [Launch planner](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/launch/mod.rs) [Log reader](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/log_reader.rs) [Presets](https://github.com/Moulberry/PandoraLauncher/blob/2d83d6f22c43e804d9b544e8dc98d49d4874e3af/crates/backend/src/quickplay_presets.rs)

## Aurora detection and evidence

Reviewed the local Minecraft 1.21.11 client JAR against the SHA-1 in Aurora's Mojang metadata fixture (`ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd`) and verified the official mappings against SHA-1 `031a68bebf55d824f66d6573d8c752f0e1bf232a`, and used those mappings with `javap`. `ConnectScreen` sets the pending multiplayer destination from `ServerData`. `Minecraft` sets the singleplayer destination using the integrated save directory ID and level name. `ClientPacketListener.handleLogin` invokes `QuickPlayLog.log` after creating the client world and setting the game mode. `QuickPlayLog` writes one JSON entry with destination type, ID, name, game mode, and its own last-played time. The artifact is overwritten on later logins in the same process.

Aurora now supplies a per-process, randomly named `--quickPlayPath` on the reviewed 1.21.11 path for ordinary launches and Quick Play. After the existing supervisor reports exit or failure for a started child, Aurora reads that child's exact bounded artifact and folds a valid entry into existing history. The captured time is the file write time, bounded by the launch session. Activity-only fallback visits carry zero duration because the artifact contains no departure time. The optional Aurora Client bridge can still record richer transitions. A matching bridge visit spanning the fallback timestamp is deduplicated.

| Evidence | Treatment |
| --- | --- |
| Saved destination | Existence alone grants no recency |
| Requested Quick Play target | Launch input only |
| Valid child-written login record | Observed entry into the recorded world or configured multiplayer destination |
| Server status reply | Presentation only |
| Favorite toggle | Separate retained server preference |

Servers retain the existing bounded 100 recent nonfavorites plus 100 favorites. Worlds retain the existing bounded visit/session history. Both use the validated owning instance and canonical target, so identical world titles in distinct folders remain distinct. No history or widget layout schema migration is required. Existing data and legacy widget size presets remain readable. No Aurora Client changes are required for fallback capture.

## Quick Play and widgets

Both native row actions pass opaque IDs and the selected account into the existing secure Quick Play command. The command resolves the original instance, revalidates its target and readiness, and reuses the standard launcher argument and process supervision path. The 1.21.11 compatibility gate remains explicit. World validation checks managed ancestry and refuses redirected saves or `level.dat` paths. A spawn failure supplies no game observation.

Servers and Worlds use scrollable continuous rows with instance and time context. Server favorites are available in their own view. Status and normalized favicon retrieval occurs only on explicit refresh, through the existing bounded native status cache for at most 20 entries. A failed response is labeled “No response”. Rust retains target addresses, and the widget does not use enrichment's address-derived fallback name. World rows use observed display names and validated directory identity.

Playtime restores the older WebView widget's 7D, 30D and All selectors with a larger total, native bars, day labels and empty state. It uses the selected instance where applicable and the established UTC calendar buckets. All shows the all-recorded total with recent-period context. Session duration still comes from the supervised process and existing bounded archive, including abnormal exits that the supervisor observes.

The existing integer grid is now authoritative for drops and resizes. The editor preserves pointer offset, previews cell spans, highlights occupied targets, and refuses collisions rather than displacing neighbors. Explicit anchors are preserved, legacy auto-placement acquires anchors on a successful edit, and viewport reflow uses bounded first-fit placement without persisting window resize. Keyboard moves, resizes and Escape use the same interaction surface. Reset layout restores the native defaults and keeps unknown saved widget entries.

## Privacy and remaining acceptance gaps

- Fallback captures the last supported destination at child exit, including abnormal game exit when a complete artifact survives. Earlier visits within the same process can be replaced. A launcher restart during play loses the fallback association. The existing optional bridge retains its previous richer behavior.
- Minecraft login processing is the observation boundary. Proxy hops that produce no new identified target, Realms and unreviewed versions remain outside this fallback. Fabric/NeoForge compatibility has no real gameplay acceptance in this report.
- The separate saved-server and saved-world browsers, `level.dat` enrichment and world artwork are unfinished. Game mode is validated as part of the artifact but is not presented or persisted. Current world rename handling follows stable directory identity.
- Existing playtime tracks observed process duration and retains the established archive bounds. A launcher interruption can leave a partial or zero-length session. Timezone presentation remains UTC. A long-horizon weekly histogram is unfinished.
- The widget renderer is native Slint. Gameplay artifact parsing does not read chat, scan general log files, or enumerate world data. Explicit status refresh contacts the selected server endpoints through the established status service. The document stores history locally.

## Verification and local commits

Verification results and implementation commit are recorded below.

- Core all-target check and Rust formatting passed. The full core library run reported 1,116 passing, 31 ignored and one explicitly filtered Windows credential-store test. The focused singleplayer argument test added after that run passed separately. The test checks a world directory with a space remains one redacted structured argument. Existing process, server-retention, playtime, filesystem and release regressions ran in the core suite. A debug native launch created a real Windows window and initialized its disposable managed root.
- The initial native suite passed 57 application tests and five integration tests. The integrated widget revision passed all 57 application tests again. The final native application suite passed 58 tests, including reset preservation. The native software renderer exercised 720 × 520, 1120 × 760 and 1600 × 1000 logical windows and 100%, 125%, 150% and 200% scales. Existing pointer/keyboard grid tests and the new collision refusal regression passed.
- The observed multiplayer and singleplayer fixtures use synthetic game-written JSON and disposable paths. They cover request-shaped and invalid evidence rejection, persistence, distinct same-name worlds, and retention through an abnormal process outcome. They provide no authenticated gameplay result.

### Native render evidence

The images use disposable Slint scenes and synthetic data. The old WebView Playtime reference was inspected in the WebView checkout at `9b99db3ac226854c8ae96a86eed4c8d3084defa7`, in `src/lib/launcher/widgets/PlaytimeWidget.svelte`.

- [Playtime 7D](ui2-evidence/playtime-7d.png), [30D](ui2-evidence/playtime-30d.png), and [All](ui2-evidence/playtime-all.png)
- [Servers at typical size](ui2-evidence/servers.png) and [Worlds at minimum size, 200%](ui2-evidence/worlds-200pct.png)
- [Grid editor at typical size](ui2-evidence/grid-editor.png)

### Resource measurements

Compared optimized Slint software-renderer builds on the same Windows host. The UI-1 executable was matched by SHA-256 to the approved UI-1 evidence. UI-2 used the optimized disposable-data-root build of the integrated gameplay/widget revision. The final separator, reset and test-only changes were verified separately in the native suite. Each idle sample used a fresh empty managed root, an actual Windows window, a 7–8 second settle and a 12-second measurement. Each process recorded 0.000 CPU seconds during its sampled idle window at the available process-clock resolution.

| Sample | UI-1 working set | UI-2 working set | UI-1 private commit | UI-2 private commit |
| --- | ---: | ---: | ---: | ---: |
| First, UI-1 then UI-2 | 32.93 MiB | 46.65 MiB | 9.72 MiB | 9.82 MiB |
| Reverse-order repeat | 32.82 MiB | 32.83 MiB | 9.61 MiB | 9.87 MiB |

Private working set on the repeat was 8.75 MiB for UI-1 and 8.99 MiB for UI-2. The larger initial working-set gap did not reproduce. These short samples do not isolate the cause of the first result or establish a performance result for populated history, visible widgets, the editor, repeated navigation, or controlled server refresh. Those measurement scenarios remain open.

### Commits

- `43ba09fd0ae793b9b068f44b9feae5bd3db73949` — Capture native gameplay visits and refine Home widgets

- `d3a60ba233cc6598399120d895842d799f05d136` — Polish widget editing and verify world Quick Play arguments
