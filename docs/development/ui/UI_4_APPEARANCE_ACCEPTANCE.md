# UI-4 — Player Appearance acceptance

**PARTIAL — implementation and automated acceptance pass; exact native medium/minimum sizing remains unverified.** The Appearance workspace, local library and interactive full-body renderer are ready for owner visual review. Actual Windows import, rendering, rotation/tilt, zoom, reset, geometry/layers, restart persistence, confirmed removal, accent/background, navigation and clean exit passed. A full native acceptance PASS is not claimed.

## Baseline and local commits

- Branch: `codex/client-3-production-authority`, unchanged.
- Starting HEAD: `f0319f105e188eb0d891bc79663d78f6ee6644e7`, exactly the expected UI-3.1 commit.
- Ending application HEAD: `3a9a0be`; ending verification/code HEAD: `3774468`.
- Delivery HEAD is the local evidence/report commit containing this document; resolve its exact SHA using `git log -1 --format=%H -- docs/development/ui/UI_4_APPEARANCE_ACCEPTANCE.md`. Its SHA is also returned with delivery; a commit cannot embed its own hash.
- Initially 1,819 tracked files, no tracked changes, seven worktrees and the four protected untracked artifacts. Full hashes, Git/history/worktree/process inventories and previous acceptance paths: [baseline](../../ui-4/baseline.json).

| Local commit | Scope |
| --- | --- |
| `8948657` | Pre-implementation capability audit and preservation baseline |
| `f49bc24` | Optional renderer layers/camera and interactive view |
| `b15dfbf` | Appearance composition, identity/status and named-account confirmations |
| `3a9a0be` | Accessible selector, stage sizing and dependency-free camera defaults |
| `3774468` | Workflow/regression/native fixture tools and architecture documentation |
| This delivery commit | Acceptance, screenshots, measured results and logs |

Complete changed paths and history are under `docs/ui-4/changed-files.txt` and
`commits.txt`. Existing application edits are confined to `navigation.ts`,
`CosmeticsPage.svelte`, `CosmeticPreview.svelte` and `playerModel.ts`; `playerView.ts`
is new. Tests add `playerView.test.ts` and UI-4 harnesses; L3 browser assertions
now follow the Appearance label, side-panel action placement and cape confirmation.
No Rust source, native DTO, package/dependency/lockfile, identifier, version,
minimum-window setting or release file changed.

## Audited capabilities and decision gate

[APPEARANCE_AUDIT.md](../../ui-4/APPEARANCE_AUDIT.md) records existing acquisition,
geometry, caches, account/session ownership, storage, widgets and operation evidence.
The repository already had a robust Rust local library, authenticated skin upload,
owned cape selection/disable and cape geometry. UI-4 reuses those capabilities;
it does not introduce a second renderer, filesystem store or authentication path.

Account operations are the existing E3/L3 contracts, supported by deterministic
native service tests and independent authoritative profile confirmation. The
historical research calls them ecosystem-corroborated contracts, not a published
official service specification or a live-production mutation proof. Skin upload
matches active skin identity/model against a separate matching-account GET;
cape selection rechecks ownership and confirms the selected ID after GET; No Cape
confirms absence of an active cape. No new authenticated capability or scope was
needed, so the gate for proposing a new backend operation was not entered.

No live skin/cape mutation, sign-in, credential copy or extra permission request
occurred. Custom cape upload, third-party capes and additional account operations
remain unsupported. Official cape artwork/ownership states in browser captures
are synthetic fixtures; native capes truthfully show the no-account state.

## Implemented behavior

The existing destination is labeled **Appearance** and keeps widget links and the
sole bottom-left account manager. A large primary stage precedes a compact side
panel with saved-model metadata, account actions, import/library controls and
skin/cape inventory. Library thumbnails are smaller and consistent; Preview and
Current text markers supplement selected color and pressed semantics. Skin and
cape tabs retain keyboard navigation, search/favorites/sorting and existing storage.

The software cuboid renderer supplies classic/slim arms, legacy mirrored limbs,
expanded transparent layers and cape geometry. Optional parameters preserve the
Home defaults. Appearance adds horizontal yaw and vertical tilt drag, arrow-key
inspection, +/− zoom, Home reset, front/back/reset buttons, local geometry, outer
layers and cape visibility. Tilt is −0.35–0.4 radians; zoom is 0.75–1.15. Tests
measure occupied pixel bounds at extreme combinations; full body stays inside the
raster. The existing high-resolution canvas with nearest-texel texture sampling
preserves pixel artwork without a new engine, smoothing pass or post-processing.
There is no automatic rotation, idle loop, telemetry or new preference.

Profile identity, current decoded skin, cached fallback, local preset, cape choice,
geometry/layers/camera and pending local/remote operations stay separate. Missing
account, missing/failed texture, offline and expired-authentication states remain
usable for local previews. A refreshed profile with unavailable pixels labels an
older account texture as cached. Unknown model information is not fabricated.
Generation guards reject previous-account responses, including A→B→A mutations.
Switching accounts clears cape selection and closes account confirmations while
retaining the explicitly labeled local skin preview. Camera/model reset on source
identity changes; startup returns to account appearance without applying anything.

Apply and cape changes require a confirmation naming the target account. Local
model preview, layers, camera, cape visibility and thumbnail selection issue no
account command. Progress/failure stay in the shared cosmetic store; success is
shown only after native profile confirmation. Failure retains the local library
and marks account state stale. Home never receives an unapplied local preview.

## Import and storage verification

The unchanged native library accepts only full CRC-checked, decoded static PNGs,
64×64 or legacy 64×32, RGB/RGBA output, at most 128 KiB encoded and a 1 MiB decoder
budget. Legacy slim is rejected. Original image bytes are preserved; user filenames
and display names never become paths. Stable generated UUIDs identify entries;
same-name imports do not overwrite others and hash/model duplicates reuse records.
Schema-2 metadata is written atomically; exact schema-1 migration remains defined.
Malformed/future state, modified bytes and redirected managed paths fail closed.
Native reads rehash assets before preview/upload/removal. No game directories are used.

Native UI import saved `Aurora-local-classic.png` from this phase's synthetic fixture.
After clean exit and restart, the real library still had the same entry and rendered
the same full-body pixels. A local Slim preview left its saved model Classic.
Confirmed removal deleted exactly UUID `9987367c-1826-4d4b-85a7-d59d6ea3e310`'s PNG
and record. [Storage verification](../../ui-4/native-restart/storage-verification.json)
proves zero remaining records, absent exact PNG, unchanged source fixture SHA-256,
unchanged unrelated sentinel, null instance selection and no account/instance documents.
No remote account skin was available or altered.

## Automated and browser results

All final checks below exited 0; exact commands/logs/results are in
[checks/exits.json](../../ui-4/checks/exits.json) and adjacent logs.

| Check | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings; version contract 1.4.2 |
| Full sequential frontend unit suite | 275 tests / 16 suites, all pass, no skips/cancellations |
| Added camera/geometry tests | 3; finite bounds, legacy/modern layer separation, default-pixel equivalence and full-body extreme views |
| Rust cosmetics tests | 18 pass; PNG dimensions/decoding/bounds, storage/hash/damage/schema, multipart/profile/ownership confirmation and sanitized failures |
| UI-4 browser acceptance | 13 groups; actual components, 10 size/text combinations, zero page errors |
| UI-1 regression | 11 cases pass |
| UI-2 regression | 17 cases pass |
| UI-2B regression | 6 groups pass |
| UI-2B.1 crop regression | 6 responsive cases pass |
| UI-3 regression | 11 groups pass |
| UI-3.1 regression | 13 groups / 10 size-text combinations pass |
| Existing L3 cosmetics regression | 4 layouts plus cape confirmation, account/widget navigation pass |
| Tauri debug no-bundle build | Static production frontend and debug executable built; no installer |
| Preservation and Git deletion/whitespace checks | Pass |

UI-4 browser tests cover named current identity, skin acquisition/display, missing
skin/cache fallback, account switch/stale result, preview versus active, no-account/
offline/expired, model/layers, capes/visibility/missing cape, pointer/keyboard camera,
zoom limits/reset, confirmation/success/failure, local rename/removal, navigation,
dialog focus return and containment. Existing frontend/native tests cover local
import, invalid bytes/dimensions/size, persistence, matching-account profile and
A→B→A guards. Routine tests require no live Microsoft authentication.

Measured sizes: 1440×900, 1280×800, 1024×768, 720×520 and the previously measured
706×512 native-client size, each at 100% and actual doubled text. All have zero
horizontal overflow. At narrow widths the stage stacks before controls/library
and uses ordinary vertical scrolling; at 200% text scrolled stage/library captures
prove access. Dialogs fit/scroll inside the viewport and Escape returns focus.
Arrows, zoom buttons, reset and geometry/layer controls are keyboard accessible.
Reduced motion/transparency and alternate accent cases pass. Full screen-reader
certification and performance benchmarking are not claimed.

The renderer coalesces pointer redraws per animation frame, renders only while
visible/intersecting and disposes its frame/listener/observer on navigation. Existing
native/frontend bounded texture caches and coalesced library/profile requests are
retained. No new network loop, cache, asset downloader or GPU allocation is added.

## Native acceptance and explicit limitations

The real compiled frontend ran under a fresh marked direct-temp data root and
separate WebView profile. Each CDP capture verified the Rust-reported managed root;
CDP was used for read-only screenshots/measurement, Computer Use for interaction.
[Review guide](../../ui-4/REVIEW_GUIDE.md) distinguishes native and browser evidence.

- Default measured CSS client: **1120×790**, DPR/display scale **1.25 / 125%**.
- Maximized measured CSS client: **2048×1152**, same scale; full character and controls visible.
- Native import, drag yaw/tilt, zoom, reset, Classic→Slim local geometry, layers,
  library visibility, no-account/cape state, Borealis/cyan, navigation away/back,
  persisted library and confirmed safe removal were witnessed.
- Native process 4360 exited 0; final-source restart 26852 exited 0, with retained
  handles and actual exit records. Earlier isolated pilots also exited 0.
- **Unverified:** exact native 1024×768 and actual enforced-minimum sizing. Boundary
  drag and system Size-menu/keyboard attempts retained 1120×790. Their JSON/PNGs
  are labeled attempts, never passing minimum/medium evidence. Browser coverage
  does not close this native requirement. Native configuration/minimum is unchanged.
- Real authenticated native skin/cape appearance and live service mutations were
  deliberately not exercised: no safe isolated signed-in account was available,
  and sign-in/production credential copying were not authorized. Browser/native
  service fixtures cover those cases without claiming a real account mutation.

The first boot attempted a development URL because a subsequent Cargo test build
had replaced the debug executable's build context. It was closed normally and the
Tauri executable rebuilt after native tests; final runs use compiled local assets.
A rebuild attempted while the isolated executable was still open failed with a
file-lock error; closing that exact process and rebuilding succeeded. Initial
browser tests exposed a stale Vite optimization cache, an incorrectly compounded
text-scaling harness, current-skin fixture selection and a missing explicit select
name. These were corrected; final passes are recorded. Existing STATIC_VCRUNTIME
and Rust test-helper warnings remain; no unrelated native cleanup was attempted.
No full Rust format/check/test pass is claimed because native source is unchanged.

## Home, visual review and preservation

All four matched Home screenshot PNGs at 1440×900, 1280×800, 1024×768 and 720×520
are **byte-identical** to UI-3.1 evidence: [hash comparison](../../ui-4/home-comparison.json).
Home component, scale, angle, crop, launch/instance controls, widgets and account
manager retain their baseline hashes. All original native/config/release/lockfiles
remain byte-identical. UI-3 Mods/Modpacks regressions and the owner accent correction
remain intact.

Actual native screenshots plus browser geometry were inspected against approved
Home/Mods materials. The character is prominent, full-body, and separate from the
compact collection; active/current/preview have textual status, capes retain portrait
artwork, and Borealis/accents remain shared. No Note screenshot/artwork was supplied;
only the request's written design principles were used. No proprietary art was copied.

[Preservation](../../ui-4/preservation.json) verifies all 1,819 original tracked paths,
the 1,813 outside the six scoped original source/document/test paths, four protected
file hashes, seven worktrees/other HEADs and zero deletions/renames. Two old regression
helpers wrote a capture/result to historical evidence paths; the preservation check
caught this immediately, new outputs were retained under UI-4, and exact baseline
bytes restored with hash proof. Final preservation passes; previous evidence is intact.

Owner production Launcher 1.4.2, accounts/credentials, instances/worlds/saves/mods/
resource packs/configuration, existing local appearance assets and release assets
were not operated on. Only marked isolated roots received acceptance state; they
are retained. Build/game/runtime data and protected local files are not staged.
No clean/reset/prune, process-name termination, push/merge/tag/publish/deployment,
version bump, release preparation or UI-5 work occurred.

## Recommended next step

Review the supplied native/browser screenshots, then complete an isolated native
1024×768 and enforced-minimum resize pass. That is the outstanding mandatory
acceptance gate. No new account backend capability needs approval for UI-4; live
skin/cape mutation remains optional and unperformed. Stop after UI-4.
