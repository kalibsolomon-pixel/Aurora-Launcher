# Aurora Launcher — L3: Cosmetics Library, Skins & Capes

## 1. Executive verdict

**COMPLETE.** The dedicated Skins & Capes destination, persistent organized local library, account-owned cape gallery, interactive player preview and compact Home summaries are implemented and committed locally. Desktop inspection verified the owner-reported overlap: the canvas now stays inside its stage, with controls and actions in separate normal-flow rows. The redundant upper-right account selector is removed, and the sidebar shirt has a wider body. The regular bottom-left account selector remains the single selector.

Local persistence and native operations were exercised against synthetic disposable data. Account-service behavior was verified with deterministic loopback responses and clearly labeled desktop fixtures. No live authenticated skin/cape mutation was attempted; this report does not claim live provider compatibility from mocks. Nothing was pushed, published or installed over the owner's Launcher.

## 2. Starting and ending source identities

| Item | Identity |
|---|---|
| Branch | `codex/client-3-production-authority` |
| Starting L2 HEAD, verified ancestor | `17db3dcd343083ed85832e2ece5af74d7446cdef` |
| Ending production implementation | `2854bd85d421117a541cad48fecb222b28642c36` |
| Ending source including review harness | `f02ec9ae93b242e3b1ecc2090dcfa7aa3a4d198b` |
| Ending source tree | `90c03b4099e3c09ff0bd4d67f311136200348aa4` |
| App version | `1.4.1`, unchanged |

The following documentation/evidence commit contains this report and architecture notes. Its identity is available from `git log -1`; embedding a commit's own hash in its contents is not possible. The production Tauri identifier, release configuration, package manifests and lockfiles are unchanged.

## 3. Existing cosmetic capability audit

The repository already had a real Rust cosmetic boundary. L3 extends it rather than introducing a second account API or library database.

| Capability | Existing support at L2 | Authority | L3 action |
|---|---|---|---|
| Saved library | Global JSON, generated UUID PNGs, 64 entries, SHA-256 metadata | Rust `cosmetics` | Extend to 256; schema-2 favorites; preserve known old records |
| Import | Bounded validated Java skin PNG bytes, explicit model, duplicate detection | Rust | Keep native validation; tighten duplicate revalidation, managed reads and legacy model rules; add full-page dialog |
| Preview | Native decoded head thumbnails; software full-body account player | Rust decoding, frontend rasterization | Add verified full saved-skin pixels; reuse renderer for cards and interactive preview |
| Active skin | Matching account profile model/texture and account avatar pipeline | Rust auth/profile | Return fresh current full texture in cosmetic state; distinguish cache/unavailable texture |
| Apply skin | Selected entitled session, multipart Minecraft Services request, profile refresh | Rust credentials/session/cosmetics | Expose explicit page action; require matching upload and fresh profile skin identity/model |
| Cape inventory | Profile-owned capes with native bounded artwork decoding | Rust | Visual owned gallery, active/available states, No Cape and account-empty states |
| Cape selection | Fresh ownership check, PUT/DELETE and profile confirmation | Rust | Reuse existing commands; preview stays separate from Equip |
| Cosmetic caching | Native avatar cache and managed preset files; shared frontend state | Rust storage plus account-keyed presentation | Reuse; bounded 64-entry full-preview promise cache keyed by id/hash/model |
| Account switching | Validated Minecraft UUIDs, independent shell account manager | Rust account authority | Preserve; generation guards for pending reads and mutations |
| Home widgets | Registered `skin-manager` / `cape-selector`, persisted order/size/visibility | Existing native Home layout/store | Compact shared-state summaries and links to full library; preserve configuration |

The audit also found that merely confirming a skin's model after upload could accept an unchanged same-model skin. The separate confirmation commit now checks active skin identity too. Existing endpoint behavior is retained; this is not a claim of a newly documented public Mojang API contract.

## 4. Sidebar and page architecture

`navigation.ts`, `navigation.svelte.ts` and the application page switch add the global `cosmetics` destination between Instances and Settings. It does not select an instance or change Play authority. The existing icon registry supplies the shirt; its torso extends from x=6 to x=18 in the 24-unit drawing.

`CosmeticsPage.svelte` presents a segmented Skin Library/Capes control, shared toolbar, wider library/gallery column and narrower contextual preview column. Below 900 CSS pixels the columns stack. The skin grid is a named, keyboard-focusable vertical scroll region. Page-level vertical scrolling keeps actions reachable on short windows. Native dialogs provide import, rename and confirmed local removal, including Escape and return focus.

The selected tab, query, favorites filter, sort and local selection remain in the shared session store while navigating. Local metadata/favorites persist natively; these temporary browsing controls are not new persisted preferences. Account context is expressed in current-skin, ownership and action labels. The shell account dialog remains accessible on the page without a duplicate page selector.

## 5. Skin library storage and migration

The library remains global to the Launcher and shared across accounts. It is independent of instances and is not removed when an account disconnects. Storage remains beneath the platform application-data root at `launcher/skin-presets/presets.json`, with validated opaque UUID PNG filenames. Display names never become paths.

Schema 2 adds `favorite: boolean` and permits up to 256 records. Only the exact known schema-1 shape migrates; reads supply false favorites in memory and leave original bytes intact. The next explicit successful mutation writes schema 2. Malformed records, duplicate IDs, missing required schema-2 favorites, unknown fields or unsupported versions fail without overwriting the document.

Metadata writes use a synchronized temporary sibling and rename. Imported validated bytes are committed before metadata references them; failed metadata writes remove only that operation's newly created asset. A crash may leave an unreferenced asset; L3 does not scan or remove pre-existing assets. Renames/model choices/favorites update metadata. Reads are bounded and reject symlink/reparse redirection inside the managed ancestry.

Identical bytes with the same model reuse the existing entry after revalidating its asset. A damaged duplicate is an error, never a successful import. Identical bytes under another model keep distinct metadata but share data through a hard link. Such sharing requires filesystem hard-link support; failure reports a storage error instead of silently weakening the storage behavior. Every native acquisition rechecks the recorded SHA-256.

## 6. Import and validation

The operating system file chooser is reached through the existing Tauri WebView file input. Only bounded byte arrays, a name and the model cross the typed command boundary; user-selected filesystem paths do not. Extensions and MIME labels grant no authority.

The native pipeline accepts static PNGs up to 128 KiB, with a valid signature, bounded/chunk-checked image data, complete decode under a 1 MiB decoder budget, dimensions 64×64 or 64×32, and decoded RGB/RGBA output. Malformed, truncated, animated, unsupported-dimension and oversized images fail with structured errors. Names are trimmed, nonempty, control-character-free and at most 80 UTF-8 bytes.

Model choice is explicit. Classic uses four-pixel arms and slim uses three-pixel arms. Legacy 64×32 imports and updates require classic; existing legacy pixels normalize to classic geometry for preview. L3 does not guess model type from appearance. Duplicate names are allowed because record identity is opaque.

Local deletion requires a dialog confirmation and affects only the saved record/managed asset. It never calls a remote skin mutation. Canceling deletion was exercised on desktop. Invalid and oversized desktop imports left the 12-entry disposable library intact.

## 7. Skin and cape preview implementation

The existing software cuboid renderer in `playerModel.ts` supplies correct classic/slim arm widths, legacy mirrored limbs, six-face texture mapping, outer layers, alpha blending and shaded depth. L3 extends it with a 10×16×1 cape cuboid, using outward UV (12,1) and inward UV (1,1), scaled for supported cape texture widths. This was cross-checked against the upstream [skinview3d model mapping](https://github.com/bs-community/skinview3d/blob/master/src/model.ts); no engine or runtime dependency was added.

`CosmeticPreview.svelte` adds pointer capture/drag rotation, arrow-key rotation, Home/reset, and explicit Front/Back buttons. Its slider exposes orientation and a visible focus ring. The canvas is positioned inside a bounded relative stage, preventing its intrinsic raster size from increasing the grid row into controls or actions. The owner-reported overlap was reproduced and corrected here.

Rendering occurs on model/orientation/visibility changes through a cancellable animation frame. Intersection and document-visibility observers gate drawing; destruction disconnects them. There is no idle animation or GPU allocation to release, and reduced-motion users are not given automatic motion. Small card swatches are visibility-gated static renders; native texture acquisition is cached with bounded entries and failure eviction. Invalid/missing textures show a readable fallback and retry rather than a broken canvas. The passive Home player retains its established arrow/reset interaction policy.

## 8. Official skin application behavior

Apply is explicit and names the selected account. Importing, previewing, renaming, changing local model, switching accounts and startup never upload a skin. Rust checks selected-account identity and obtains the existing usable entitled Minecraft session; credentials and downstream tokens remain native and redacted. Saved assets are revalidated before upload.

The existing Minecraft Services multipart operation sends the chosen official variant and validated PNG. A response is not sufficient evidence of success: native code extracts the upload response's active skin identity/model and performs an independent profile GET for the same account. Only matching identity and model confirm the operation. Empty upload bodies, old same-model identities, wrong-account profiles and failed refreshes return honest errors. Successful DTOs contain normalized profile data and decoded pixels, never token strings or request headers.

The frontend updates only the matching account's shared avatar state and current profile. Failed requests retain saved skins and mark the last fetched remote state stale until explicit refresh. A service may have accepted a mutation whose later confirmation failed; the UI reports that uncertainty instead of claiming rollback or confirmed success. [Minecraft's skin troubleshooting guidance](https://www.minecraft.net/en-us/article/minecraft-java-edition-skins-issue-update) was consulted as context, not as a complete API specification.

## 9. Cape inventory and selection

The gallery comes from the selected account's normalized Minecraft profile and shows owned names/art, Active or Owned · Available, and No Cape. Missing artwork uses a fallback without fabricating ownership. The backend limits optional cape texture fetches and decodes their pixels natively; frontend URLs do not select providers or local paths.

Choosing a card changes only preview state. Equip and Use No Cape are separate explicit actions. Native selection re-fetches the owned inventory before mutation, rejects unowned/stale choices and confirms the resulting account profile. The existing PUT/DELETE endpoints and credential handling are reused. An already selected cape avoids a needless mutation. Failed, offline and authentication-required states retain local skins and pause account changes.

The preview heading states whether a choice is active or preview-only. If a saved local skin is used to inspect a cape, an additional note identifies that local skin rather than representing it as the account's equipped skin. No arbitrary cape import, unlock or assignment is implemented.

## 10. Local versus official distinctions

Saved local skins, the selected account's current skin and official owned capes are distinct concepts. A local selected skin is labeled Local preview · not applied. Current marking requires matching SHA-256 and model against the freshly fetched current texture; a cached avatar is not proof that a saved entry is current. Save current skin uses the existing native acquisition command and requires fresh usable account state.

Cape inventory comes only from native official profile authority in production. Fixture ownership exists only inside the separate review entry. Local library deletion never removes an official skin, and switching a preview never equips a cape. Remote errors do not discard local records.

## 11. Account switching, stale data and offline behavior

`CosmeticsStore` separates local loading/mutation errors from account requests, coalesces library reads and waits for an initial read before publishing an edit. Request generations reject late profiles, wrong-account replies and an old mutation after A→B→A. A further guard covers switching during the fallback avatar refresh. Multiple remote actions are excluded while one is pending.

Switching accounts immediately clears the previous remote profile and cape choice. The global local library stays available. A cached avatar can still be displayed as cached, never relabeled freshly confirmed. Session-expired/reauthentication-required or failed remote state disables mutation. The normal shell selector was used on desktop in both directions; SecondPlayer had zero capes and never displayed AuroraPlayer's inventory.

No account, one account, multiple accounts, no capes, several capes, offline retrieval, expired session, failed mutation and successful mock profile refresh were exercised. Native auth restoration/revocation/session behavior also remains covered by the existing Rust suite. Remote requests are bounded, redirect-free and explicitly retried; L3 adds no polling or telemetry. Fresh means refreshed this session, not an assertion of indefinitely current server state.

## 12. Home widget refinement

Skin Library now summarizes account appearance plus two/three/four favorite-first recent entries at small/wide/large sizes. It truncates long labels, shows models and opens the full library or a selected saved skin. Cape Selector shows the current choice or No Cape, compact art, account, owned count and a link to the cape tab. Cached or unavailable state stays labeled.

Both widgets reuse the same native DTOs/store as the full page. Complex import, management and remote mutation flows live on the full page. Existing widget IDs, supported sizes, order, visibility toggles and layout schema are unchanged. An isolated real native save/restart verified persisted size/order/visibility alongside unchanged saved skins. Existing owner widget preferences were not touched.

## 13. Desktop screenshot paths and inspection

All PNGs below are actual Tauri WebView2 desktop captures. Accounts/capes are synthetic unless the row explicitly states no account with the actual backend. Synthetic artwork was generated in the review fixture; no private collection or account identifier is included. Wide before/after captures use matching 2000×1288 pixel WebView viewports (1600×1030 CSS pixels at host scaling). The native narrow window was physically resized to 852 CSS pixels wide; it was not a browser-only viewport emulation.

| Required view | Evidence under `docs/evidence/l3/` |
|---|---|
| 1–2. Both original Home widgets | [01-02-home-widgets-before.png](docs/evidence/l3/01-02-home-widgets-before.png) |
| 3. Populated native local library | [03-skin-library.png](docs/evidence/l3/03-skin-library.png) |
| 4. Owned cape inventory fixture | [04-cape-inventory.png](docs/evidence/l3/04-cape-inventory.png) |
| 5. Empty/no-account library | [05-empty-library.png](docs/evidence/l3/05-empty-library.png) |
| 6. Account with no capes | [06-no-capes.png](docs/evidence/l3/06-no-capes.png) |
| 7. Front inspection | [07-player-front.png](docs/evidence/l3/07-player-front.png) |
| 8. Back inspection | [08-player-back.png](docs/evidence/l3/08-player-back.png) |
| 9. Synthetic PNG import dialog | [09-import-dialog.png](docs/evidence/l3/09-import-dialog.png) |
| 10. Saved skin rename/action state | [10-skin-detail-rename.png](docs/evidence/l3/10-skin-detail-rename.png) |
| 11. Preview cape before explicit Equip | [11-cape-selection.png](docs/evidence/l3/11-cape-selection.png) |
| 12. Compact Home widgets | [12-home-widgets.png](docs/evidence/l3/12-home-widgets.png) |
| 13. Physical narrow window | [13-narrow-layout.png](docs/evidence/l3/13-narrow-layout.png), [13b-narrow-preview-actions.png](docs/evidence/l3/13b-narrow-preview-actions.png) |
| 14. Destination and single account selector | [14-sidebar-destination.png](docs/evidence/l3/14-sidebar-destination.png) |
| Extra: offline browsing | [15-offline-library.png](docs/evidence/l3/15-offline-library.png) |
| Extra: rejected cape mutation | [16-cape-failure.png](docs/evidence/l3/16-cape-failure.png) |
| Extra: regular selector switches account | [17-switched-account.png](docs/evidence/l3/17-switched-account.png) |
| Extra: optimized bundled actual-backend boot | [18-optimized-native-smoke.png](docs/evidence/l3/18-optimized-native-smoke.png) |

Screenshots were opened and visually inspected, including matching widget comparison and front/back/cape/dialog states. Labels, proportions, cape direction, focus state, spacing and active states are legible. The preview no longer covers its text or buttons. Content scrolls vertically where the collection or page exceeds the viewport; this is intentional, with no horizontal overflow or inaccessible controls. Home comparison captures intentionally scroll to the widget row, so the upper Home hero is partly outside the viewport in both.

## 14. Automated and functional acceptance

| Check | Result |
|---|---|
| `npm run check` | Pass, zero Svelte errors/warnings |
| `npm test` | 265 passed, zero failed/skipped |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Pass |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | Pass |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 854 unit + 6 icon integration passed; 29 intentionally ignored helpers/live diagnostics |
| `npm run build` | Pass, production static SPA |
| Native desktop optimized `tauri build --no-bundle` | Pass with external isolated config; bundled production assets booted |
| `tools/visual/l3.test.mjs` | Pass at 1600×1030, 1120×760, 860×800, 720×520; containment, single selector, keyboard, dialog focus, account isolation and widget navigation |
| Existing `tools/visual/l1.test.mjs` | All four acceptance groups passed |
| Existing `tools/visual/correction.test.mjs` | All five acceptance groups passed |
| Actual desktop native library operations | Import, duplicate, rename, favorites, search, canceled/confirmed delete and invalid/oversized rejection passed |
| Actual process restart | 12 records, three favorites, renamed entry and all full-preview hashes retained; metadata bytes unchanged on read |
| Actual native widget layout restart | Size, order and visibility retained |
| Actual optimized bundled app | No fixture bridge, zero accounts, 12 synthetic local skins, native offline preview/rotation, disabled Apply, contained layout |

New deterministic coverage includes migration without read-time writes, old-capacity growth, changed/missing assets, duplicate sharing and model separation, bounded full preview/alpha, malformed/oversized PNGs, multipart upload, matching fresh identity confirmation, wrong/stale ownership, failed provider requests, account-switch races, search/order/current association, bounded Home summaries and cape UV/front/back geometry. Existing authentication and secret-redaction tests pass. Focused tests were rerun after final store/widget edits; final layout checks were rerun after visual changes.

The Rust check retains four warnings in untouched pre-existing test helpers; the native build retains the existing static-VC-runtime deprecation warning. No dependency upgrade or unrelated warning cleanup was bundled. Initial harness timeouts from stale review modules/obsolete artwork fixture handling and a duplicate diagnostic app process were corrected by a cold isolated restart and the current normalized artwork fixture. Final cold desktop acceptance and regressions pass. Test selectors were corrected when they used obsolete accessible names; those timeouts were harness failures, not successful acceptance runs.

Sanitized machine-readable results are beside the screenshots: `desktop-acceptance.json`, `restart-acceptance.json`, `additional-desktop-acceptance.json`, `widget-restart-acceptance.json`, `expired-session-acceptance.json`, `optimized-native-smoke.json` and `verification-summary.json`. Full diagnostic logs/inventories remain outside the repository because they contain local paths.

## 15. Live versus fixture evidence

| Evidence | Actual behavior exercised | Boundary |
|---|---|---|
| Native Rust tests | Real filesystem/validation/metadata and loopback HTTP request/response behavior | Deterministic synthetic data and provider responses |
| Desktop `native-library` | Actual Rust library reads/writes, hashes, previews, file-input import flow and restart persistence | Input files assigned through desktop automation; all accounts, profiles, owned capes and remote mutations are review fixtures |
| Regular desktop selector | Actual shared shell account-dialog interaction and page invalidation | Synthetic account roster/selection response |
| Optimized actual-backend desktop | Bundled production SPA plus real native commands and isolated application storage | No account or instance; synthetic local PNGs only; no review invoke alias |
| Live Microsoft/Minecraft providers | No live authenticated mutation performed | Live skin/cape compatibility remains unexercised |

The production build was smoke-tested with an external distinct review identifier. Release builds ignore the diagnostic managed-root environment variable, so their native library was seeded only in that separate identifier's disposable platform application-data folder. The checked-in production identifier and installed application were untouched. No owner instance, credential, account selection or cosmetic collection was used.

## 16. Preservation verification

Preflight recorded status, branch, full HEAD/history/remotes, tracked files, protected untracked/ignored files and hashes of owner application data. L2 ancestry was confirmed. The original inventory contained 716 tracked files, 15,144 pre-existing untracked files, 175 protected ignored files and 27,271 owner files. Generated build/dependency directories were excluded from the protected ignored set.

Before each local commit, the inventory was rehashed, critical-file presence checked, name-status/deletion diffs inspected and staged paths reviewed. Every original file still exists. Protected untracked/ignored and owner data remained byte-identical; tracked changes are confined to the scoped implementation, tests and documentation. L1/L2 documentation/evidence, diagnostic/performance work, release/update configuration, account storage, managed instances, installed Launcher files, manifests and lockfiles remain intact. No file deletion or broad cleanup was performed.

Only synthetic disposable local library/layout and isolated WebView data were written during desktop acceptance. Full preservation records remain external; committed [verification-summary.json](docs/evidence/l3/verification-summary.json) contains counts and results without personal paths or identifiers.

## 17. Local commits and rollback

| Commit | Scope |
|---|---|
| `9380cfc` | Native library favorites, validation and full previews |
| `b64e2f2` | Strong upload/fresh-profile confirmation and regression |
| `854bbe4` | Sidebar, page, shared state and integrated interactive renderer |
| `2854bd8` | Compact Home summaries without changing persisted widget configuration |
| `f02ec9a` | Isolated fixtures, reusable layout acceptance and harness instructions |
| Following report/evidence commit | This report, architecture notes and sanitized desktop evidence |

The suggested page and renderer commits were combined because the full-preview component, texture cache and cape presentation are one integrated page boundary. Native confirmation remains separately revertible. No commit contains dependencies, owner data or unrelated cleanup. No push occurred.

To revert the whole phase, revert these commits in reverse order, starting with the report/evidence commit shown by `git log -1`. Use focused reverts rather than resetting or cleaning the workspace; existing user files must remain intact. Independent commits permit review and selective rollback, but the page requires the new native DTO/commands, so reverting its native boundary alone leaves frontend references unresolved.

**Data rollback limitation:** once an explicit edit has persisted schema 2, the L2 executable deliberately refuses that document. Reverting code does not downgrade metadata or erase the collection. Before testing L3 against a personal collection, retain a separate byte-for-byte backup of its schema-1 metadata and PNGs. Restore such a backup explicitly if returning to L2; it cannot include later L3 edits. No speculative downgrade or automatic destructive migration is supplied. Mere reads of a schema-1 library leave its bytes unchanged. This session did not migrate the owner's library.

## 18. Remaining limitations

Live authenticated provider compatibility requires a separately authorized real account test; fixtures cannot establish it. The existing official-service operations are retained, with stricter confirmation that may conservatively report uncertainty if a service returns an unexpected success-body shape or delayed profile state. Use explicit refresh to resolve such a state.

The local library is bounded at 256; there are no folders/tags, arbitrary cape imports, animated cosmetics, grants/unlocks, auto-upload, background polling or instance-specific libraries. Full-preview texture caching assumes managed assets do not change externally during a presentation session; native reacquisition/upload always revalidates, and failed entries can be retried. Cross-process/external-writer races are outside the existing process-local storage exclusion. Hard-link sharing requires support from the managed filesystem. No automatic orphan collection or generalized repair was added.

The software renderer deliberately favors the existing lightweight implementation over a new engine. It provides change-driven rotation and a modest fixed pose, not physics or idle animation. Current official skin/cape art is unavailable if native texture acquisition fails even when profile metadata was fetched; that absence is shown honestly. Future work should follow an explicitly scoped phase and preserve the same storage/account boundaries. L3 stops here after local commits and this report.
