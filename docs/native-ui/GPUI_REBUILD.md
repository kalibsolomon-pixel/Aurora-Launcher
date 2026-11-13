# GPUI rebuild continuity

The owner requested a new native Aurora Launcher interface on 2026-11-12,
preserving the Rust backend and existing features while reducing launcher
overhead. The user job is to select an instance, understand whether it can run,
and launch it; configuration and content tools remain available without
competing with that primary action.

## Current architectural direction

- Keep `core/` authoritative for installation, validation, accounts, credentials,
  configuration, content, sync, updates and process supervision.
- Develop a standalone host in `native-gpui/`, beside the supported Slint host
  in `native/`. Keep existing distribution builds working during the rebuild.
- Use `gpui-kit = 0.9.1` with default features disabled and `gpui-fast` enabled
  (`gpui-fast 0.1.14`). Use unstyled `gpui-base` behavior with Aurora-owned
  presentation. Do not introduce the styled component or assets feature.
- Reuse the framework-neutral typed API in `native/src/api.rs`. Send work to
  the Tokio runtime; deliver results and host events to the GPUI thread through
  an awaited channel. Do not poll for state or schedule idle redraws.
- Use the existing Aurora palette, spacing and typography as the starting
  vocabulary. Simplify surface treatment; use no persistent decorative motion.
  Appearance values still come from the core's appearance service.

The October decision in `FRAMEWORK_DECISION.md` describes the shipped Slint
host and the upstream GPUI 0.2.2 probe. It remains historical evidence; its
upstream GPUI measurements do not describe the selected GPUI fork.

## Evidence and limits

External evidence is in
`C:/Users/kalib/Aurora Launcher/work/rebuild/`:

- `probe/gpui-fast-bare.json`: earlier release probe measured 0% idle and
  minimized CPU, 89.6 MiB working set, 105.2 MiB private memory and 26.4 MiB
  dedicated GPU memory on this host. These are **bare probe measurements**,
  not measurements of a functioning launcher or a cross-machine guarantee.
- `spec/PARITY_MAP.md`: static feature inventory of the existing Slint host.
  Large controllers were spot-checked; this is a migration checklist, not
  proof that a replacement implements the features.
- `gpui/controls.md`: control-probe results, when available. Compile success
  does not establish native input or accessibility behavior.
- `integration/`: original repository status, HEAD, tracked-file list and
  hashes of critical existing files recorded before implementation.

Java was running when this phase began. The owner's standing desktop rule
therefore blocks application boots, screenshots and fresh resource measurements
while it runs. Do not terminate the game. Future diagnostic windows must stay
on the portrait secondary display, use `focus: false`, refuse to open if that
display is absent, and avoid real input or foreground activation.

## Acceptance before replacing the current host

1. Implement and verify the complete existing feature map: Home and widgets,
   instances and configuration, creation/deletion/recovery, content/import and
   update reviews, Discover, accounts, skins/capes, sync, media/desktop
   integrations, appearance, activity and launcher/Client updates. Track each
   capability as implemented, compile-tested, runtime-tested or still pending.
   A preserved backend API alone does not establish UI feature parity.
2. Preserve launch authority. Readiness comes from deep core verification with
   at most one check in flight; changes invalidate stale results and coalesce
   into a follow-up. Launch uses the exact selected instance/account, pending
   launch guards and Rust-owned secrets. No renderer-owned session or shell
   command assembly.
3. Preserve async behavior: selection/startup revisions, event routing,
   serialized/coalesced appearance writes, content folder revisions, review
   fingerprints, skin preview revisions and one committed widget gesture.
   Test out-of-order results, cancellation, failure and worker termination.
4. Diagnostic mode must isolate data, account credentials and integrations.
   Creating a disposable data directory alone does not isolate OS credentials
   if credential-bearing commands remain enabled. Do not sign in, restore
   sessions, start Discord/sync or launch games during UI acceptance.
5. Check text editing, IME, clipboard, selection, keyboard activation, visible
   focus, tab order, Escape/cancel, dialogs and popup focus restoration, disabled
   states and native accessibility semantics. Inspect representative native
   sizes and 125–200% DPI with screenshots when the desktop rule permits it.
6. Run formatting, checks, relevant tests and release builds. Compare a working
   release host with the supported launcher for startup, visible/minimized idle
   CPU, private/working-set/GPU memory and navigation/scroll responsiveness.
   Idle measurements must exclude attached accessibility inspection clients.
7. Review changed/deleted files and preserve all pre-existing work. No pushes,
   publication, installation over the owner's launcher or release workflow
   dispatch is part of the rebuild request.

## Implemented development surfaces

`native-gpui/` now builds an experimental host using the shared typed API.
It exposes instance/account selection, readiness and available play/preparation
actions, Microsoft sign-in/cancel and saved appearance choices. The control probe's
semantic button wrapper is integrated with focus, selection and disabled
states. Theme surfaces and accent colors resolve from persisted appearance.
Worker failures return sanitized terminal errors instead of leaving a busy flag
waiting for a missing completion. The release executable uses the Windows GUI
subsystem and has an optimized profile.

The integrated host has eighty-eight passing tests, including exact instance and
account matching, stale-readiness, delayed launch replies after a terminal
process event, malformed events, worker failures, appearance queue completion,
loader pins, creation without Aurora service availability, stale content and
import reviews, and Discover requests, reviews, cancellation and destinations. Process events and launch
results now retain running/terminal state and failure diagnostics; Play is gated
against the current target/account and running processes. Failed launches trigger
read-only recovery inspection. The async runtime is owned by `main`, with a
two-second bounded shutdown. Formatting/checking and a release rebuild are
recorded in the external integration logs. No application boot, visual acceptance, keyboard/accessibility
runtime check or functioning-launcher resource measurement has run.

The Instances page composes catalog-driven creation and a configuration editor.
Creation loads official Minecraft and core-declared platform catalogs only when
the user opens New instance. It supports snapshots, automatic or exact pinned
loader selection, and an explicit Aurora choice guarded by the current
compatibility result. Published Aurora releases influence the initial version
without blocking plain creation or overriding a user selection. Catalog,
loader and compatibility responses reject superseded selections. Failed
creation reports a retained record for retry and refreshes instance state.

The editor provides selection, rename, typed-name deletion, configuration
save/revert, Java checks, validation, install/retry and folder opening. Game and
loader catalogs load after user activation, with snapshots and exact saved pins
kept visible. Unchanged loader and custom-Java objects retain their exact JSON;
unknown configuration fields survive expected-baseline saves. Saved game changes
require the explicit Install saved configuration action. Aurora enable/disable
uses a fingerprinted modal review; recovery uses explicit inspection, a frozen
fingerprint and the required removed-mod choice. Neither path silently installs
or discards local content. Failed applies invalidate previews and refresh root
readiness/content views as well as the editor, because rollback can retain state.
Deletion uses a base modal; opening a folder preserves the edit state. Registry
refreshes preserve unrelated drafts. Host progress does not repeatedly reload
the registry. The root avoids issuing a redundant editor read when that editor
already refreshes after its own mutation.

Settings is constructed when first visited. It exposes core theme options,
accent presets and validated custom hex colors; appearance writes carry full
snapshots and serialize/coalesce changes. Superseded reads cannot undo a newer
save. The root receives acknowledged appearance immediately and invalidates old
snapshots before reloading. Background and motion values are preserved and shown
as unavailable in this preview; no working effect selector is implied.
Discord preferences, explicit reconnect, desktop shortcut create/remove, Start
menu shortcut creation, opening the launcher folder and requested update status
are integrated. Live Discord connection events invalidate older reads, and
delayed save replies preserve newer connection status. Constructor reads never connect Discord, check updates or create
shortcuts. The Start menu button calls its distinct core operation.

`integration/feature-pages-{check,tests,release}.log` records the Settings/creation integration;
`integration/feature-pages-final-*.log` records the final event-wiring revision.
That revision passed all-target checks, twenty-four host tests and the
optimized release build. `feature-pages-final-binary.json` records its hash,
25,411,584-byte size and Windows GUI subsystem (2). Its compiled source is captured
in the immutable `feature-pages-review-3` snapshot. That binary has not been booted
or measured as a functioning launcher.
Standalone harnesses remain useful for independently checking feature modules.
Compilation does not verify modal focus restoration, popup placement, scrolling,
keyboard operation or native accessibility behavior.

Content is now a separate page, constructed on first visit. It provides local
instance selection, mod/resource-pack/shader-pack inventories with core-owned
permissions and provenance, toggles, removal review naming the exact file, and
folder opening. Local file import requires a core-issued preview ticket and
fingerprint followed by explicit installation. Target/path changes discard
reviews; stale results are rejected and their tickets revoked. One file preview
can run at a time, and the reply pump drains queued tickets after view disposal.
Row actions retain the target/revision they displayed. Inventory mutations
refresh root readiness and the instance editor; opening a folder preserves state.

Discover is also constructed on first visit. It supports Modrinth kinds,
categories, sorts and compatible-instance filtering; capped/deduplicated paging;
project/version details; exact reviewed content and modpack installation; new
or compatible existing pack destinations; optional files; explicit quick install;
cooperative cancellation; and retry/resume of retained failed pack instances.
Selections, requests and fingerprints retain their serial/identity guards.
Categories and searches are requested after navigation, not at launcher startup.
Root event routing forwards modpack progress, and settled installs invalidate
instance/content state and readiness. `content-discover-review-1` captures the
integration for independent review; `content-discover-*.log` records validation.
The combined snapshot passed all-target checks, sixty-three tests and an
optimized release build; `content-discover-binary.json` records its hash and
28,356,096-byte size. It has not been booted or measured.

Independent transaction review found two stale installed-marker defects.
Content mutations now invalidate Discover reviews and refresh its inventory;
returning to an existing Discover target also refreshes inventory. Each inventory
read has its own generation plus exact target/kind identity, so an older read
cannot overwrite a newer post-install result within the same search. Three
additional regression scenarios pass, bringing the host suite to sixty-six.
The review is `integration/CONTENT_DISCOVER_REVIEW.md`; follow-up evidence is
`integration/DISCOVER_INVENTORY_FIXES.md` and `discover-inventory-fixes-*.log`.
An optimized rebuild of these fixes remains pending the next feature checkpoint.

Discover currently runs one install at a time in its page. The shared Downloads
queue, project artwork, local `.mrpack` navigation, and origin-aware navigation
remain pending. Archive-review progress uses honest static wording; no periodic
byte polling was introduced. Data packs have no current core content kind.

Content now embeds the managed-update module using its existing instance and
kind selectors. Explicit checks, pin/unpin, reviewed single/bulk updates and
managed-modpack updates use core fingerprints; pack conflicts require keeping
local files. The parent and child mutually block transaction starts, forward
pack progress and invalidate root readiness/Discover after mutation settlement.
An inventory request serial prevents an older read restoring pre-update rows.
Child-origin mutations preserve the update target revision for the child's
follow-up check; external content changes advance it. Same-target invalidation
also advances a child epoch, so checks/previews already in flight cannot restore
discarded availability or reviews. Constructors and target changes never check
providers automatically. The integrated suite passes 141 tests and all-target
checking; final release evidence uses `managed-updates-final-*.log` and
`managed-updates-binary.json` when the build completes. A separate immutable
review of the delivered module is pending; it predates parent integration and
the added same-target invalidation epoch. Native interaction is unverified.

Sync is now a lazy page with saved-policy initialization at production startup.
Diagnostic mode still constructs no Host and submits no initialization work.
The page edits a complete acknowledged policy through an explicit Apply/Revert
workflow, including participants, categories, selected paths, exclusions and
per-participant overrides. Saved missing participants remain identifiable by id.
Conflict resolution uses a base modal with exact conflict identity, separate
winner buttons, Escape/defer and busy veto. Server-list conflicts remain pending.
No polling or periodic presentation work was added; the existing backend owns
its authorized saved-policy worker.

Integration review corrected stale policy reads after save/revert, live status
versus snapshot ordering, stale conflict reads during resolution, duplicate
resolve completions clearing newer work, and same-participant exclusion inputs
not restoring on Revert. A failed policy read retains the last acknowledged
display but blocks edits/writes until a valid read. Startup completion cannot
clear unrelated operation flags. A finished backend sync invalidates root
readiness, content inventory and Discover markers. The root routes host events
and acknowledged page mutations without saving draft edits on navigation.

The Settings/creation review is `integration/SETTINGS_CREATION_REVIEW.md`.
Only an explicit Minecraft selection or starting creation now freezes the
provisional version; platform, loader and Aurora choices leave the late
published-release default eligible. Accent presets now use the base semantic
button with focus and disabled states. Select popups have a viewport bounded by
window space, scrolling and automatic reveal of the keyboard highlight. These
interaction fixes compile; native focus, scrolling and accessibility acceptance
still require a permitted GUI session.

The Sync integration initially passed all-target checking and eighty-three
integrated tests (`sync-integration-tests.log`), including the earlier Discover
inventory races. A subsequent check overlapped the instance-editor implementation
landing and recorded its in-flight compile errors in
`sync-integration-editor-inflight-check.log`; that is not a final editor result.
The isolated Settings, creator and Sync harnesses then passed 12, 8 and 18 tests
respectively (`sync-settings-creation-tests.log`). Formatting passes for the
coordinator-owned files. Whole-host checks and an optimized rebuild must run
again after the concurrent editor work settles. The older binary manifest does
not represent this source checkpoint. No rebuilt-host runtime, visual or
resource acceptance has been performed.

The independent Sync integration review (`SYNC_INTEGRATION_REVIEW.md`) found no
substantiated actionable defects after verifying all twenty-two snapshot hashes.
It explicitly leaves native interaction and performance acceptance open. The
instance-editor expansion then passed all-target checking and eighty-eight host
tests. Follow-up integration corrects failed Aurora/recovery apply invalidation;
current binary checking, tests and optimized build are recorded separately in
`editor-sync-final-*.log`. The final optimized build passed at 30,309,888 bytes with Windows GUI subsystem 2;
`editor-sync-binary.json` records SHA-256 `6748B656E50AAB591EAB9A552517ACC599F0279877F84D203A9920FBE70D9972`. All 141 captured compiled
source/manifest inputs stayed unchanged throughout the checks/build. The initial
editor build is an earlier checkpoint; its source must not be confused with the
final compiled-source manifest. The independently reviewed Sync snapshot still
uses earlier editor context; the new editor has a separate immutable snapshot.

## Remaining UI parity

The Accounts page is now integrated lazily, with explicit session refresh,
reviewed removal, account selection and system-browser Microsoft sign-in. Read
serials and mutation epochs reject stale listings. Cancellation replies are tied
to their original attempt, and a new sign-in waits for an older cancellation
reply to settle. The Accounts review found that the original global cancellation
could miss dispatch or arrive after the browser callback. GPUI now creates a
single-use backend request and its cancellation capability before dispatch.
The core observes sticky cancellation across callback and token/profile work;
an atomic boundary immediately before saving rejects cancellation as TooLate.
The page then reports finishing and awaits the account outcome. Failed
refreshes invalidate root readiness and reload the authoritative account status.
Launch/preparation and account mutations have mutual guards. Account rows use
stable account identifiers, and the removal dialog stays mounted across empty,
loading and error states so its focus restoration can run. Account rows now
render validated cached heads, then request missing signed-in heads through the
existing cache-first API. This can restore a saved session on a cache miss,
matching the supported host. Artwork reads do not emit mutation events. Listing
and operation generations reject late replies after removal or status changes;
fresh cached pixels can replace an older retained head without an old fetch
overwriting them. The Accounts harness passes 23 tests, including the new
cache-generation regression. Native image display remains unverified.

The independent instance-editor review (`INSTANCE_EDITOR_REVIEW.md`) identified
three issues. Corrections preserve the configuration draft and original expected
snapshot through rename/install/retry, identity-key loader catalogs and clear
them before requests, and bound Aurora/recovery review text to the viewport.
Review bodies use focusable scroll regions with arrow/Page Up/Page Down/Home/End
handlers; decision controls stay outside the text scroller. These interaction
changes still need native acceptance. All 31 authentication-flow tests passed,
including deterministic pre-dispatch cancellation, three post-callback phases,
the saving boundary and an old capability versus a later login. These fixtures
use loopback endpoints, a scripted callback and in-memory credentials; no real
sign-in occurs. Final GPUI validation and binary evidence are recorded in
`accounts-editor-final-*.log` and `accounts-editor-binary.json` after the source
checkpoint is verified. The earlier build detected source drift and is not
accepted as evidence for the cancellation correction. Older manifests describe
older source. The supported Slint commands retain their existing signatures;
the shared backend additionally observes post-callback cancellation.

`AUTH_CANCELLATION_REVIEW.md` verified the initial cancellation correction and
found a lifecycle gap for early failures/dropped requests plus error/cancel
arbitration. Requests now close their capability on drop and atomically settle
errors against cancellation, including application setup and slot rejection.
Three additional deterministic tests cover unpolled futures, rejected attempts,
failed setup and both outcomes of an auth-error/cancel race. The earlier Accounts
checkpoint built at 30,736,896 bytes with Windows GUI subsystem 2 and SHA-256
`517561DCC05463D0A52B58867AB44584EC59DC30D4E29E2DAF2124DE8100E532`;
all 142 recorded inputs were stable and the 3,240-path preservation check passed.
That binary predates the lifecycle correction and Content update integration.
The new managed-updates checkpoint captures 143 compiled inputs, including the
newly registered module, and records its own release/preservation evidence.

That checkpoint passed all-target checks, 141 integrated tests and the optimized
build: 32,156,160 bytes, Windows GUI subsystem 2, SHA-256
`306CB5645F7130FD1BEF1E3DA40152C3F5691E61523EF581AB23EA8E9E89388C`.
All 143 inputs remained stable; all 3,240 baseline paths and protected hashes
were preserved. It predates the subsequent managed-update review corrections.

The two managed-update reviews identified a mismatched progress event, stale
completion text under a superseded target, and Close buttons incorrectly
disabled by a parent preview. The page now consumes `modpack-update-progress`,
rejects superseded result text while preserving terminal mutation notifications,
and keeps dismissal available whenever its own operation is idle. The broader
Unpin rule is deliberately documented: a pinned record can rejoin future updates
even when no update is currently offered. Pack-update progress still lacks
instance attribution in the core payload; labels are advisory and the exact
operation's completion remains authoritative.

Skins preparation reuses the existing CPU player renderer, latest-pose render
gate and owned native file pickers. A small image adapter validates decoded RGBA
length/dimensions and converts to GPUI's BGRA format once per image. Its direct
image, raw-window-handle and Windows bindings already existed in the lockfile;
the image dependency adds no codec features. No image encoding or second dialog
stack is introduced. The integrated suite passes 156 tests, including the shared
renderer and gate tests. `managed-updates-reviewed-final-*.log` and its binary
manifest describe the next source checkpoint; release validation is in progress.
This preparation does not integrate a Skins page or establish runtime acceptance.
The reviewed checkpoint subsequently passed: 32,154,624 bytes, Windows GUI
subsystem 2, SHA-256
`EEAA78A9FF6ED535493F43A005EF2B285DDD9835F2EC0914D49024788E000FC2`.
All 148 source inputs were stable, and the 3,240-path/protected-hash preservation
check passed. This binary predates the local-modpack integration below.

Instances now embeds the explicit local `.mrpack` importer. It uses the owned
native picker and the existing core's exact Modrinth identification, then opens
Discover's normal hosted review pinned to that version. No install or review is
started automatically. Missing exact versions never fall back to the provider's
default. Parent launch/preparation and import starts have mutual guards; an
active Discover installation cannot be replaced by navigation. A late instance
bootstrap cannot invalidate pack details that do not depend on the content target;
merge destinations retain their separate project/version identity guards.
Pack-import duplicate harness fixtures were removed; its twelve harness tests
now consist of eight module fixtures and four shared API tests. Four navigation
fixtures bring the integrated host suite to 168 passing tests. No GUI was run.

The completed Skins page is now registered as page 7. The local library works
signed out and supports PNG import, favorites, rename, reviewed removal and real
head thumbnails. Signed-in actions save the current skin, review an exact preset
and model before applying, and inspect/select/disable capes through the unchanged
API. The root supplies acknowledged account snapshots and invalidates the old
appearance immediately when account work starts. Account and skin mutations and
root launch/preparation actions have mutual guards.

The separate `SkinPreview` entity reuses the existing CPU renderer/gate, starts
inactive, and activates only on page transitions. Pointer/keyboard rotation,
zoom, front/back and outer-layer controls request bounded renders; there is no
polling or idle animation. Appearance updates arrive through retained event
subscriptions, including synchronous account invalidation. A parent correction
drains only requested library/cosmetics reloads: crossed replies cannot keep
enqueuing each other's reads. The integrated suite passes 187 tests, including
both crossed-reply orderings. `skins-guarded-dispatch-tests.log` records this
checkpoint. Independent `SKINS_ACCOUNTS_INTEGRATION_REVIEW.md` verified all 26
snapshot hashes and found a write-invalidated read could be stranded. Writes
now preserve replacement intents for both outstanding domains. A regression
checks stale replies before/after successful, failed and cancelled writes,
requiring fresh snapshots and return to idle. The prior optimized binary predates
these changes, and GUI/accessibility/performance acceptance remains unverified.

Home now embeds explicit Aurora Client update controls for the exact selected
registry instance. Construction, retargeting and refresh make no provider calls.
Check requests a core preview; a bounded dialog freezes the target/revision and
fingerprint for explicit approval. Apply uses the existing verified transaction;
up-to-date, installed-newer, blocked and malformed offers remain distinct. The
root routes real `client-update-progress` phases and blocks launch/preparation,
account/skin/import starts and navigation during the single preview/apply.
Running Minecraft, sync, root reloads and readiness verification block new Client
work. Root mutations invalidate old review revisions synchronously; terminal apply
replies trigger registry, editor and content reloads even after failure. A retarget
to an unavailable instance cannot hide the active operation's frozen-name dialog.
The integrated suite passed 209 tests (`skins-client-review-tests.log`). The
independent `CLIENT_SKINS_FINAL_REVIEW.md` verified all 24 snapshot hashes and
confirmed the Skins race correction. It found that the apply-triggered reload
cleared terminal feedback and an unavailable target hid a running check. Completed
feedback now has its own instance identity, survives reload revisions or a failed
registry read, and clears on a different instance or a new Check; dismissed errors
do not reopen. Review fingerprints remain invalidated. In-flight labels render
before applicability messages, retaining the frozen name until terminal settlement.
Two regressions replay success/failure through reload revisions and unavailable
targets during both preview and apply. All 211 tests pass in
`client-review-correction-tests.log`. Native event delivery and UI behavior still
need functioning-host acceptance. This is the
Aurora Client mod updater, and does not change the Slint-only signed launcher
self-update distribution boundary.

The `skins-client-reviewed` optimized checkpoint passed formatting, all-target
locked/offline checking, all 211 integrated tests and the release build. Its
executable is `C:\Dev\aurora-gpui-build\release\aurora-launcher-gpui.exe`,
35,407,360 bytes, Windows GUI subsystem 2, SHA-256
`C91111560A844907F82540A8097DF08C7506701924C9DE3BB84BFA370A0338B5`,
built `2026-11-12T20:44:08.6231183-05:00`. All 152 recorded source inputs stayed
stable during validation; all 3,240 baseline paths and protected hashes were
preserved. Evidence is `skins-client-reviewed-{binary,preservation-check,
compiled-source-hashes}.json` and `skins-client-reviewed-final-*.log`. No GUI was
started: Java remained running. This checkpoint establishes build/test results,
not visual, accessibility, functioning-host performance or complete feature parity.

Downloads is now registered as page 8 and shares one retained queue with
Discover. Prepared, fingerprinted requests serialize by instance while distinct
instances/new packs can run concurrently under the unchanged core transfer
permits. Cancellation is cooperative; exact attempt guards protect retries and
bounded one-shot contention waits. Failed new-pack jobs resume the core-reported
kept instance. Root guards consult both live queue state and a busy mirror;
root/Client preparation blocks new submissions and retries while Cancel remains
available. Local-pack navigation no longer needs the old singleton-install gate.

Parent corrections start eligible successors when a cancelled backoff loses its
scheduled wakeup, and trim finished history on settlement as well as submission.
Two new fixtures cover those cases; the integrated suite passes 225 tests in
`downloads-parent-correction-tests.log`. Queue events refresh Discover inventories
and registry/editor/content state on mutation settlement. Per-task transfer bytes
are sampled on real events; live aggregate byte counters remain pending under
the no-presentation-polling constraint. Independent static review of the verified
24-file snapshot found no actionable defects; see
`DOWNLOADS_INTEGRATION_REVIEW_2.md` in the rebuild integration evidence directory.
Runtime, visual, keyboard/focus and performance acceptance remain unverified.
The optimized `skins-client-reviewed` binary predates these Downloads changes.

Settings now offers Reduce motion through the existing complete-snapshot
appearance queue (enabled writes 0; disabled writes 50), plus package-derived
version and MIT license information in About. Malformed appearance reads never
offer a fabricated writable preference. The focused Settings harness passed 13
tests. Parent inspection confirmed the existing queue/failure path is retained;
the explanatory copy was shortened. Runtime behavior remains unverified. General
motion-speed editing, background rendering and the HSV picker remain pending.

The Home Media card now hosts the event-driven `Media` entity (`media.rs`).
A pure rule runs its worker only with a Host, an active Home page and Media
enabled in the committed layout; gesture previews never start or stop it.
Hiding Home, disabling/resetting the widget, a failed layout read or dropping
Home revokes the worker. The entity is created lazily on a foreground task, never
in render. The integrated suite passes 268 tests (`media-home-test.log`) and the
Home harness 46. Native session interoperability and display remain unverified.

A grep-based parity audit (`work/rebuild/spec/GPUI_PARITY_STATUS.md`) then
drove a gap pass, each item compile- and fixture-tested only:

- Home shows readiness blocker labels with their Slint actions (sign in,
  install Java, Java settings, review recovery in the editor, open instance,
  review mods), a compact instance picker with stored icons, the bounded
  stale-running re-check (five, 750 ms), a fatal-load screen with Try again
  (superseded attempts dropped) and one startup update check whose status is
  shown in Settings. The host still offers no launcher package installation.
- Content rows show mod/pack icons (`mod_icons`/`pack_icons`, Modrinth artwork
  fallback); Discover cards and details show project artwork through a bounded,
  epoch-guarded cache (`artwork.rs`). Only the first 64 content rows request
  icons, because GPUI reports no row visibility.
- `.mrpack` files dropped on Instances and `.jar`/`.zip` files dropped on Content
  enter the existing reviewed import flows. Pack merging was already complete.
- The editor sets instance icons through a searchable, virtualized picker over
  the shared Material Symbols table (`symbols.rs`), shows performance advice by
  the memory field and reads managed Java status on validate/install/retry.
- The window draws its own title bar (`chrome.rs`): platform hit-testing
  provides drag, snap, resize and maximize insets; caption buttons have
  accessible names but no keyboard focus yet.
- `borealis.rs` ports the Slint band layers and pose math. Bands are composed
  off-thread into one bounded image because GPUI has no rotated-image
  primitive; the single animation task exists only while motion > 0, Borealis
  is selected, the window is shown and rendering is accelerated. Fresh profiles
  default to the Simple background (`core/src/appearance.rs`), so they schedule
  no animation work. Settings adds the background selector, motion slider with
  live preview and the HSV accent picker; navigation cancels previews.

The integrated suite passes 362 tests and every example harness passes
(`borealis-root-*.log`, `ghost-button-test2.log`). Borealis animation cost,
occluded-window behaviour and every visual result still need a permitted GUI
session.

A final gap pass ported the playtime chart layout, the sidebar Downloads
activity badge and settle notices, caption-button Tab focus with Enter/Space,
Slint's Quick Play refusal/progress wording, and origin-aware Discover
navigation (Content "Find more" opens Discover for that instance and kind;
"Back to instance" returns while the instance exists; the origin preselects a
compatible destination). `historical_playtime` and `quick_play_server` are
unused by the Slint host as well. The refreshed parity status lists 80 of 82
rows implemented, one not applicable and one deferred: launcher update
download/installation, which waits on the distribution decision below.

The `parity-complete` release build is 43,938,816 bytes, SHA-256
`5B0C07239B2C424136D580E21ADB0CDEDF71511F69DA7FCCC3FBEABC2D092222`
(`parity-complete-binary.json`). At that source state the GPUI host passed 376
integrated tests and all sixteen harnesses, the core passed 1,212 tests and the
supported Slint host passed 105. Java remained running, so the host has not
been booted, inspected or measured.

Remaining before replacement: the launcher distribution decision, then runtime
acceptance items 4–6 above (visual/DPI review, keyboard/IME/accessibility,
Borealis and idle resource measurements against the Slint host). The preserved
Slint application remains the supported feature-complete host. Source modules
and passing compile harnesses alone do not establish integrated or runtime parity.

## Shared supervisor correction

Static review found the existing supervisor used `kill_on_drop(true)`, contrary
to the supported host's stated contract that Minecraft survives closing the
launcher. A disposable fixture test reproduced termination on runtime shutdown
before the change. `core/src/launch/process.rs` now uses `kill_on_drop(false)`;
the supervisor group passes seven tests, including the new regression. Five
helper subprocess tests are intentionally ignored as direct suite entries.
The GUI and the owner's game were not used for this check. Log drains and final
supervision notifications do not continue after launcher shutdown.

The rest of the backend and the existing Slint host are preserved. This is a
targeted correction to game-process lifetime, not a new process termination API.

## Distribution boundary

The current signed native updater explicitly requires `runtime == "slint"`
in `core/src/updates/native.rs`. Keep the signed schema and validation intact.
The development GPUI host must not offer self-installation of Slint packages.
Distribution integration needs a reviewed runtime compatibility decision,
packaging changes and acceptance of exact signed bytes before replacing the
supported host. Preserve the public Aurora Launcher branding and stable native
data/credential identifiers; an experimental executable is not a new release.

No Aurora Client source changes are required by this UI rebuild.
