# Aurora native release readiness

2026-11-01, Windows x64. **READY FOR PROTECTED RELEASE ACCEPTANCE.**

Local signed update/install/restart, native packaging, automated desktop checks,
strict-reader compatibility and recovery verification pass. Protected production
signing/CI, public authority setup, exact signed installed-update acceptance and
the remaining physical desktop checks require the owner actions in section 14.
This is the existing **native-preview** release policy; no stable WebView-to-native
switch, publication or authority advancement was authorized or performed.

## 1. Source identities and candidate branch

| Identity | Full SHA |
| --- | --- |
| Starting complete native/Pandora candidate, `codex/pandora-backend` | `75c2c3baa056d7ce6038cb3ed4156aad196ecb52` |
| Final readiness implementation and harness | `8339d371d189df9f4716f085dcea32e754d41030` |
| Exact clean snapshot used for the final ordinary executable/installer | `66303fac86a1087bcb012561eddb0ff5b238a268` |
| Original native checkout, `codex/native-ui-migration`, unchanged | `0e2c8dd9e13ea1b1d548505d575674d9114ee386` |
| Original WebView checkout, `codex/spxctra-ui-refinement`, unchanged | `9b99db3ac226854c8ae96a86eed4c8d3084defa7` |

The candidate is `C:\Users\kalib\.codex\worktrees\pandora-backend\aurora-launcher`.
The final report/evidence is committed subsequently; its commit is returned in
chat. The two implementation SHAs above are immutable acceptance references.
The second commit changes release tooling and acceptance-only code, not the
ordinary native UI/core represented by the clean packaged snapshot.

## 2. NATIVE-10 and PANDORA-4 integration

The Pandora branch descends directly from the original native HEAD; its merge
base with that checkout is `0e2c8dd9e13ea1b1d548505d575674d9114ee386`.
It already contains NATIVE-10 custom Java, PANDORA-4 implementation
`2f4fe1e9c1e7a3a712e5113ca77d9332a787109a`, documentation
`40af1244ca31536aec3ab35d4ad6f44af1e1cc0f`, and later native fixes.
No merge or cherry-pick was necessary or performed. Earlier owner-accepted
authentication, gameplay, content functionality and Pandora architecture were
not reopened as feature investigations.

Before release, review/integrate the complete intended branch onto the approved
release ref. Do not cherry-pick PANDORA-4 alone without its native/backend parents.
Updating the original native branch would be a separately authorized operation;
the main release integration also remains separate. The prepared workflow requires
the exact reviewed current `main` SHA and cannot release this local branch by
assuming it was merged.

## 3. Preservation audit

Baseline: `%TEMP%\aurora-final-readiness-20261101-182740\` records all worktrees,
full HEAD/history, branch/status, staged/unstaged patches, tracked/untracked lists,
root inventories, critical hashes and hashes of pre-existing owner changes.
The candidate started with 3,285 tracked files. Its pre-existing modified
`AGENTS.md`, `native/src/layout_acceptance.rs`, `native/ui/widgets.slint`, responsive
acceptance document and screenshot directory were preserved byte-for-byte and
left unstaged. The final ordinary build uses a clean Git archive to exclude that
uncommitted work. Local signed fixtures used the working source, including that
preserved presentation patch; they are explicitly test configurations.

The original WebView modifications/untracked files and all other checkouts were
preserved. No tracked baseline file disappeared; critical documentation,
manifests, both Rust lockfiles, npm metadata and native configuration remain.
No dependency was added. Build/game/runtime data and signing material were not
staged. No `git clean`, `cargo clean`, reset, broad cleanup, push, merge, tag,
workflow dispatch or remote write occurred. No Client source, owner credential,
production instance or world was changed.

Production executable, before and after:
`%LOCALAPPDATA%\Aurora Launcher\aurora-launcher.exe`, version 1.5.1,
SHA-256 `42c34fbab45ece1c88727374b52751067d5aeebb419a705f72f6e16dd6a2387c`.
The normal native preview installation path was not used for acceptance.
Temporary artifact/data/evidence copies were retained; their package registrations
were removed by their exact owned uninstallers after verification.

## 4. Signed local update acceptance

**PASS, isolated test identity.** `native/tools/test-signed-update.ps1` built
optimized acceptance versions 1.4.2 and 1.4.3, using a random compiled package/data/
credential identity, fixed temporary installation, fixed loopback origin and
separate ephemeral Ed25519 test key. The checked-in harness now uses parallel
ThinLTO for these two fixtures; the ordinary distribution retains its original
full-LTO release profile. The private test key stayed in the helper's memory.
No test key or acceptance flag appears in the final ordinary binary, and the
packager rejected the acceptance executable as a distribution artifact.

Actual path exercised: native discovery, signed-envelope verification, strict
native offer validation, real HTTP download, SHA-256 and installer signature,
private staging, offer reauthentication and retained-file reverification,
exact-parent PID handoff, NSIS replacement, restart, and Rust launcher-state load.
The acceptance driver invokes the real typed APIs, not an alternate installer or
mock verification engine. Accounts, credentials, sync and Discord are not loaded
by automatic acceptance. Ready here means the updated launcher loaded its state;
it is not a new gameplay/authentication claim.

| Observation | Result |
| --- | --- |
| Versions | 1.4.2 → 1.4.3; executable resources and restarted Rust version agree |
| Transfer | 10,615,020 bytes; 31 monotonically increasing progress events; final count equals installer size |
| Source/restarted PID | 16700 → 33576; exact installed path recorded |
| Source executable SHA-256 | `7438d338c745d4fe94c6c4fc4c16ae15a1d81dde2f5c9981a869a3f5a9937b7d` |
| Updated executable SHA-256 | `d7ac751c27a1383cf102afb47d074b0821577a63f63414a1ca4c6a5dc710b75f` |
| Signed test installer SHA-256 | `87cbd9f2d2138633614ebde32afaa419e8f9191d90961252cab239aaa44bf502` |
| World sentinel before/after | `a189f21a35ab84caa8bdbaeda10bca3f76dc55e6e970f3bcc07a08f5295c8579` |
| Restart | Correct new executable bytes, version 1.4.3, `stateLoaded: true` |

Successful run evidence:
`%TEMP%\aurora-native9-signed-e2e-72c9c501799e4817bdb38b2443dfb6f4\`.
Its discovery envelope, public test configuration, progress, installed state,
restart receipt, hashes and results are retained. No production endpoint or key
was substituted. Production signed-update acceptance remains open.

An initial fixture attempt failed before discovery after a diagnostic side effect
exited the temporary signing helper. It was not counted as a pass. The old
installed executable and world sentinel remained unchanged. The final run used
a fresh identity, the bounded 90-minute helper and no diagnostic attachment.

## 5. Negative cases and handoff correction

| Cases | Evidence |
| --- | --- |
| Invalid/tampered signature, unknown key, bad hash/size/name, architecture/platform/package/channel/runtime/product mismatch, WebView JSON | Re-executed Rust native verification and release-tool tests; signed incompatible fixtures are rejected |
| Same version, downgrade, build/prerelease version confusion, unsupported minimum | Re-executed strict upgrade and payload/authority tests |
| Corrupt, truncated/interrupted or missing download | New real loopback transfer test; only the failing operation's staging is removed; prior verified data/sentinel survive |
| On-disk interrupted download staging | Cannot manufacture process-local pending install authority |
| Running executable replacement / failed installer preflight | Actual accepted installer refused an exclusive file lock, exit 2; installed hash and world preserved |
| Conflicting package mutex | Actual installer refused exact package mutex, exit 2; installed hash and world preserved |
| Interrupted NSIS staging | Actual installer refused `native-new.exe`, exit 3; evidence, installed executable and world preserved |
| Parent still running | Successful local signed handoff waits for the exact source child before replacement |
| Parent already exited | NATIVE-9's accepted rapid-exit fixture and unchanged NSIS `ERROR_INVALID_PARAMETER` handling retained; no installer source change |

`native/tools/test-update-recovery.ps1` also verified the random installed package
identity, version 1.4.3 and exact HKCU uninstall location. It cleans only its own
positively created staging sentinel. Earlier NATIVE-9 installer ownership,
shortcut, downgrade and activation-restoration evidence is reused, not counted
as newly repeated tests. No universal power-loss/multi-file journal guarantee is
introduced; interrupted transaction files remain a reviewed repair conflict.

## 6. Native channel isolation and trust

Native keeps `com.aurora.launcher.native-preview`, `native-preview`, Slint,
Windows/x86_64, `nsis-per-user`, `native-v<version>` and the fixed native filename.
Its signed schema binds those identities plus version/minimum, source SHA,
repository URL, size, SHA-256 and separate installer signature. WebView retains
its existing identity, updater, `v<version>` namespace and authority.
Native prereleases use `make_latest=false`; they do not silently become the
legacy launcher's latest stable release. Native version ordering is independent
of the installed WebView 1.5.1.

Existing trimmed encoded public-key SHA-256 remains
`bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`.
The WebView authority was read at
`df7e824b2e49f8a5b9d2447ca2314bcafa21d119`; it was never written.
The native authority ref is absent. The ordinary local artifact remains honestly
unconfigured: neither owner public key nor native authority opt-in was supplied
to that build. Only the protected workflow supplies both for release acceptance.

## 7. Signing configuration

Updater signing is implemented using the existing protected minisign identity.
The CI build checks the established public-key fingerprint and verifies both
signatures and the final byte inventory. Private signing values were neither
requested nor read/exported by this task.

**Windows Authenticode is not configured.** The final executable/installer are
`NotSigned`; updater signatures do not establish Windows publisher identity.
The owner must accept that distribution policy or provision an authorized
certificate/signing process. Sign executable before packaging and installer
before updater signatures/hashes if publisher signing is added.

## 8. CI readiness

`.github/workflows/native-preview-release.yml` is prepared locally and validated
with actionlint 1.7.12. It reuses the existing Windows/NSIS, protected signing,
GitHub draft/immutable publication and dedicated authority App process. No CI run
was dispatched. Read-only GitHub inspection found only the existing WebView
environments, not the three new native environments.

Guardrails: manual trigger; reviewed current main SHA; synchronized version and
native PE identity; explicit secrets; signing and missing-artifact failure;
strict signed manifest/hash inventory; protected publication defaulting off;
atomic native-tag creation and commit verification; no tag overwrite; private
draft-byte verification; immutable public-byte verification; explicit authority
parent; non-forced advancement of only the native ref last; dedicated App token
without ordinary-token fallback; bounded CDN reads and deliberate uncertain-write
recovery. WebView workflows/authority are unchanged.

Exact protected setup, inputs, bootstrap, acceptance and repair procedure:
[PROTECTED_NATIVE_RELEASE.md](../../native-ui/PROTECTED_NATIVE_RELEASE.md).

## 9. Final ordinary artifact identity

Built from clean tracked source `66303fac86a1087bcb012561eddb0ff5b238a268`,
excluding owner uncommitted files, with the unchanged optimized native release
profile. Both artifacts are retained under the baseline audit root.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `final-aurora-native.exe`, product Native Preview 1.4.2 | 34,137,088 | `0c998b5a3759de334ebadbbca0135d2c1778f21f201b7bb90519659ad4ba8bfa` |
| `final-package/Aurora.Native.Preview_1.4.2_x64-setup.exe` | 10,241,805 | `83cf2cae8695aa955fa9b844f72c20932591cc503d396b55e1cf122b447db805` |

The packager verified x86_64 PE payload identity, product/version metadata,
ordinary feature guards and no output overwrite. Binary audit confirmed both
test public keys, diagnostic-root override and desktop acceptance flag absent.
This is local packaging evidence; protected release bytes must be rebuilt and
accepted from their exact authorized source/run.

## 10. Desktop, DPI and accessibility

Fixed one reproduced blocker: the old shared dialog let Tab reach and activate
covered background controls after three steps. Shared dialogs now use Slint's
real popup focus scope, keeping existing panel/scrim geometry and tokens. Tests
prove forward/reverse traversal containment, reachable dialog action, Escape,
focus restoration and initial confirmation-field typing. Requested field focus
uses a one-shot activation callback, with no persistent polling or decoration.

| Check | Result / limit |
| --- | --- |
| Complete real component layouts | Passed at 720×520, 1120×760 and 1600×1000; minimum-size 100/125/150/200% software-renderer scales |
| Shared dialog focus/DPI render | Passed at 100/125/150/200%; inspected retained images; focus visible and panel present |
| Keyboard/modal regression | Passed both traversal directions over 23 positions, Escape and prior-focus restoration; confirmation text receives initial typing |
| Reduced Motion | Existing real software-frame tests passed: busy indicators stop animating under the in-app setting; Borealis requests no idle frames |
| Real optimized native desktop | Fresh disposable Home observed at minimum size with process 125% override; UIA exposed navigation, caption, sign-in and Create names |
| Further physical input | **Stopped by the owner's physical Escape key. No further Computer Use/input was attempted.** |
| Remaining physical checks | Final modified-dialog keyboard behavior, typical-size interaction, real 150/200%/per-monitor DPI, caption/Snap, picker focus restoration, complete scrolling and Narrator speech |
| High contrast / OS animation policy | Custom theme does not provide certified automatic Windows high-contrast integration; OS preference interaction not newly verified |

Headless rendering is not physical desktop acceptance, and a UIA tree is not
screen-reader certification. No WCAG/full Narrator claim is made. Prior native
picker acceptance is retained; the final suite's five picker/DTO tests are
automated checks, not a new physical picker run. No runtime performance benchmark
or speed claim was added. The preserved owner widget refinements were not redesigned.

Evidence: [dialog 125%](../../native-ui/release-readiness/modal-focus-125.png),
[dialog 200%](../../native-ui/release-readiness/modal-focus-200.png), and
[earlier disposable Home](../../native-ui/release-readiness/desktop-before-min125.png).
The dialog images are headless; the Home image precedes the modality correction.

## 11. Migration compatibility

Fresh first-run state and native preview 1.4.2→1.4.3 upgrade were exercised only
in temporary roots. Existing WebView-format data was copied, not moved or shared
with production. Current-core tests preserve registry/Minecraft/loader records,
custom Java, installed game/Aurora/cache references, icons, appearance/widgets,
sync policy/overrides and unknown future history, plus mod/config/resource/shader/
server/world/user sentinels. Existing pack and synchronization/NBT regression
fixtures cover their structured metadata and conflict history. Sentinel archive
bytes are preservation fixtures, not claims of playable installed content.

The source fixture and independent backup remain byte-identical after current
loading, restart and unrelated atomic writes. The actual old core at
`1d7c30738569b29fd6a32f85352aadce4ea69813` was archived into a separate temporary
source tree and executed with the same bounded probe against a copied fixture:

| Reader | Registry incl. custom Java | Widget-grid config | PANDORA-4 policy | Refused policy write / data preserved |
| --- | --- | --- | --- | --- |
| Pre-PANDORA-4 actual core | Accepts | Rejects | Rejects | Refused; all pre-existing bytes preserved |
| Current core | Accepts | Accepts | Accepts | Read-only probe preserved all bytes |

The old refused write adds only its existing empty `.sync-settings.lock`.
NATIVE-10's independently accepted actual older WebView test already proves its
reader rejects `java` without rewriting it; that investigation was not repeated.

## 12. Rollback and recovery

Coexistence is the supported WebView path: close native preview and resume the
untouched legacy installation/data. No automatic stable migration is enabled.
For an older native reader, first preserve the complete newer tree and a verified
compatible backup. Custom Java must be reset to managed with a supporting launcher
before an older WebView reads that registry. Restore compatible full configuration
for pre-grid widget readers. Merely disabling sync does not remove serialized
`overrides`; restore a known compatible sync-policy backup and retain newer
history/backups separately. No JSON fields are silently stripped or schemas lowered.

NSIS intentionally rejects downgrade. A reviewed manual compatible reinstall/data
restore is required, or a higher-version forward repair. Do not lower public
authority to roll back. Preserve and inspect `.new`/`.previous` evidence; do not
wipe an unknown installation. The fresh/upgrade/refused-update fixtures verify
data preservation, not universal crash/power-loss recovery.

The documented manual route was also executed on the successful disposable
package: uninstall 1.4.3, install its compatible 1.4.2 baseline, verify the restored
source executable hash and all four existing managed-data file hashes, then
uninstall that temporary package. Every operation exited 0; the world/config/cache
bytes and artifact copies remained. This fixture has compatible defaults; it does
not bypass the newer optional-field boundaries above. The failed first fixture's
temporary package registration was also removed, preserving its world and source
copy. No post-Escape desktop window was controlled or terminated.

## 13. Commands and regression results

Full logs are retained in the audit directory; concise machine-readable results
are in [validation.json](../../native-ui/release-readiness/validation.json).

| Command / check | Actual result |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features --features native-preview` | 1,112 core + 6 icon tests passed; 31 existing ignored, not passes |
| `cargo test --manifest-path <clean snapshot>/native/Cargo.toml --locked --target-dir native/target` | 52 native + 5 picker/DTO tests passed; 0 ignored |
| `cargo build --manifest-path <clean snapshot>/native/Cargo.toml --locked --release --target-dir native/target` | Passed, ordinary optimized profile |
| `package-windows.ps1 -Binary <final ordinary exe> -OutputDirectory <new temp output>` | Passed; NSIS package/hash/metadata verified |
| `native/tools/test-signed-update.ps1` | Final fresh-identity run passed; first failed helper attempt excluded |
| `native/tools/test-update-recovery.ps1 -Root <successful fixture>` | Passed actual executable lock, package mutex and interrupted staging refusals; identities/data preserved |
| Exact temporary uninstall → compatible 1.4.2 reinstall → uninstall | Passed; source hash restored, four existing managed-data hashes preserved; both temporary registrations cleaned |
| `cargo run ... --example native_release_data_probe -- <marked fixture> old/current` | Actual archived old and current readers produced expected compatibility results and preservation |
| `cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets` | Passed with existing warnings; added probe also compiled/executed host-neutral |
| Core/native `cargo fmt ... --all -- --check`, `git diff --check` | Passed |
| `npm run check` | 0 errors, 0 warnings |
| `node --test tools/release/*.test.mjs` | 123 passed; 0 skipped/ignored/failed |
| `actionlint` on native candidate/protected workflows | Passed; 1.7.12 download checksum verified |
| PowerShell parser on packaging/signed/recovery scripts | Passed; signed/recovery scripts also executed |
| Artifact/hash/feature audit | Passed; ordinary and test artifacts distinguished |

No live Modrinth/Minecraft benchmark, new login, gameplay repetition, protected
workflow or publication was run. Existing Rust warnings remain; ignored live/
diagnostic tests are explicitly excluded from the pass count.

## 14. Exact remaining owner actions

1. Review the local commits and integrate the chosen complete candidate onto the
   approved release ref. Separately decide the preserved uncommitted widget work;
   it is not part of the clean packaged snapshot. Choose/synchronize native release
   versions without assuming a main integration or tag exists.
2. Configure reviewer-gated `launcher-native-preview-signing`,
   `launcher-native-preview-publication`, and
   `launcher-native-update-authority-production`, their exact-main deployment
   policies, established public key and owner-held signing/App inputs. Apply the
   native ref's layered history/writer/creation protections, then separately
   authorize its exact empty-root initialization. Retain WebView protections.
3. Decide unsigned Windows publisher distribution versus real Authenticode setup.
   If signing is added, rederive all updater signatures/hashes after signing.
4. Explicitly authorize the protected run. Accept its exact signed artifact in a
   disposable Windows profile/VM, including the remaining physical desktop checks.
   Bootstrap an authority-enabled older native baseline there if necessary; an
   old unconfigured preview cannot gain update authority at runtime.
5. Only with separate explicit approval, publish immutable native bytes and
   advance native discovery last. Complete the real strictly-newer installed
   update under the established production signing identity. Local test-key
   evidence does not replace this production acceptance.

## 15. Verdict

**READY FOR PROTECTED RELEASE ACCEPTANCE.** The independent local engineering
and automated verification are complete. Production release authorization,
protected signing/CI execution, final physical desktop acceptance and installed
production-trust update validation remain open. No public release or stable
installation replacement is implied.

## 16. Local commits

- `66303fac86a1087bcb012561eddb0ff5b238a268` — protected native pipeline, isolated
  signed lifecycle, shared modal correction and migration/transfer tests.
- `8339d371d189df9f4716f085dcea32e754d41030` — exact native tag provenance,
  acceptance diagnostics/efficiency and focused installer recovery.
- A subsequent report/evidence-only commit records this acceptance; its full SHA
  is returned in chat. No push, merge, tag, dispatch or publication occurred.
