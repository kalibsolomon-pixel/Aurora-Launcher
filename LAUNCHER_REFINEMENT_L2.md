# Launcher refinement L2 — Instance management and Recent Servers

## Verdict and source identity

**COMPLETE.** Recent Servers now retains substantial local history with scrolling and persistent favorites. Rename is prominent and reliable. Supported instance settings have been audited and connected to native persistence, and the redundant Aurora Configuration card has been removed from Settings.

Branch: `codex/client-3-production-authority`.

- Starting source: `f7cbd54a8e8481e35f1babe0ec0653cf1a624e2c` (completed L1).
- Ending implementation source: `e6293148ce82e9d86c52d2a4239f857ed4b3026e`.
- This report, architecture notes, sanitized evidence and the isolated review harness are a subsequent documentation/evidence commit titled **Record L2 configuration audit and desktop acceptance**. Its exact identity is available from `git log -1` at delivery; it does not change shipped behavior.

No push, publication, installer replacement or L3 implementation was performed. L1 implementation and evidence were preserved.

## Recent Servers architecture

The authoritative source is the existing Rust gameplay recorder and activity bridge used by supervised Minecraft sessions. Structured activity supplies validated world/server targets and display labels; session timing supplies visit end times and durations. This work does not scrape owner logs or import another launcher's history. The local launcher-owned `gameplay-history.json` remains the only history document. Records are partitioned by stable instance ID, not account. Home can show entries across instances, and missing-instance entries remain unavailable.

Previously the widget queried five entries and displayed only three in its small presentation. Storage also retained bounded sessions/visits, so merely raising a display limit could not preserve old favorites. Schema 2 adds durable server summaries to that same document, folded before session pruning. Existing 256-session, 16-visit and daily-playtime bounds remain in place. Server duration is a retained-session aggregate with a preserved previous maximum, not a new exact lifetime counter; the server list does not present it as lifetime playtime.

Identity retains the established `ServerTarget` parser: lowercase hostnames, canonical IP representation and explicit default port 25565. Distinct ports and different instances remain distinct. The opaque SHA-256 target identifier keeps the existing instance-plus-target formula. Labels never determine launch addresses. The existing sanitized display label is preserved separately; native status enrichment can overlay server name, display address/port, favicon, MOTD and online/offline status.

Retention is **100 nonfavorite server identities plus up to 100 favorites**, globally across instance partitions. Favorites are not evicted for inactivity. Recent shows the newest 100 retained entries, including favorites where they fall in that order; Favorites shows every retained favorite, including older ones outside Recent. The total native query is bounded at 200. Unfavoriting an old entry returns it to normal retention and may evict it. Equal timestamps use stable identifier ordering. Rejoining a canonical identity updates the existing row and its order.

The schema-1 migration accepts only its exact known validated shape, preserves sessions and archive, and initially runs in memory. Reading does not alter the original file. A subsequent successful atomic mutation writes schema 2. Unknown versions, unexpected fields and malformed documents fail deliberately without replacement. This cannot recover records already pruned by an older build.

The Home list is a 246 CSS-pixel vertical scroll region, showing approximately four compact rows. It has a named keyboard focus stop, PageDown/arrow scrolling, accessible star controls, Recent/Favorites buttons, empty/loading/error states and truncated long labels with full text available through titles. World history keeps its prior presentation. Status enrichment is lazy for visible/nearby server rows, batches no more than the existing 20-entry native limit, and preserves native concurrency, timeouts, cache/backoff and parsing protections. No polling, cloud sync, telemetry, third-party history submission or automatic quick connection was added. The existing direct status query occurs only when a row enters view; merely retaining or favoriting an old server does not poll it.

Quick join still accepts only the opaque history identifier and resolves its instance and server natively. It then uses normal native readiness, session, managed Java and locked launch validation. The frontend cannot supply a launch address or authorize spawning.

## Configuration audit

Classifications describe the starting defect and final behavior. The complete `InstanceConfiguration` fields are Minecraft version, Aurora association, loader policy, memory, JVM text and optional window dimensions. Registry identity, lifecycle and installed pins are deliberately separate.

| Property | Classification / initial finding | Final control and authority |
| --- | --- | --- |
| Display name | Editable; existing control was inconspicuous and interaction/synchronization incomplete | Shared header/Settings Rename dialog; Rust validation and atomic registry save; stable ID unchanged |
| Desired Minecraft version | Editable and exposed; saved snapshots could disappear from filtered options | Official version selector, Include snapshots discovery filter, always retain saved value; explicit installation required |
| Loader kind | Editable native capability was missing from Settings | Vanilla, Fabric and NeoForge capability choices; original Aurora association requires Fabric; unsupported Forge/Quilt stay unavailable |
| Loader version/policy | Editable but automatic/pinned handling and platform lookup were incorrect | Correct Fabric/NeoForge label and lookup, automatic or exact pin, saved pin retained; old responses cannot replace another platform/version context |
| Aurora association (`auroraEnabled`) | Editable through reviewed transition, redundantly located in Settings | Installed Mods → Aurora installation association; existing preview/approval transaction and release requirements preserved |
| Java executable/component/major | Derived and security-owned | Read-only managed runtime explanation and existing Check/Install/repair actions; resolved game plan remains authority |
| Memory | Editable but 256-step input rejected valid persisted values; MB label incorrect | Whole MiB, 512–32768, step 1; native bounds and heap ownership unchanged |
| Additional JVM arguments | Editable and exposed | Persist raw text; Rust parses/validates quoting, lengths/count, forbidden heap/classpath flags at save and launch; no shell |
| Custom window size | Editable and exposed | Enable/disable custom size, width/height 100–7680; persisted optional value; Minecraft's normal window behavior otherwise |
| Fullscreen, monitor, refresh rate | Unsupported launcher preferences | Remain Minecraft-owned; no invented persisted fields |
| Instance/game/cache/runtime directories | Immutable managed identity and security-owned paths | Existing Open folder actions; no editable path or relocation control |
| Instance icon/color/background | No persisted per-instance capability | Deferred separate feature; launcher appearance remains launcher-wide |
| Mod enabled state and removal | Editable under content/dependency policy | Existing Installed Mods controls and native mutation checks, including original bootstrap mod behavior |
| Resource-pack enabled state | Editable and exposed | Existing Resource Packs controls and native activation persistence |
| Shader activation | Game-owned | Existing browse/install/remove; activation explicitly managed in-game |
| Provider update pin/channel | Editable and exposed | Existing provider lifecycle action controls and update previews; stable/beta/alpha policy and pin preserved |
| Installed provider identity, artifact hashes, dependency/pack ownership | Security-owned / immutable receipts | Inspect through existing inventory/details; no arbitrary provenance or ownership mutation |
| Exact installed Minecraft/platform/Aurora release pins | Derived from verified installation | Displayed installed state; updates/installations use existing reviewed workflows, never editable receipt text |
| Modpack project/version identity and overrides | Installation-owned | Existing exact pack creation/review; no unsupported in-place pack conversion or arbitrary ownership edits |
| Lifecycle state and readiness | Native-owned | Read-only status, validation/retry/install actions; saved settings alone never mark changed content ready |
| Selected instance | Launcher-wide editable reference | Existing Home selector and instance navigation; validated registry IDs only |
| Account/session/credentials | Separate account/security domain | Existing account controls; no password/token/configuration exposure or per-instance token copy |
| Other launch preferences | No additional persisted per-instance fields | No unsupported custom Java, executable, arbitrary launch command, environment or working-directory controls added |

Snapshot inclusion is a discovery filter, not an invented persisted instance preference. Launcher appearance, widget composition, privacy/Discord and desktop integration stay in their existing launcher-wide destinations.

## Rename and reliable saves

The previous Rename editor lived inside the Settings configuration form. Enter submitted configuration rather than rename; focus, Escape, byte-aware validation, operation-specific error/busy handling and a prominent workspace entry point were absent. It used character `maxlength` while native names have an 80 UTF-8-byte limit, and success depended on a later full-state refresh.

Both entry points now call `beginRename` and share one Aurora-styled native HTML modal. It prepopulates/selects/focuses the current name, handles Enter and Escape, restores trigger focus, exposes Save/Cancel, validates whitespace/control characters/80 UTF-8 bytes, and prevents duplicate submissions. Native failure keeps the draft and original persisted name with a visible error. Duplicate display names remain permitted; stable IDs disambiguate identity and repeated names in recent widgets.

Rename and configuration commands now require the original name/configuration alongside the proposed value. Rust compares that baseline under the existing registry mutex before atomic save. A stale draft is refused rather than silently overwriting a newer save. This is process-local serialization with optimistic value comparison, not cross-process locking or an ABA-proof revision system. Internal existing Rust callers retain their original wrappers; the frontend command path requires the baseline.

Successful operations merge the returned native record into shared state immediately. Header, Home selector, instance collection and history instance labels therefore use the same updated record. Readiness is refreshed after rename/configuration saves. Failed configuration saves retain the unsaved draft and expose native errors; Discard reloads the current record. No false success is inferred from local controls.

Rename changes metadata only. It neither moves nor recreates directories or changes game/user files. During launch preparation, changing the record invalidates the existing final snapshot and requires retry. During Starting/Running, cosmetic rename is allowed and leaves supervised child state unchanged. Desired launch settings are for the next launch; active process arguments are not rewritten. Installation/content changes retain their existing native locks and launch restrictions.

Minecraft/platform/loader edits remain **desired** configuration. Validation reports a mismatch until the user explicitly installs and verifies the new configuration. Aurora-compatible versions remain constrained by release authority. Saving cannot approve an unsupported combination or bypass final spawn validation.

## Settings cleanup and Aurora authority

The Aurora Configuration card and redundant Content navigation card are gone from Settings. General/Performance/Java/Display lead the page. Maintenance and confirmed Delete remain accessible beside them at wide sizes and below them at narrow sizes. The conditional Aurora update panel remains.

Installed Mods is the place to enable/disable/remove the current Aurora mod where the established bootstrap/provider policy permits. A collapsed Aurora installation association section exposes the existing reviewed transition when the original installation association itself must change. The transition component, preview, fingerprint, release metadata, content lock, artifact verification, rollback and readiness rules were retained. Changing association also advances the settings draft baseline to the returned native configuration.

Original installation intent and current mod-file presence remain distinguishable. Removed/disabled Aurora content is never silently reported as an active verified Aurora artifact. No update source, production registration, release endpoint, digest or security authority was invented or weakened.

## Desktop evidence and functional acceptance

Screenshots came from actual Tauri/WebView2 windows using the established isolated review shell, with native persistence enabled for history, favorites, rename, settings and older quick join. The explicitly labeled shell uses fixture accounts, runtime/readiness/content presentation and suppressed status enrichment. Its Ready badges are visual fixtures, not proof of an installed game; native state correctly reports the deliberately absent Aurora artifact as missing. The shipped production SPA was separately booted against an empty isolated application root.

Before captures load the original three relevant components from starting source without altering the checkout. Wide before/after viewport is **1600×1030 CSS pixels**, producing **2000×1288 PNGs** at Windows scaling. Narrow viewport is **900×930**, producing **1125×1163 PNGs**. Screens were inspected as rendered images, with native desktop inspection as well as WebView assertions. Background, glass, turquoise focus/accent, sidebar, card geometry and Home composition were retained. The long-server fixture truncates without horizontal overflow; the scroll boundary and keyboard focus remain visible. The three-row after capture limits the native DTO result read-only without changing stored history.

All paths below are relative to this report:

| Capture | Evidence |
| --- | --- |
| Before Home / before Settings | [Home](docs/evidence/l2/before-home-three-wide.png), [Settings](docs/evidence/l2/before-settings-wide.png) |
| Home, three servers | [Three rows](docs/evidence/l2/home-three-wide.png) |
| Home, twenty servers / scrolled | [Twenty rows](docs/evidence/l2/home-twenty-wide.png), [keyboard-scrolled](docs/evidence/l2/home-twenty-scrolled-wide.png) |
| Home favorites | [Favorite view](docs/evidence/l2/home-favorites-wide.png) |
| Instance Overview | [Overview](docs/evidence/l2/overview-wide.png) |
| Settings with card removed | [Settings](docs/evidence/l2/settings-wide.png) |
| Rename dialog | [Focused dialog](docs/evidence/l2/rename-dialog-wide.png) |
| Saved renamed instance / settings | [Saved native values](docs/evidence/l2/settings-renamed-wide.png) |
| Narrow Home / Settings / Maintenance | [Home](docs/evidence/l2/home-narrow.png), [Settings](docs/evidence/l2/settings-narrow.png), [lower Settings](docs/evidence/l2/settings-narrow-lower.png) |
| Production SPA boot | [Empty isolated production boot](docs/evidence/l2/production-boot.png) |

| Acceptance case | Result / evidence level |
| --- | --- |
| Empty / one / three / twenty servers | Native empty/one history tests; real production empty state; real desktop three/twenty screenshots and twenty-row assertions |
| Beyond retention, duplicate joins, new ordering | Native 280-join fixture; 100 nonfavorites plus preserved old favorite; canonical rejoin keeps one identity and moves it first |
| Legacy history migration | Exact schema-1 fixture preserves visits/instance partitions and original read-time bytes; native favorite mutation persists schema 2 |
| Restart and favorites | Full native process restart retains twenty servers and one favorite; native store-reopen test also retains favorite older than session retention |
| Unavailable / long text | Existing status failure/offline tests and new unavailable-row render assertion; real long-label desktop capture, zero horizontal overflow |
| Keyboard scrolling | Real WebView PageDown moves list scroll offset about 215px; list remains 246px high for roughly 1075px of twenty-row content |
| Older quick join | Rust resolves a favorite beyond the 256-session window; real desktop oldest row reaches native preparation and is safely refused with “The exact pinned launch metadata could not be resolved.” No Minecraft/server connection or successful gameplay join is claimed |
| Normal/Unicode rename, Cancel, Enter/Escape | Real desktop prefill/focus, Escape/Cancel and Enter Unicode save; shared record updates visible destinations |
| Blank/whitespace/max length/control/Unicode overflow | Desktop/renderer validation; native byte-bound test accepts 80 ASCII bytes and rejects 81 bytes, 27 three-byte characters and control characters |
| Duplicate names / stale rename | Native desktop permits two distinct IDs with the same display name, refuses stale expected name, restores fixture name |
| Failure / duplicate submission | Deterministic frontend failure preserves original record and draft/error; duplicate submit invokes once; native stale writes preserve registry bytes |
| Active/transitioning launch | Native test changes name after preparation and rejects final snapshot; test Starting/Running process states remain unchanged during cosmetic rename |
| Configuration persistence/readiness | Real native save of 4097 MiB, quoted JVM text and 1280×720 window; full restart retains values; frontend test verifies readiness refresh and original-baseline DTO |
| Version changes, locks, migrations | Existing native desired-vs-installed, platform, registry migration, transition, content lock and final-boundary regression tests all executed |
| Narrow layout | Actual native narrow window; Home scroll and Settings/Maintenance/Delete remain accessible; no document horizontal overflow |

Sanitized machine-readable summaries and image hashes are in [acceptance.json](docs/evidence/l2/acceptance.json). All names and history are disposable fixtures; no owner account IDs, server addresses, credential data or owner filesystem inventories are included.

## Automated verification

| Check | Final result |
| --- | --- |
| `npm run check` | PASS — 0 errors, 0 warnings |
| `npm test` | PASS — 257 tests, 0 failed/cancelled/skipped |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml` | PASS — 850 library tests, 6 icon integration tests; **29 library tests ignored, not passed**; binary/doc test targets contain 0 tests |
| Production frontend + native build | PASS — `npm run tauri build -- --no-bundle --config <external-isolation-config>`; static adapter/Vite and optimized native executable built |
| Real production application boot | PASS — final optimized binary copy in external acceptance storage, isolated identifier/root, empty instance/account/history UI |
| Real review desktop | PASS — native persistence, process restart, screenshot inspection and functional cases above |

The build uses an external application-identifier override and an empty diagnostic updater key to keep acceptance separate. Tracked production identifier/updater configuration and installed application are unchanged. Packaging, signing, updater publication and installer replacement are outside this request and were not run. Existing Rust warnings (four in test builds) and the existing static-runtime deprecation warning were not converted into unrelated cleanup.

Intermediate verification encountered review-server dependency-cache invalidation and a running debug executable file lock. Stopping the review server or running a private executable copy resolved those environment conditions; final full runs passed. Ignored network/diagnostic tests were not enabled or represented as passing. Native tests use disposable loopback fixtures and require no owner credentials.

## Preservation, commits and rollback

Preflight recorded branch/HEAD/history/remotes, complete tracked and untracked inventories, protected ignored developer files, critical file hashes and owner-managed data hashes. Baseline contained **698 tracked files, 15,144 pre-existing untracked files, 175 protected ignored developer files and 27,271 owner-data files**. Generated dependency/build trees were inventoried separately rather than treated as immutable source.

Repeated before-commit SHA-256 comparison found **no missing baseline files and no unexpected changes to pre-existing untracked, protected ignored or owner-data files**. All tracked differences are scoped L2 changes. L1 report/evidence, lockfiles, manifests, native build configuration, protected diagnostics and owner instances remained intact. No credentials or owner data were staged. Only explicitly named L2 paths were added; no broad staging/cleanup/deletion was used. Existing unrelated untracked files remain, so a globally empty status is not claimed.

Each commit was preceded by diff/name-status and whitespace/deletion checks, critical-file existence checks, full protected-data comparison and explicit staged-path/privacy review:

| Commit | Purpose |
| --- | --- |
| `cd82c0e6cc058c53edfa1b42822ca5e2805782bb` | Recent server summaries, retention, schema migration, favorites, lazy enrichment, scrolling and native tests |
| `257e5a34dc0eada01a6bfb5117cf5d40f7068eed` | Shared Rename, native stale-draft protection, settings control completeness and lifecycle tests |
| `e6293148ce82e9d86c52d2a4239f857ed4b3026e` | Settings card cleanup, Mods association access, transition baseline synchronization and frontend regression tests |
| `Record L2 configuration audit and desktop acceptance` | This report, architecture update, sanitized screenshots/receipts and native isolated-review routing |

Rename and configuration changes share the baseline-aware native DTO/store boundary, so they form one coherent editing commit. The Settings cleanup remains independently reviewable/revertible from that editing work. Nothing was pushed.

For source rollback, revert the evidence commit, then `e629314`, `257e5a3`, and `cd82c0e`, newest first, inspecting normal Git conflicts. **History downgrade needs a separate data decision:** the old binary deliberately cannot read schema 2. Preserve migrated history before reverting server support; only restore a known valid pre-migration schema-1 backup if intentionally accepting the loss of later history/favorites. Do not edit the schema number or overwrite damaged/owner files. L2 did not migrate owner production data during acceptance.

## Limits and L3 recommendation

Server status remains best-effort; real external servers, owner credentials and successful Minecraft multiplayer launch were not exercised. Deterministic native resolution/validation and safe desktop refusal cover the changed older-target route. Cosmetic launch-state acceptance uses native test process states, not an owner game. Cross-process writers, cloud history, unlimited favorites, exact lifetime server counters, historical data recovery, arbitrary paths/custom Java, per-instance icons and new display preferences remain separate features.

L3 should address the dedicated Skins & Capes page using the existing cosmetic/account authority and isolated acceptance workflow after its own authorization. Existing Home cosmetic widgets were not introduced by L2. No L3 work has begun.
