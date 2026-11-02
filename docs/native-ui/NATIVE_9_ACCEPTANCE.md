# Aurora Launcher — NATIVE-9 implementation and acceptance

**Verdict: PARTIAL — PRODUCTION PREPARATION INCOMPLETE.** The NATIVE-9 local
implementation, installer, picker/import and updater-verification gates pass,
and two defects found during the final review are fixed. Specific native
functional-parity gaps and the signed end-to-end self-update remain open; see
[Final continuation](#final-continuation-claude-2026-10-31) and
[Release readiness](#release-readiness).

Windows x64, 2026-10-31 / 2026-11-01. Branch `codex/native-ui-migration`.
Implementation commit (Codex): **`1eb5ad3f3799618c80cdcc049f68d0beae841c95`**.
Final continuation commits are listed in the continuation section.
Starting HEAD was **`a5b1fc7e9467691cd53a8515abadb310d9287c0b`**, the committed
NATIVE-8 acceptance, with a clean index and working tree. Its parent was the
historical continuation reference `117dab7ed7e5f317ccf3546611a257a718001cb4`.
NATIVE-8 was confirmed committed rather than inferred from its report.

This report distinguishes implemented behavior, deterministic/synthetic tests,
real native UI checks, and owner-attended acceptance still required. It is not
permission to ship, replace stable, activate a native authority, or claim full
functional parity. The existing NATIVE-8 acceptance and its bounded performance
evidence remain intact; no Borealis investigation or Client changes were needed.

## Preservation and authority

Before implementation, preservation records were written to
`%TEMP%/aurora-native9-20261031-160837/`: branch/full HEAD/history, index and
working patches, all worktrees, 3,168 tracked paths, untracked inventory, root
inventory, critical hashes, main-checkout status, release tags, production
executable identity and the original optimized NATIVE-8 executable. No original
native changes needed separating. The main checkout's pre-existing changes and
untracked developer files were left in place. Existing worktrees and evidence
were retained. Acceptance roots are disposable `aurora-native9-*` directories;
they are preserved, not recursively cleaned.

Read-only release inspection found immutable production release **v1.5.1**,
release ID **418145573**. The existing encoded public verification identity's
SHA-256 is **`bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`**,
matching that release's `release-assets.json`. Only public key material was
read. No private key, credential, authenticated launch command or token store
was read/exported. No remote write, push, tag, workflow dispatch, publication,
authority advancement, key rotation or production installation was performed.

Production executable preservation reference:
`%LOCALAPPDATA%/Aurora Launcher/aurora-launcher.exe`, SHA-256
`42c34fbab45ece1c88727374b52751067d5aeebb419a705f72f6e16dd6a2387c`.

## Implemented behavior

### Shared installation cancellation

`content_jobs` is one cooperative scope over the existing content/download/
installation services. Native jobs each own a fresh control, and retries create
fresh controls. Queued jobs cancel immediately; running jobs display
**Cancelling** while active writes or a transaction settle. The queue exposes
Queued, Downloading, Verifying, Installing, Cancelling, Cancelled, Completed and
Failed. No installation future is aborted/dropped, Launcher process killed, or
verified cache entry removed by cancellation.

Checkpoints stop new acquisition/retries and reject activation after cancellation.
An already active shared transfer finishes into the verified store; independent
consumers can reuse it. Cancellation does not promise instantaneous network
interruption. Commit guards serialize entry against cancellation and shield
existing atomic/rollback transactions, including nested provider operations.
A request arriving inside a successful commit reports actual completion.
Unfinished new pack records stay non-ready; existing-instance merges retain
their existing transactional rollback and ownership rules.

Tests cover cancellation before any network request, settled shared acquisition
with another successful consumer, independent controls, verification/activation
checkpoints, mod/resource/shader activation refusal, a noninterruptible commit,
cache-preserving retry and normal completion. The broader existing pack/merge
rollback suite passes. These tests do not constitute a separately instrumented
GUI cancellation run at every phase of every large real modpack.

### Pack options and local imports

Hosted Modrinth pack details expose optional file checkboxes. Selected paths are
frozen in plan order; changing a selection invalidates the review fingerprint,
requiring a new explicit review. New-instance and merge services stay shared.

The Windows COM picker is owned by the exact native window, permits only one
picker at a time, validates the owner PID/handle, and avoids adding selected
files to Recent Items. Skin PNG selection continues through its existing bounded
validator. Local content selection flows through Rust review and an expiring,
root/instance-scoped ticket, then the shared activity queue.

Supported local inputs are actual Fabric/Forge/NeoForge mod JARs, resource-pack
ZIPs and shader ZIPs. Validation checks archive contents as well as extensions,
entry paths, duplicates, symlinks, entry counts, decompressed sizes, data/CRC,
current loader/dependencies, protected mods, inventory changes and destination
collisions. Installation rechecks reviewed bytes, imports through the SHA-256
store, revalidates a private staging copy, then uses no-clobber activation.
Local bytes remain user-managed, with no fabricated Modrinth provenance.
Dismissed reviews release their in-memory tickets. Nothing silently overwrites
an existing file.

**Local `.mrpack` import is explicitly unsupported.** The authoritative workflow
requires hosted pack/provider identity and reconciliation. A local archive is
not assigned an invented provider identity. The picker does not advertise that
format, and an archive containing `modrinth.index.json` is rejected with a clear
message. Adding a separately reviewed local-pack domain workflow remains work.

Synthetic fixtures round-trip all three supported import types, reject stale
reviews/unsafe paths/fake extensions, preserve provider records and existing
files, and support cancellation/retry. Real native UI acceptance selected a
resource fixture, reviewed it, installed it, observed completion and its resource
inventory entry, then reviewed the duplicate and verified installation disabled.
Screenshot inspection found and repaired clipped review text. The sanitized
[conflict screenshot](native9/local-import-conflict-final.png) contains
only synthetic instance/content information. Owner media/screenshots are excluded.

### Media and accounts

Windows media remains event-driven. Fixes balance WinRT initialization, remove
partially registered handlers on failure, prevent late snapshots from repainting
a removed/re-enabled widget, begin cycling from the actual current session,
support separate play/pause capability when toggle is absent, and bound source
thumbnail dimensions. No master-volume or per-application-volume control is
offered; the widget explains that volume stays in the media app.

`native9_media_acceptance` publishes two unique, silent disposable Windows
sessions and addresses only those IDs. **Live system-session acceptance passed**:
appear/disappear, synthetic title/artist/artwork, play/pause, supported previous/
next with resulting metadata, unsupported-control flags and child cleanup.
This validates the Windows session transport; it is not a claim that every
widget control was driven end-to-end against every player. NATIVE-8 widget
addition/removal acceptance is retained. No universal Spotify/YouTube/Twitch
claim is made, and no owner's listening history enters Git evidence.

Native preview uses **`com.aurora.launcher.native-preview/account/`**, separate
from production credentials. Removing an account now requires an explicit native
confirmation. Core OAuth/PKCE, refresh rotation, entitlement and error handling
remain shared and covered by existing synthetic service tests. Existing account
metadata is read using the established schema. No real logout, credential
removal, profile-changing operation, refreshed owner identity or new authenticated
Play was performed. NATIVE-7/8's real authenticated lifecycle is historical
evidence, not a new NATIVE-9 account acceptance. Older native executables still
share the production credential namespace and retain that warning.

### Native distribution and updater

The detailed contract is [NATIVE_DISTRIBUTION.md](NATIVE_DISTRIBUTION.md).
Native preview has its own per-user install/data/uninstall/shortcut identities
and independent release ordering. The stable Tauri host keeps its identity and
update engine. Native cannot consume the WebView update manifest or package.

The native engine verifies a signed envelope **before interpreting its payload**,
using the existing public verification identity. Signed fields bind native product,
channel, renderer, platform, architecture, package type, version/minimum version,
source commit, fixed repository/tag/filename, size, SHA-256 and a separate installer
signature. Downloads are bounded; discovery denies redirects and artifacts permit
only the pinned HTTPS release/CDN path. The UI reviews, downloads/verifies, then
offers explicit **Exit and install**. Handoff reauthenticates the exact offer and
rehashes/reverifies staging. It requires the fixed installed preview location,
checks games/jobs, passes an exact owned PID to NSIS and exits only after spawn
success. No running executable is overwritten or process-name termination used.

Updates require an exact compile-time native-authority opt-in **and** the existing
public key. Final local distribution embeds the existing public key but leaves
native authority **disabled**. No native authority/release was created. Test
signatures use an isolated ephemeral test key and harmless bytes; private test
key material is not persisted or embedded. Verification tests accept the valid
fixture and reject separately signed incompatible identities, tampering, bad
hash/size/name, WebView JSON and discovery redirects.

Offline release-preparation tooling verifies signatures and writes payload/
envelope files without network publication or private keys. A separate manual CI
workflow prepares private candidate artifacts only. It has not been dispatched;
its environment/compiler provisioning remains unaccepted. Native protected
publication/authority-advancement automation is not yet implemented.

Standalone NSIS packaging does not retain Tauri. It installs x64 per-user with
icons/version metadata, Start Menu/optional desktop shortcuts and HKCU uninstall
registration. It fixes the preview path, guards redirected paths and ownership,
refuses downgrade/foreign shortcuts/in-use executables, serializes installers
and uninstallers, stages an executable and restores the previous one if activation
fails. Uninstall removes fixed owned files/target-proven shortcuts and preserves
user data and unrelated sentinel files. The package is **not Authenticode signed**.
Cryptographic updater signing and Windows publisher signing remain distinct.

There is no claim of a durable multi-file installer journal, universal power-loss
rollback, post-exit failure UI or accepted complete signed self-update/restart.
Interrupted transaction evidence is retained as a deliberate repair conflict.

## Data compatibility and rollback

No persisted registry/account/configuration/icon/widget/provider/pack/recovery/
runtime/asset-identity schema changes were introduced. Comparison with the
v1.5.1 production readers found those readers unchanged; the path override was
extended only for explicitly opted-in disposable acceptance builds.

A full synthetic WebView-format fixture included an installed-game record,
icon/customization, account metadata with no credential, settings/themes,
game-required Java and verified artifact/cache metadata, and world/mod/resource/
config sentinel files. Native
state loading and read-only validation preserved all original bytes. New
appearance preferences and an unknown native-media widget round-tripped;
the original source fixture remained byte-identical. Mock game bytes do not
establish playable Minecraft/runtime/authentication acceptance. An actual
installed managed-runtime migration was not newly exercised by this fixture;
its readers and path/identity rules remain unchanged.

This is disposable-copy validation, not a production registry migration or a
newly executed old-WebView downgrade. Safe preview rollback is to close/uninstall
preview and resume stable using its untouched data. Preview does not automatically
copy/share credentials or production data. A future stable replacement requires
an explicit backup/migration/rollback decision and acceptance of that exact path.

## Functional surface audit

“Retained” means implemented shared/native behavior with prior bounded acceptance;
it does not newly certify every UI action. No historical missing row is silently
promoted to complete solely because its core API exists.

| Area | Current implementation and evidence | Remaining parity / acceptance |
| --- | --- | --- |
| Home | Selection, Rust readiness/Play, widgets retained; disposable native boot exercised | New owner-attended authenticated Play/exit under final package |
| Instances | Create/select/rename/icon/configure/validate/repair/delete surfaces retained | Complete destructive lifecycle/repair/recovery UI matrix on disposable instances |
| Java | Exact managed install/reuse/validation retained, shared tests | Native custom-Java-path selection and complete UI parity |
| Minecraft/Fabric | Official metadata/loader compatibility and installations retained | Exact loader-version pin selector UI; additional real configurations |
| Forge/NeoForge | Existing normalized install services and automatic creation surface | Exact supported configuration acceptance; no universal support claim |
| Mods | Browse/review/install, list/filter, enable/disable/remove, protected/dependency checks; local import added | Content update/check/apply/bulk/policy native controls and complete acceptance |
| Resource packs | Browse/review/install/list/toggle/remove retained; real local import/conflict acceptance | Broader compatibility/update GUI matrix; activation in Minecraft |
| Shaders | Browse/review/install/list/remove retained; synthetic local import | Actual shader-player activation stays in game; wider UI acceptance |
| Modpacks | Hosted details/version/review/new instance/merge retained; optional files/cancellation added | Local `.mrpack`, managed pack update/recognition parity and phase-by-phase real cancellation matrix |
| Downloads | Shared verified cache/single-flight/retry/concurrency retained; cooperative job cancellation implemented | Large live-pack timing/responsiveness matrix; active transfers settle rather than abort |
| Accounts | Shared listing/select/sign-in/refresh/entitlement/error handling; preview isolation and removal confirmation | Owner-attended isolated sign-in/refresh/Play and destructive account-scope acceptance |
| Skins/capes | Native previews/import/library/equip controls retained, picker now owned | Authorized real upload/equip/remove acceptance; no profile changes performed |
| Appearance/navigation | NATIVE-8 cards/icons/tooltips/chrome/accent/themes/Borealis retained | No regression found; no repeat Borealis motion investigation |
| Media | Event-driven worker corrections; live synthetic Windows transport accepted | Full widget-driven matrix and named-player interoperability claims need actual evidence |
| Discord | Preferences/integration retained | Complete live presence/privacy interoperability matrix |
| Client updates | Existing native review/apply boundary retained | Real production Client offer on this historical branch; no unrelated Client work |
| Native updates | Signed native-only verification/staging/review/handoff implemented, disabled authority | Full signed download→exit→installer→restart/failure/repair acceptance and protected publication |
| Installer/data | Standalone preview packaging, disposable install/upgrade/uninstall/preservation tests | Final publisher signing decision, UI installer acceptance, supported production migration decision |
| Accessibility/platform | Existing accessible labels, keyboard controls, reduced motion retained | Complete keyboard/high-DPI/screen-reader matrix; non-Windows is not this candidate's acceptance |

## Verification and release gates

Verification covers the native host and ordinary Tauri compatibility. Full logs
are retained privately under `%TEMP%/aurora-native9-*.txt`; only sanitized results
and screenshots are source evidence. The final artifact identities and installer/
resource results are recorded in `docs/native-ui/native9/validation.json`.

| Check | Result |
| --- | --- |
| Shared core, native-preview | 1,032 passed, 31 explicitly ignored; 6 icon tests passed |
| Shared core, ordinary Tauri | 1,034 passed, 31 ignored; 6 icon tests passed; final all-target check passed |
| Shared core, host-neutral | 1,028 passed, 31 ignored; 6 icon tests passed |
| Final focused NATIVE-9 cases | 10 passed after ticket-scope/discard/target-review corrections |
| Native all-target tests | 31 main, 12 appearance, 4 NATIVE-8-driver, 4 provider-driver tests passed |
| Frontend | 301 tests passed; production build passed; Svelte check 0 errors / 0 warnings |
| Release tooling | 118 tests passed, including new native preparation tests; final native tests 2 passed |
| Rust format / patch whitespace | Passed |
| Live synthetic Windows media | Passed; only exact synthetic session IDs read/controlled |
| Native desktop | Real isolated boot, picker/review/import/completion/inventory/conflict/dismissal exercised |

The complete native-preview core suite was repeated after the final target label;
native tests preceded that display-only label, and its native build/real review
were repeated. Ordinary Tauri/host-neutral full tests preceded the review-ticket
discard addition; final Tauri all-target checking and focused core tests cover
the command change. Ignored tests retain their existing explicit live-network/
diagnostic opt-ins; ignored is not passed. Four existing Rust test warnings remain.

Production acceptance remains **open**, principally for full functional parity,
owner-attended account/profile scope, signed end-to-end self-update and failure/
repair acceptance, exact final package UI/upgrade behavior, migration/rollback
policy, CI validation and separately approved signing/publication/authority setup.
No Authenticode claim is made. Local implementation and a testable preview
package do not close those gates.

## Final continuation (Claude, 2026-10-31)

### Repository identity and recovered state

Codex's session ended after committing `1eb5ad3`. The continuation started from
that HEAD with a clean index; only this report and two conflict screenshots
were untracked. Codex's last optimized **acceptance** build had finished, but
its **distribution** build was still compiling when the session ended, so no
final installer package or `validation.json` existed. That build was allowed to
finish and its executable retained as a reference (SHA-256 `a44c03c9…`), then
superseded by the final builds below. Final commits are listed under
[Commits](#commits).

Preservation was rechecked at the end: tracked-file set unchanged (3,183
paths), no deletions or renames, the main checkout's HEAD and status identical
to the session start, production executable
SHA-256 still `42c34fba…2387c`, and no real preview installation or HKCU
uninstall registration. All installs used disposable `aurora-native9-*` roots.
Every acceptance registration created here was removed by its own uninstaller.

### Defects found and fixed

1. **Update handoff race (installer, real defect).** After spawning NSIS with
   `/UPDATEPID=<pid>`, the native UI quits immediately. Nothing else holds the
   launcher's handle, so the PID was often already gone when NSIS reached
   `OpenProcess`. NSIS then aborted silently ("could not be identified"): Aurora
   closed and no update happened. Codex's test used a 4-second sleeping fixture
   and never hit this. Reproduced 6/6 with a fixture mirroring the launcher
   (spawn, then exit, launched with no surviving handle); 2/2 delayed controls
   succeeded. Fix: NSIS validates the PID as a positive integer, and treats
   `ERROR_INVALID_PARAMETER` (no such process) as "already exited". Any other
   failure still aborts, and the sharing-violation in-use probe still runs
   afterwards. `test-installer.ps1` now covers a malformed PID and the
   fast-exit handoff via `native/tools/update-exit-fixture.rs`.
2. **Specific import refusals were hidden.** `ContentError::UnsupportedActionWith`
   carries app-authored reasons ("The selected file is not a valid ZIP/JAR
   archive.", the `.mrpack` refusal, inventory block reasons), but the command
   mapping replaced all of them with "This content entry cannot be changed
   here." The error code `unsupported_content_action` is unchanged; the message
   now carries the reason. These sentences contain no paths or remote bodies,
   and the inventory reasons were already sent to the UI. Unit-tested.
3. **Review label polish.** The import review showed the raw `resourcePack`
   identifier and "0.0 MiB" for small archives; it now shows "Resource pack"
   (Mod/Shader pack) and KiB below 1 MiB. Unit-tested.
4. **Updater upgrade rule consolidated.** `check`, `download` and `install`
   each compared versions inline; `check` compared `Option`s, where an
   unparsable installed version would make any offer look newer (unreachable
   today, since the version is compiled in). One `is_upgrade` helper now fails
   closed and is tested against equal, older, build-metadata, pre-release and
   unparsable versions, including a validly signed offer for the installed or
   an older version.

### Verified in this continuation

**Installer.** Verified the in-use probe against a *running* executable (it
gets `ERROR_SHARING_VIOLATION`), not only a file lock. The full acceptance
script passes with the final distribution payload: install, reinstall,
concurrent install/uninstall refused (exit 2), foreign shortcut, downgrade and
in-use refused, exact-PID wait (6.0 s against a 4 s owned process), malformed
PID refused, fast-exit handoff completes, uninstall removes only owned files
and target-proven shortcuts, user sentinel preserved, reinstall into the
preserved directory. The package lock is a package-scoped `Local\` mutex that
refuses immediately (no waits, so no deadlock). Release after success and after
handled failure is shown by successful runs that follow refused ones. Inspected
installs: the installed exe equals the payload, the x64 PE has an embedded
icon, and the HKCU entry has the expected name/version/publisher/
NoModify/NoRepair, removed on uninstall. The real distribution installer was
packaged and inspected but **not installed**.

**File picker and import (optimized acceptance build, disposable root).** At
the Win32 level, the `#32770` dialog is owned by the Launcher window and the
Launcher window is disabled while it is open. Clicks on Play and on the
Launcher close button were ignored. Minimizing the parent hides the dialog and
restoring returns the same dialog. A rapid double-click opens exactly one
dialog. Cancel changes nothing, and keyboard focus returns to Import file. A
second Launcher process did not take the dialog. Also verified: fake `.jar`
refused; a valid long-named resource pack reviewed (name wraps, Target label
kept, buttons clear), installed and listed; the duplicate refused with install
disabled, existing bytes unchanged.
Screenshots: [long-name review](native9/local-import-long-name-review.jpg)
and [completed inventory](native9/local-import-completed-inventory.jpg)
(captured before fix 3, so they still show the raw type label).

**Not captured:** minimum-size and higher-scale captures of the review dialog.
The desktop user became active in another Aurora window mid-pass, and the
interactive work was stopped rather than take over their mouse and keyboard.
The review text is in a height-bounded `ScrollView`
(`min(300px, max(120px, height − 240px))`), so overflow scrolls instead of
overlapping the buttons. This is code-reviewed, not captured.

**Updater.** Each of the 16 invalid fixtures is **separately and validly
signed**, so their rejection proves field validation, not just signature
failure: schema, product, channel, WebView runtime, platform, aarch64, MSI,
minimum version, build metadata, hash, zero/oversize, two URL variants,
source SHA and an unknown field. Tampered payload, WebView manifest JSON,
truncation, oversize, redirects and installer hash/size/filename/byte tampering
are rejected. The test key exists only under `#[cfg(test)]`. The production
key and authority are compile-time only, with no runtime override. The final
distribution build had **neither** variable set, so native updates are
unconfigured; this is stricter than Codex's note that its local build embedded
the public key. A full local signed download → exit → install → restart run is
**not possible without weakening the design**: discovery is pinned to the
fixed HTTPS authority with no test override. It therefore remains an
owner-environment gate. The handoff half is covered by the installer
acceptance above.

**Migration.** Codex's disposable WebView-format fixture test
(`native9_disposable_webview_data_copy_preserves_documents_and_user_sentinels`)
passes in all three core configurations. No persisted schema changed in this
continuation.

### Tests (final source)

| Command | Result |
| --- | --- |
| `cargo fmt … --check` (core, native) | exit 0 |
| `cargo test --manifest-path native/Cargo.toml --locked --all-targets` | 32 + 12 + 4 + 4 passed, 0 ignored |
| core `--no-default-features --features native-preview` | **1,034 passed**, 6 icon, 0 failed, 31 ignored |
| core ordinary Tauri | **1,036 passed**, 6 icon, 0 failed, 31 ignored |
| core `--no-default-features` (host-neutral) | **1,030 passed**, 6 icon, 0 failed, 31 ignored |
| `npm run check` / `npm test` | 0 errors, 0 warnings / 301 passed |
| `node --test tools/release/native-update.test.mjs` | 2 passed |
| `native/tools/test-installer.ps1` (final payload) | exit 0, all cases above |
| Release builds (distribution, acceptance) | exit 0 |

Ignored tests are existing explicit live-network/diagnostic opt-ins and are not
counted as passes. Counts exceed Codex's because of the new tests and Codex's
own later additions. Artifact hashes and installer results are recorded in
[validation.json](native9/validation.json): distribution exe
`5c693e75…fcdb9`, distribution installer `dc12abb9…7f51c` (not Authenticode
signed), acceptance exe `50c8e862…3d354`.

### Performance smoke

The copy of the NATIVE-8 populated disposable root (Borealis) used 1120×760
logical, the exact application HWND, a 25 s settle and 3 × ~10 s intervals.
CPU is % of **one** of 28 logical cores. The window held foreground in every
interval.

| Build | Borealis foreground CPU | WS / private MiB | Minimized CPU | WS / private MiB |
| --- | ---: | ---: | ---: | ---: |
| NATIVE-8 final (historical) | 0.78–3.11 | 43.5 / 16.7 | 0.00 | 38.3 / 11.4 |
| Codex HEAD, same run | 0.93–1.71 | 43.8 / 18.0 | 0.00 | 35.9 / 10.0 |
| **Final** | 1.09–2.33 | 44.5 / 18.4 | 0.00 | 36.5 / 10.3 |

Final versus HEAD under identical conditions differs by less than 1 MiB, which
is noise. The ~1.7 MiB private increase over NATIVE-8 is present in HEAD too,
so it comes from NATIVE-9 features, not this continuation. The RAM advantage
over the historical Tauri figures is preserved; no new comparison claim is
made. Not measured: unfocused-visible pause (the desktop was not quiescent),
page navigation, Downloads idle, and the media widget (enabling it would read
the owner's live media sessions). An earlier run that targeted
`Process.MainWindowHandle` was rejected, because winit's event-target window
can be returned instead of the app window.

### Observations, not verified defects

- One acceptance process (driven by the desktop-automation tool, then resized
  programmatically) hid its window on close but kept running; `window.run()`
  never returned. Seven controlled repeats on the final build (plain close,
  minimum, below-minimum, below-then-minimum, background focus requests) and
  both perf runs closed cleanly. Not reproduced, so it is not called a defect.
  Worth watching in owner testing.
- Programmatic `SetWindowPos` can size the window below its declared 720×520
  logical minimum. User drag-resizing was not tested here.
- During this session, an Explorer-launched process started Codex's old
  **pre-fix debug** build from `%TEMP%\aurora-native9-ui-…\bin`. It wrote to the
  real preview data root `%LOCALAPPDATA%\com.aurora.launcher.native-preview`
  and installed a "Vulkan Optimized" modpack there. This was not done by the
  agent and was left untouched. An earlier accidental launch of the same
  executable by the agent's desktop tool created a default config there. Only
  those exactly-proven new files were removed, before the later user launch.
  Three pre-existing `com.aurora.launcher.native-preview/account/` credential
  entries were not touched.

## Release readiness

**Completed technical gates:** installer safety, package lock, exact-PID and
fast-exit handoff, uninstall preservation; owned modal picker and reviewed
local import with conflict refusal; signed native-only offer verification and
channel/package/architecture isolation; strict upgrade ordering; data-copy
migration fixture; core, native, frontend and tooling tests; optimized builds;
no measured resource regression.

**Remaining local gaps (why this is PARTIAL):** the parity rows in the table
above that still say a native control is missing, chiefly native content
update/check/apply/bulk controls, custom Java path, the exact loader-version
selector, and local `.mrpack`. Also the review-dialog minimum-size/high-DPI
capture, and the unreproduced lingering-close observation.

**Owner-attended gates:** isolated real Microsoft sign-in/refresh/Play and
destructive account-scope tests in the preview credential namespace; real
skin/cape upload/equip; installing the real preview package on the owner's
machine.

**Protected / owner-authority gates:** a signed native build in the protected
environment (`AURORA_UPDATER_PUBKEY` plus the exact native authority);
Authenticode decision; a full signed download → exit → install → restart and
failure/repair acceptance with those exact bytes; validating and dispatching
`native-preview-candidate.yml`; native authority initialization and
protection; immutable native release publication; and any stable-replacement
migration/rollback decision. None was attempted.

### Commits

On `codex/native-ui-migration`, directly after `1eb5ad3`: one commit with the
installer, import-message, review-label and updater fixes plus their tests and
the distribution-contract update, then one commit with this acceptance record
and its sanitized evidence. Nothing was pushed, merged, tagged or published.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
