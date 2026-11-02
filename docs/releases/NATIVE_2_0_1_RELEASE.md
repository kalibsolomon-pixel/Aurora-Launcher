# Aurora Launcher Native 2.0.1 release record

**BLOCKED — PROTECTED APPROVAL REQUIRED**

Both corrections are integrated and locally validated. Public 2.0.1 publication,
production signing and authority advancement have not completed. This document
must not be read as a released verdict.

## Source and release identity

- Starting branch: `codex/native-2.0.0-release`, clean index/worktree and no untracked files.
- Starting full SHA: `5deab8326c870175be91afc40d528088b4096924`.
- Starting remote main: `1fcc562b0e176e946c3889a35cfa7877deefd27c`.
- Final integrated release-source SHA: `82620e57b5c8b933dc970fae02200635df8cfa7c`.
- Integrity correction: `5deab8326c870175be91afc40d528088b4096924` (already present; not cherry-picked again).
- Notification correction: `62fc4e0e96633b74af96ad4767a913dd877a9980`.
- Version, title and notes: `e9da01dab6384270f39b552e8aac767abd1c9fb5`.
- Integration: [PR #2](https://github.com/kalibsolomon-pixel/Aurora-Launcher/pull/2).
- Version/tag: `2.0.1` / `native-v2.0.1` (tag not yet created).
- Intended title: Aurora Launcher Native 2.0.1 — Modpack Reliability and Download UI Fixes.
- Intended release URL: https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/native-v2.0.1 (not yet published).
- Identity retained: `com.aurora.launcher.native-preview`, channel `native-preview`, Slint, Windows x86_64, `nsis-per-user`.
- Native/core Rust metadata, both Cargo lockfiles, npm metadata/lockfile and workflow default agree on 2.0.1. PE resources, application display and NSIS derive this version from native Cargo metadata.

## Artifacts and protected workflow

- Installer: `Aurora.Native.Preview_2.0.1_x64-setup.exe`.
- Official installer SHA-256: pending protected CI signing; no public bytes yet.
- Official extracted executable SHA-256: pending the exact signed CI installer.
- Protected workflow: [Run 41547960793](https://github.com/kalibsolomon-pixel/Aurora-Launcher/actions/runs/41547960793), exact source above. Resolve passed; build is waiting for required owner review of `launcher-production`. Publication and authority jobs have not started.
- Local optimized build and NSIS result: passed (`cargo build --release --locked`, 9m31s; existing NSIS 3.11). Local EXE and installer ProductVersion/FileVersion are 2.0.1; PE x64 and production package/feature rejection guards passed.
- Local validation-only executable SHA-256: `8346b486aee0a05780fd313bcef905236cc4e553cdfe86e7927f980090d1bf00` (34,161,152 bytes).
- Local validation-only installer SHA-256: `9d42008fe9ab6106cbd982801598678e445dc0609f1c588195ef49440e537303` (10,238,436 bytes). These local builds have no production signing/authority configuration and are not candidates for publication; official hashes must come from this run's exact signed CI artifact.
- Production installer/envelope signatures: pending protected signing; no local private key or disposable test key used for distribution.
- Production trust fingerprint retained: `bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc` (SHA-256 of trimmed encoded public key).
- Authenticode remains unconfigured under the established 2.0.0 publisher policy. Updater signatures do not claim Windows publisher identity.
- Required owner action: approve the `launcher-production` deployment for run 41547960793 through GitHub Review deployments. This is the repository required-reviewer gate, not a request to change protections.

The actual reviewed current workflow is `.github/workflows/native-release.yml`;
`native-preview-release.yml` was renamed during the native-only transition. No
workflow or signing identity was created. Requested inputs retain minimum native
version `1.4.2`, exact source SHA, publication/advancement enabled and the observed
native authority parent. Publication and authority have separate protected gates.

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
- Native authority after/current: unchanged `c663d66f28fee33d611021200fae5f00b42668e8` pending protected release.
- Legacy WebView authority before/after: `df7e824b2e49f8a5b9d2447ca2314bcafa21d119` (unchanged).
- Public 2.0.1 download/signature/discovery verification: pending; no eligibility or distribution success is claimed yet. Keeping minimum `1.4.2` preserves the existing 2.0.0 upgrade path once signed 2.0.1 is published; native/WebView identity separation remains enforced.
- All six immutable 2.0.0 assets were downloaded without authentication and matched GitHub's recorded SHA-256 values; the existing installer/envelope signatures and inventory passed `native-update.mjs verify` with the production public key.
- 2.0.0 installer SHA-256 remains `b72888bb1a57921e94b0fdbdec895c834c2f1522b4437f346827e1de3e28e09f`.
- Existing public native discovery envelope SHA-256 remains `63e1187c2528aaff4344e6f034815d675324799c7e193d2d46690658b1b0ba10`.
- 2.0.0 tag remains `33af6a6b619c375ed84b7ff495ff125c282386cd`; immutable assets/history were not overwritten.
- Initial status/index/tracked/untracked/history/worktrees, public assets and both authority refs were recorded under the task's temporary evidence directory. Every baseline tracked file still exists; no deletion diff or generated game/runtime data was staged. The original correction branch remains preserved.
- No cleanup/reset/force-push, key rotation, protection change, owner instance/world/account/credential access, WebView installation change or Client modification was performed. Only the disposable boot process was closed; the pre-existing owner process remained running.

## Remaining acceptance and completion

After the required protected signing review, inspect this run's exact private
signed installer/envelope/inventory before approving publication, then verify
immutable unauthenticated public bytes before approving native authority
advancement. Complete exact production signature, executable/installer identity,
source/tag, native 2.0.0 update eligibility, public discovery and README/download
metadata verification before changing this verdict to RELEASED. Do not rebuild
accepted bytes between gates or install over owner data automatically.

Once 2.0.1 is installed, the owner can perform:
Repair → Ready → Launch Minecraft → Exit → Check. The affected instance should
remain Ready. This owner-only follow-up has not been performed here.

Detailed logs, hashes and headless renders remain in `C:\Users\kalib\AppData\Local\Temp\aurora-native-201-20261102-182821`. This evidence follow-up is on `codex/native-2.0.1-release-record`; main remains at the exact dispatched release source while approval is pending.
