# Phase D acceptance — 2026-09-26

## Git

Starting HEAD, fetched origin/main and merge-base were `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`, ahead/behind **0/0**, with no tracked modifications. C2 history and publication were left intact.

Implementation commits:

- `e591a38dd31e7cac28a435265cfeeea9aab2f15c` — `feat: expose validated Minecraft avatars and content capabilities`.
- `5c130d77328df6eb42d7ba6e6789f343a96c8947` — `feat: organize Home and account identity around native state`.
- `9c967ed417dfe6ce5dab8efc32a236a02ef99743` — `fix: honor native provider capability in content panels`.
- `6cfc8c477a50ca1387444a6e436ede4547860d28` — `fix: reuse form styling for Home instance selection`.

This acceptance/documentation commit is separate. Final hashes and ahead/behind are reported after committing. No Phase-D commit was pushed.

## Information architecture and Home

The established Home/Instances/Accounts/Settings/About navigation and visual tokens remain. Home selects, summarizes and launches; **Manage Instance** opens the existing Workspace. Sidebar shortcuts open management without changing the Play target. Accounts and launcher Settings stay global. Content, Java, configuration, validation, repair, provider lifecycle/dependencies and optional Aurora transitions stay inside an instance's Workspace.

Installed native DTOs drive the Vanilla/Fabric/version/Aurora Off/version summary. No filename or inventory reconstructs configuration. Native readiness/process snapshots drive Ready/Starting/Running and blockers; Play continues through the synchronized C2 native command. Selection and account snapshot filtering plus request generations prevent stale asynchronous display state. Backend capability data gates Modrinth Browse; Vanilla local content remains inspectable without a Fabric-mod claim. Fabric without Aurora remains a complete configuration.

## Accounts and profile/skin

Active Minecraft identity and multiple-account switching remain Rust-owned and independent of instances. Signed-out users can manage local instances while native authentication blockers disable Play. Account removal now confirms locally; it never claims Microsoft-wide revocation. No real account was removed during acceptance. Credential storage, token redaction and native launch authority remain intact.

The existing official authenticated Minecraft profile endpoint supplies UUID, username, skins/capes and the active skin's CLASSIC/SLIM model. Capes remain unused. Cosmetic metadata is optional: malformed or absent skins do not invalidate valid identity. Only native official profile data can supply a texture locator; frontend requests contain account ID and refresh flag only. Exact textures.minecraft.net texture identities are normalized to HTTPS, with userinfo/custom ports/query/fragment and redirects refused. PNG responses are bounded to 128 KiB, decoded with a 1 MiB allocation budget and restricted to static 64×32/64×64 textures. Native base-head/hat alpha composition produces 8×8 RGBA pixels; no 3D renderer or remote image URL reaches the frontend. CSP is unchanged.

A maximum of 16 validated heads is reused for 15 minutes in native memory only. There is no disk image cache or new persisted cosmetic document. Explicit refresh rereads official cosmetics with the existing session token and bypasses image reuse. Fallback is a local initial-based identity. Image failure never changes Play readiness or authentication lifetime. Session restoration and local account removal share a narrow process-local gate to prevent concurrent refresh rotation or restoration after removal.

## Automated verification

| Check | Result |
|---|---|
| Rust complete suite | **475 passed / 16 ignored / 0 failed** (469 library + 6 integration passed) |
| Frontend | **82 passed / 0 failed** |
| Svelte/TypeScript | **0 errors / 0 warnings** |
| Rust format | Passed |
| Rust all-target check | Passed |
| Frontend production build | Passed |
| Tauri production build | MSI and NSIS passed |
| Version contract | **1.0.0**, passed |
| Whitespace | Passed |
| Production boot / normal close | Passed; final rebuilt executable **exit 0** |

New native coverage tests official cosmetic metadata and graceful malformed-skin fallback, strict locators, PNG formats/dimensions/overlay transparency, bounded loopback transport/failures, cache corruption/expiry/bounds, concurrent restoration and removal. Existing Vanilla/Fabric/Aurora, provider ownership/transitions/rollback, stale/duplicate Play, runtime, auth, LaunchSpec and supervision tests remain passing. Frontend additions render actual compiled Svelte surfaces with mocked native DTOs, and test selection invariance/native Play targeting, account switching, stale results, fallback, many instances and long names.

Logs and all image evidence are outside the repository at `C:\Users\kalib\AppData\Local\Temp\aurora-phase-d-evidence`.

## Live production acceptance

The repository production executable was used, without installing over the user's older shortcut. Home was verified with all three safe acceptance instances: `C2 Vanilla acceptance` (Minecraft 1.21.11, Vanilla, Aurora Off), `C2 Fabric without Aurora` (Minecraft 1.21.11, Fabric 0.19.5, Aurora Off), and `Aurora performance acceptance` (Minecraft 1.21.11, Fabric 0.19.5, Aurora 2.1.2). Each reached Ready and displayed the correct native configuration. Manage Instance opened the selected Workspace; Vanilla Mods showed local-only files and no Modrinth Browse. Live testing found an old Fabric-only lifecycle error being requested for Vanilla; the focused capability correction removed that request, and the rebuilt production view passed.

Home selection used the native picker and keyboard arrows/type-ahead. After switching all configurations **all 85 snapshotted instance/content files were identical and all four registry records were byte-identical**. Only the selected ID in launcher configuration changed. Closing and reopening the production launcher retained Vanilla as the target. The original selection was restored through Home afterward, and the complete launcher configuration matched the original byte-for-byte, including OLED appearance and custom accent. Selection did not create, reinstall, repair or edit an instance.

The real account remained **Spxcterr**, signed in and active. Its official head displayed on Home, the sidebar and Accounts. Explicit avatar refresh completed successfully. Add/check/remove controls and account-management navigation were inspected without entering authentication dialogs or removing the account. Only one real account exists, so multiple-account switching, signed-out state, destructive removal and controlled missing/offline imagery were verified deterministically with the actual compiled components/native fixtures rather than by modifying real account state. No live authentication failure or network outage was induced; this is the limit of live cosmetic-fallback evidence.

| Home launch target | Observed acceptance |
|---|---|
| Vanilla / no Aurora | Starting → Running; duplicate Play disabled; Vanilla 1.21.11 title screen; normal **Quit Game**; supervised **exit 0** → Ready |
| Fabric / no Aurora | Starting → Running; duplicate Play disabled; Minecraft 1.21.11/Fabric title screen; normal **Quit Game**; supervised **exit 0** → Ready |
| Fabric + Aurora | Starting → Running; duplicate Play disabled; Aurora menu; `Aurora Client ready.` at **18:24:07** in the acceptance instance's `latest.log`; normal **Quit**; supervised **exit 0** → Ready |

The actual native C2 launch command remained the only path. No process-name termination or fabricated frontend Running state was used. The narrow Home launch also passed. Normal desktop (922×671 outer), minimum size (722×551 outer / 720×520 content) and maximized desktop (2048×1152 capture) were inspected. Home actions wrap at narrow width; account controls remain reachable; the instance sidebar scrolls independently while navigation/account identity stays visible. Full labels remain accessible when native options or sidebar names truncate. Long names/usernames and 80-instance options additionally have deterministic component coverage. Production closes returned exit 0; final style-only rebuild/boot proof is recorded with the evidence files.

## Screenshots

Before-change references: `before-home.jpg`, `before-instances.jpg`, `before-workspace.jpg`, `before-content.jpg`, `before-accounts.jpg`, `before-settings.jpg`, and `before-narrow-home.jpg`. The narrow Home reference is 722 pixels wide, including native window chrome. These were captured from the C2 repository release executable before building the new frontend. The installed desktop shortcut was older (registry schema 3), so it was closed rather than changing schema-4 acceptance data.

All paths below are under **`C:\Users\kalib\AppData\Local\Temp\aurora-phase-d-evidence`**, outside Git. Captures are actual release application windows. The final selector style reuses the existing field primitive; launch captures precede that styling-only adjustment, with unchanged launch/selection logic.

| Evidence filename | Proves |
|---|---|
| `after-restart-vanilla.jpg` | Vanilla selection survives production restart; correct configuration, identity, Ready |
| `after-fabric-no-aurora.jpg` | Fabric without Aurora is complete/Ready |
| `after-aurora-home.jpg` | Fabric + Aurora configuration and active identity |
| `after-instance-picker.jpg` | Native options distinguish all configurations and selected target |
| `after-workspace.jpg` | Manage Instance entry, instance-local detailed readiness |
| `after-vanilla-content.jpg` | Local Vanilla files, no Fabric Browse claim or misleading error |
| `after-accounts.jpg`, `after-avatar-refresh.jpg` | Real active username/head and safe account controls/refresh |
| `vanilla-starting.jpg`, `vanilla-running.jpg`, `vanilla-menu.jpg`, `vanilla-exit-ready.jpg` | Vanilla Starting/Running, duplicate disabled, menu, normal exit 0/Ready |
| `fabric-starting.jpg`, `fabric-running.jpg`, `fabric-menu.jpg`, `fabric-exit-ready.jpg` | Fabric/no-Aurora launch and normal exit 0 |
| `aurora-starting.jpg`, `aurora-running.jpg`, `aurora-menu.jpg`, `aurora-exit-ready.jpg`, `aurora-ready-log.txt` | Fabric + Aurora supervision/menu/readiness log/exit 0 |
| `after-narrow-home.jpg`, `after-narrow-accounts.jpg`, `after-large-home.jpg` | Minimum-size and large-desktop layouts/navigation |
| `selection-audit.txt`, `restoration-audit.txt` | Selection is content-invariant; original launcher config restored |
| `frontend-check.txt`, `frontend-tests.txt`, `rust-check.txt`, `rust-tests-final.txt`, `tauri-build-final.txt` | Exact automated/build results |

The final rebuilt executable was booted after the shared field-style adjustment. All three selections reached Ready again; the styled selector was keyboard-operated and checked at the minimum dimensions. It closed normally with **exit 0**, and final config/registry hash comparison passed. Final style evidence:

- [Vanilla Home](C:/Users/kalib/AppData/Local/Temp/aurora-phase-d-evidence/final-home-vanilla.jpg)
- [Fabric/no-Aurora Home](C:/Users/kalib/AppData/Local/Temp/aurora-phase-d-evidence/final-home-fabric.jpg)
- [Fabric+Aurora Home](C:/Users/kalib/AppData/Local/Temp/aurora-phase-d-evidence/final-home-aurora.jpg)
- [Instance picker](C:/Users/kalib/AppData/Local/Temp/aurora-phase-d-evidence/final-instance-picker.jpg)
- [Minimum-size Home](C:/Users/kalib/AppData/Local/Temp/aurora-phase-d-evidence/final-narrow-home.jpg)
- [Original selection after final boot](C:/Users/kalib/AppData/Local/Temp/aurora-phase-d-evidence/final-home-original-restored.jpg)
- [Final normal close](C:/Users/kalib/AppData/Local/Temp/aurora-phase-d-evidence/styled-production-exit.txt)

## Repository safety

Starting tracked-file inventory: **142**. All **606** pre-existing `.zcode*` diagnostic files were SHA-256 snapshotted and remained unchanged/untracked through implementation commits. No cleanup or broad deletion ran. Commit audits found **0 tracked deletions and 0 missing baseline files**; root docs, manifests, lockfiles, native configuration and source trees remain present. PNG reuses the already locked decoder version; lockfiles and the version contract are unchanged. A separate read-only selection snapshot records all four instance registry records and 85 manifest/provider/mod/config/content files for the live switching comparison.

The original selected ID `413e831999bd444a8e0df2d22fc6d134` was restored. The full original launcher configuration is byte-identical after acceptance, and the registry stayed byte-identical. Normal gameplay may write its own logs/configuration; the 85-file invariance comparison was performed before gameplay to isolate selection behavior. No installer/provider/repair mutation was performed. Final untracked state is the same **606 protected diagnostics** once this report is committed; no screenshots, build/game/runtime data or diagnostic files are staged. Final audits repeat all 606 hashes and compare the tracked baseline before the documentation commit.

## Deferred

No Forge, NeoForge, Quilt, CurseForge, cross-process locking, durable crash recovery or automatic dependency conflict resolution work began. No per-instance account assignment, 3D model renderer, instance deletion, automatic content upgrades or next-phase work was added. Stop after Phase D for review.
