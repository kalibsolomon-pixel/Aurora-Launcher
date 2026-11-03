# Aurora Launcher Native 2.0.1 release record

**RELEASED — AURORA LAUNCHER NATIVE 2.0.1 VERIFIED**

Both corrections are in the exact tagged source. The protected build, signing,
immutable publication and native authority advancement completed. Independently
downloaded public bytes match the accepted signed CI artifact and discovery offer.

## Source and release identity

- Starting branch: `codex/native-2.0.0-release`, clean index/worktree and no untracked files.
- Starting full SHA: `5deab8326c870175be91afc40d528088b4096924`.
- Starting remote main: `1fcc562b0e176e946c3889a35cfa7877deefd27c`.
- Final integrated release-source SHA: `82620e57b5c8b933dc970fae02200635df8cfa7c`.
- Integrity correction: `5deab8326c870175be91afc40d528088b4096924` (already present; not cherry-picked again).
- Notification correction: `62fc4e0e96633b74af96ad4767a913dd877a9980`.
- Version, title and notes: `e9da01dab6384270f39b552e8aac767abd1c9fb5`.
- Integration: [PR #2](https://github.com/kalibsolomon-pixel/Aurora-Launcher/pull/2).
- Version/tag: `2.0.1` / `native-v2.0.1`, verified at the exact integrated source above.
- Release title: Aurora Launcher Native 2.0.1 — Modpack Reliability and Download UI Fixes.
- Public release: [immutable GitHub release 432112669](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/native-v2.0.1), published 2026-11-03T00:22:25Z, stable and Latest.
- Identity retained: `com.aurora.launcher.native-preview`, channel `native-preview`, Slint, Windows x86_64, `nsis-per-user`.
- Native/core Rust metadata, both Cargo lockfiles, npm metadata/lockfile and workflow default agree on 2.0.1. PE resources, application display and NSIS derive this version from native Cargo metadata.

## Artifacts and protected workflow

- Installer: [Aurora.Native.Preview_2.0.1_x64-setup.exe](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.0.1/Aurora.Native.Preview_2.0.1_x64-setup.exe).
- Official installer SHA-256: `8598d38b9ab7e0e1cff8ba42acb0124374e826d4c1b89a19f359176e7cbf28dd` (10,218,269 bytes).
- Official extracted executable SHA-256: `7c31def9b9c1fdab805e5b692ecb071cc3b4fad9b82217f3c740e199f64c20c6` (33,668,096 bytes); extracted without executing the installer.
- Protected workflow: [run 41547960793](https://github.com/kalibsolomon-pixel/Aurora-Launcher/actions/runs/41547960793), exact source above. Resolve, Windows build/signing, publication and authority jobs succeeded.
- Accepted signed CI artifact: `13193331076`; GitHub reported ZIP SHA-256 `e47c793e2622911ea8ba6bcc4512946946e388be907487a768e48baf52ce22f0`. Its extracted signed files were verified before the publication review.
- Local optimized build and NSIS result: passed (`cargo build --release --locked`, 9m31s; existing NSIS 3.11). Local EXE and installer ProductVersion/FileVersion are 2.0.1; PE x64 and production package/feature rejection guards passed.
- Local validation-only executable SHA-256: `8346b486aee0a05780fd313bcef905236cc4e553cdfe86e7927f980090d1bf00` (34,161,152 bytes).
- Local validation-only installer SHA-256: `9d42008fe9ab6106cbd982801598678e445dc0609f1c588195ef49440e537303` (10,238,436 bytes). These local builds have no production signing/authority configuration and are not candidates for publication; the official hashes above are from this run's exact signed CI artifact.
- Production installer and envelope signatures passed on the exact downloaded CI files and independently downloaded public files. The complete signed inventory matched and the extracted x64 payload contains the established package, production public key and native discovery endpoint, with no acceptance flags or test identity.
- Production trust fingerprint retained: `bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc` (SHA-256 of trimmed encoded public key).
- Authenticode remains unconfigured under the established 2.0.0 publisher policy. Updater signatures do not claim Windows publisher identity.
- The owner explicitly delegated the three required reviews. Each review used the normal protected GitHub environment gate; protection rules and signing configuration were unchanged.

The actual reviewed current workflow is `.github/workflows/native-release.yml`;
`native-preview-release.yml` was renamed during the native-only transition. No
workflow or signing identity was created. Requested inputs retain minimum native
version `1.4.2`, exact source SHA, publication/advancement enabled and the observed
native authority parent. Publication and authority passed their separate protected gates in order.

## Notification acceptance

The existing single notification slot now anchors inside the Launcher window,
16 logical pixels from right/bottom using `Theme.space-4`. On Discover its width
leaves the centered Show More (the reported Load More) control clear, including
720×520. Other pages retain the existing maximum width within the content area.
There is no full-window input surface; only the Toast bounds consume pointers.
The single slot bounds visible notifications to one, with the existing replacement
and automatic disappearance behavior. Download scheduling/backend behavior is unchanged.

`discover_download_notification_clears_pagination_at_every_size_and_scale` passed
for 720×520, 1120×760 and 1600×1000 at each of 100%, 125%, 150% and 200% scaling.
It renders the real AppWindow with the shipping Slint software renderer, dispatches
actual pointer events to Install, scrolls results and clicks Show More through
transparent space beside the notification, then clicks Dismiss. It exercises
completion replacement, same-window resize/DPI changes, multiple Downloads rows,
progress text and Cancel/Retry keyboard activation. The Install callback uses a
synthetic operation: no provider download, real instance mutation or gameplay is
claimed. Desktop movement/maximize behavior and a real provider download remain
manual acceptance; headless coordinates do not depend on desktop/monitor origin.

Representative [minimum](native-2.0.1-ui/minimum.png),
[typical](native-2.0.1-ui/typical.png) and
[200% minimum](native-2.0.1-ui/minimum-200.png) renders were visually inspected,
as was the larger layout. These are headless renders, not desktop captures. The
prior centered source and its overlap were inspected; the checked-in images show
the corrected state. No decorative treatment was added or removed. Keyboard
Dismiss/Cancel/Retry semantics are retained; no comprehensive accessibility audit
or new performance measurement is claimed.

A separate real Windows debug boot used an empty, marked temporary managed-data
root. The exact process reported that root, opened its HWND and exited cleanly
through CloseMainWindow. The owner's existing launcher process was preserved.
This is a development boot, not acceptance of a production-signed installer.

## Integrity and regression results

- Core: **1,114 passed, zero failed, 31 opt-in tests ignored** (ignored tests are not passes).
- Native: **53 unit tests + five integration tests passed**, zero failed/ignored.
- Eight release-tool tests passed, including wrong-version title rejection before tag mutation, signatures, identity, downgrade/drift, immutable publication and authority guards.
- Core/native all-target checks and both Rust formatting checks passed. Version contract and whitespace checks passed.
- The full approved correction remains present: runtime-mutable classification/provenance, precise missing/hash diagnostics, interrupted promotion replay/authoritative validation, runtime-only resolver, Smart recovery and deterministic destination sync protection.
- Relevant passing regressions include mutable drift versus deterministic mod damage/missing files, interrupted promotion, Java resolution with damaged pack while launch refuses it, pack-owned sync destinations, deterministic post-commit tamper/rollback and recovery tests. Launch readiness remains strict.
- Earlier correction evidence is reused: 24 deterministic files verified, 35 mutable files skipped and zero failures in read-only affected-instance validation. That forensic/instance validation was not repeated and no owner's instance was repaired by this release task.

## Authority, public downloads and preservation

- Native authority before: `c663d66f28fee33d611021200fae5f00b42668e8`, version 2.0.0.
- Native authority after: `0146fba27066b10166c2444e78d9d44798e4cb05`, a non-forced fast-forward with the previous authority as its sole parent. The public signed discovery envelope is byte-identical to the release; SHA-256 `fd59876504daa817153db3ec76bb6c8a4cdefbee52464c2c9237ac459cf30c5c`.
- Legacy WebView authority before/after: `df7e824b2e49f8a5b9d2447ca2314bcafa21d119` (unchanged).
- All six public 2.0.1 files were downloaded without authentication and matched the accepted CI inventory, with both signatures independently verified. The public 2.0.0 executable contains the same production trust key and discovery endpoint; the signed 2.0.1 offer is eligible for it (minimum 1.4.2). Legacy WebView authority and package isolation remain unchanged.
- All six immutable 2.0.0 assets were downloaded without authentication and matched GitHub's recorded SHA-256 values; the existing installer/envelope signatures and inventory passed `native-update.mjs verify` with the production public key.
- 2.0.0 installer SHA-256 remains `b72888bb1a57921e94b0fdbdec895c834c2f1522b4437f346827e1de3e28e09f`.
- The immutable 2.0.0 release envelope remains unchanged at SHA-256 `63e1187c2528aaff4344e6f034815d675324799c7e193d2d46690658b1b0ba10`; discovery now serves the 2.0.1 envelope above.
- 2.0.0 tag remains `33af6a6b619c375ed84b7ff495ff125c282386cd`; immutable assets/history were not overwritten.
- Initial status/index/tracked/untracked/history/worktrees, public assets and both authority refs were recorded under the task's temporary evidence directory. Every baseline tracked file still exists; no deletion diff or generated game/runtime data was staged. The original correction branch remains preserved.
- No cleanup/reset/force-push, key rotation, protection change, owner instance/world/account/credential access, WebView installation change or Client modification was performed. Only the disposable boot process was closed; the pre-existing owner process remained running.

## Remaining owner acceptance

No disposable Windows VM/profile was available for a live production-signed
upgrade/restart. Exact signed bytes, installer payload identity, public update
eligibility, source-level UI interaction and the isolated development boot were
verified. The unchanged installer was not run against the owner’s installation,
and no owner account, instance, credential or world was loaded or changed.

Once 2.0.1 is installed, the owner can perform:
Repair → Ready → Launch Minecraft → Exit → Check. The affected instance should
remain Ready. This owner-only follow-up has not been performed here.

Detailed logs, hashes and headless renders remain in `C:\Users\kalib\AppData\Local\Temp\aurora-native-201-20261102-182821`. The documentation follow-up records the completed workflow without changing the tagged release source.
