# Aurora Launcher 1.1.0 release acceptance record

Date: 2026-09-27. No release is accepted or published by this audit.

Current status: the aggregate 39-commit audit and deterministic release gates
passed. The historical client/version blockers below were resolved by the
recorded 2.1.3 preparation. The owner subsequently attested the remaining manual
installer and game gates; see the appended record for its evidentiary limits.

## Mandatory stop conditions

1. Aurora Client source still declares `mod_version=2.1.2` in
   `C:/Dev/Aurora-Client/gradle.properties`. The bridge-enabled build cannot be
   published under the existing immutable `v2.1.2` identity. The requested
   instructions explicitly require stopping and reporting the required increment.
2. `.github/workflows/launcher-release.yml` does not supply
   `AURORA_DISCORD_APPLICATION_ID`. `src-tauri/src/discord.rs` consumes that value
   at compile time through `option_env!`. The official workflow therefore does
   not establish the required production Discord configuration.

No push, tag, workflow dispatch, client publication, launcher publication, version
mutation, or production distribution mutation was performed. This is an early
gate report, not a completed aggregate security audit.

## Git and safety baseline

Both origins were fetched without changing working files. The resulting state
matches the requested baseline:

| Repository | Local HEAD | Remote branch and HEAD | Ahead / behind | Tracked | Pre-existing untracked |
| --- | --- | --- | --- | ---: | ---: |
| Launcher | `77c93c30327301b491cef8fe82b2e1c1e5c8248d` | `origin/main`: `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf` | 33 / 0 | 183 | 607 |
| Client | `a1f0ab6d35fab2427a82b06be6128d7295b630d8` | `origin/master`: `8ff9cf45be8a8c3dfa6c3a428a841d0af837760c` | 3 / 0 | 374 | 0 |

Each merge-base equals its remote branch HEAD. SHA-256 and byte-size inventories
of protected untracked files and tracked-file path inventories were recorded
outside the repositories in the current user's temporary directory as
`aurora-launcher-release-audit-baseline-20260927.json` and
`Aurora-Client-release-audit-baseline-20260927.json`.

The requested launcher audit range is
`1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf..77c93c30327301b491cef8fe82b2e1c1e5c8248d`.
Its aggregate name-status contains additions and modifications, no deletions.
Every-commit review, full documentation review, migration review, and domain
security review remain outstanding; none is claimed to have passed.

## Exact release preparation required

Client conventions use `v<version>` and `aurora-<version>.jar`. The remote has
`v2.1.2`; the targeted remote lookup returned no `v2.1.3` tag. Version `2.1.3`
is an appropriate proposed next patch identity, subject to the release collision
verifier checking GitHub releases as well as tags before publication.

The authoritative source change is `mod_version=2.1.3` in `gradle.properties`.
`build.gradle` derives `project.version` from that property and expands the
`${version}` placeholder in `src/client/resources/fabric.mod.json`. Rebuild and
inspect the resulting `aurora-2.1.3.jar`; never rename the old JAR or reuse its
hash/size. Run the client test/build and release-verifier gates, then use the
existing manually dispatched, `aurora-production`-gated publication mechanism.
No proposed 2.1.3 artifact hash, size, release commit, or public URL is verified.

Launcher version preparation requires synchronized `1.1.0` values in
`package.json`, both root version fields in `package-lock.json`, the package
version in `src-tauri/Cargo.toml`, and the launcher package entry in
`src-tauri/Cargo.lock`. Tauri already derives its version from `../package.json`.
The current version contract remains `1.0.0`; there is no accepted 1.1.0 build.
Historical acceptance documents must retain their historical versions.

Supply public ID `1553653987545317396` as `AURORA_DISCORD_APPLICATION_ID` in the
official launcher build job before Rust compilation. Retain `aurora-logo`, which
is currently selected in the native presence projection. No OAuth, bot, or secret
configuration is required. Production executable and installer acceptance must
prove the resulting configured build.

Only a separately verified immutable public client artifact may subsequently
enter the production distribution manifest through its existing validation path.

## Current distribution and artifact evidence

The unchanged launcher production manifest selects Aurora `2.1.2`, Minecraft
`1.21.11`, Fabric Loader `0.19.5`, Fabric API `0.141.6+1.21.11`, Java `21`.
Its Aurora URL is
`https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/v2.1.2/aurora-2.1.2.jar`,
with expected SHA-256
`55ac97f7494daa3866bb3b4aa8d23e49b240fe5ced00fbf1742f7214fc77c52a`
and size `2450086`. These are manifest values, not a fresh public verification.

The local bridge build remains `build/libs/aurora-2.1.2.jar`, size `2467058`.
Its freshly computed SHA-256 is
`eb2b06bc3955881ee9ff0dc561c25ced617de822a276c61fa3a9f79343602777`,
matching the supplied historical local-build identity.
It is not a publishable new release identity. Client build properties target
Minecraft `1.21.11`, Loader `0.19.2`, and Fabric API `0.141.4+1.21.11`; embedded
source metadata requires Java `>=21`. Production compatibility with the launcher's
newer pinned Loader/API still requires final acceptance.

## Verification and limitations

The launcher version verifier passed for existing `1.0.0`; `git diff --check`
passed before creating this report. Full Rust/frontend/client regression suites,
production builds, MSI/NSIS inspection and installation, executable boot, clean
production instance creation, game launch/main menu/normal exit, and public artifact
verification were not rerun after this early stop. No final test counts are claimed.
The supplied historical counts are not final release-candidate results.

Previously documented Discord application recognition, connection/reconnection,
launcher presence/logo, and master clear/restore evidence remains historical
evidence. Live Discord gameplay world/server privacy rendering remains unaccepted.
The client LF/CRLF source-contract qualification remains; this report does not
claim an unqualified Windows source-contract pass.

Other retained limitations include arbitrary cross-mod interactions, limited
dependency-transition solving, deferred cross-process writer exclusion and durable
crash journaling, and conservative blocking of differing root/nested versions.
Their security impact has not been reassessed in a completed aggregate audit here.

No release notes were published. Release commits, public 1.1.0 tag/release,
installer identities, updater metadata changes, and post-publication results are
not applicable. The only intended repository change is this uncommitted report.

Final safety verification: all 183 launcher and 374 client baseline tracked paths
remain; tracked worktrees have no modifications or deletions. All 607 protected
launcher untracked files retain their original sizes and SHA-256 values. The client
remains clean; the launcher adds only this report to its pre-existing untracked
inventory. Final `git diff --check` passed in both repositories.

## Resumed candidate preparation — 2026-09-27

The sections above remain the historical first stopped gate. This resumed task
prepared the candidate locally but **did not accept or publish the release**.

### Public client verification

Client HEAD, origin/master and `v2.1.3` resolve to
`e978d0b5545e4991137e0fc1387e9897e752dfc7`. The GitHub release API identifies
`v2.1.3` as public, immutable, and targeted at that exact commit. Source changes
from reviewed bridge HEAD `a1f0ab6` are the version property and release notes;
the bridge source has no changes in that range.

Independently downloaded public artifact:

- Release: https://github.com/kalibsolomon-pixel/Aurora-Client/releases/tag/v2.1.3
- Artifact: https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/v2.1.3/aurora-2.1.3.jar
- Filename: `aurora-2.1.3.jar`
- Size: **2,467,058 bytes**
- SHA-256: `4bf78dc1ef8f18e124377575c508ca357327be1c230b203e9f8181bdcb9ebc81`

The downloaded hash matches GitHub's asset digest. Its embedded mod version is
2.1.3, client environment, Minecraft `~1.21.11`, Java `>=21`, Fabric Loader
`>=0.16.0`, Fabric API `*`, Mod Menu suggested. All eight bridge class entries
remain. No duplicate archive entries, nested JARs, Discord SDK, acceptance
receiver, or added test fixture was found. Entry names match the reviewed local
2.1.2 bridge artifact exactly; every entry is byte-identical except root
`fabric.mod.json`, which contains the version increment. The pre-existing
`BlurTestScreen` classes remain, unrelated to bridge acceptance. No new code or
secret was introduced. Client files were not modified by this task.

### Local release commits

- `6b52c73`: public Discord ID in the official release build job, exact-value
  assertion before compilation, and setup documentation.
- `4376d78`: exact verified 2.1.3 production entry, preserved 2.1.2 entry,
  distribution contract test, and bridge artifact allowlist pin.
- `dd73fb9`: synchronized launcher 1.1.0 package/npm-lock/Cargo versions.
- `d77428e`: 2.1.3 first in the manifest and a creation-selection regression test.
- `d25d6c4`: existing required-mod fixture version aligned with first production
  entry. After reordering, that fixture failed because it still constructed 2.1.2
  installed state while mutating the first 2.1.3 manifest entry; no production
  behavior was changed to conceal that failure.

The new creation test proves 2.1.3 selection and its bridge digest, while exact
2.1.2 resolution remains available. Current production combination: Minecraft
1.21.11 / Fabric Loader 0.19.5 / Fabric API 0.141.6+1.21.11 / Aurora 2.1.3 /
managed Java requirement 21. Existing instance pins are not upgraded silently.

### Official build wiring and verification

The sole production compilation path is the `build` job of
`.github/workflows/launcher-release.yml`. Job-level public environment
`AURORA_DISCORD_APPLICATION_ID=1553653987545317396` reaches both Rust tests and
`npm run tauri build`; its exact-value assertion precedes compilation. The
publish job transfers and verifies installer bytes without rebuilding. YAML
was parsed with PyYAML installed outside the repository, and scope/build-path
assertions passed. Ordinary developer builds may still omit the variable.
The native asset remains `aurora-logo`.

Final deterministic suite after fixture correction:

| Check | Result |
| --- | --- |
| Rust library tests | 538 passed, 20 ignored, 0 failed |
| Rust icon integration tests | 6 passed, 0 failed |
| Rust total | **544 passed, 20 ignored, 0 failed** |
| Frontend | **116 passed, 0 failed** |
| Svelte/TypeScript | 0 errors, 0 warnings |
| Rust formatting | passed |
| All-target Rust check | passed |
| Frontend production build | passed |
| Version contract | 1.1.0 |
| Configured Tauri production build | passed; MSI and NSIS emitted |

Client tests were not rerun; published byte comparison proves the bridge class
bytes match the previously accepted build. Its documented Windows LF/CRLF test
qualification remains. No fresh client test-count claim is made.

Final artifact metadata and hashes:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| aurora-launcher.exe | 10169344 | `caaad5ae53b1ecb9e143227fd6dddf4e72630fe565a1fbec6645da5e6804d610` |
| Aurora Launcher_1.1.0_x64_en-US.msi | 5771264 | `4d937f2bfc872dd223112b08bf8de16d79b9003586da77a314cee56692e25050` |
| Aurora Launcher_1.1.0_x64-setup.exe | 4212407 | `f635a5376d293735bdbf2bc8109cf6c24612a795a813d3186815ffd20848a2de` |

Executable and NSIS PE product metadata say Aurora Launcher / 1.1.0. MSI
Windows Installer properties say ProductName Aurora Launcher, ProductVersion
1.1.0, Manufacturer aurora. Icons passed the six integration invariants. The
exact final executable contains both the production Discord public ID and 2.1.3
bridge digest, stayed alive during the boot smoke, and created a real window.
It was closed normally. Settings UI recognition was not visually accepted.
Installer installation/uninstallation acceptance was not performed.

### Real production acquisition through the native lifecycle

Explicit opt-in `benchmark_live_production_instances` passed separately:
1 passed, 0 failed, 91.24 seconds. It invoked the normal native creation and
installation pipeline with production URLs in a uniquely created disposable
temporary root; no manual JAR substitution occurred.

Root:
`C:/Users/kalib/AppData/Local/Temp/aurora-production-bench-4ee13401-4599-4ae3-8d70-3b26f2e12f18`.
Two Ready instances were persisted (`58a38297bcd7463bbadab72c969f3745` and
`08de552161d24dbbb43db4ab776ddb36`). Cold install, warm reinstall, second
creation and deep read-only Ready validation passed. Both registry pins select
2.1.3. The first installed Aurora JAR was independently hashed and matches the
public 2.1.3 artifact exactly; Fabric API also matches its manifest hash and
2,426,039-byte size. This is native integration evidence, not GUI acceptance
through an installed launcher.

### Remaining mandatory gates / stopped verdict

No launcher push, tag, release dispatch, or publication occurred. The remote
main base remained `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf` after refetch.

The full per-commit/aggregate audit of the unpublished range remains incomplete.
Focused inspection covered configuration schema 1/2/3 migration into 4, false
privacy defaults and malformed/future rejection; serialized final launch
validation; link/reparse rejection in deletion/mod boundaries; bounded bridge
authentication/frames/sequence replacement and clearing; and public release
build wiring. Those focused checks and passing tests do not constitute a final
security sign-off for filesystem, downloads, auth, provider lifecycle,
compatibility, diagnostics, or Discord/privacy across the whole range.

Also outstanding: disposable installer installation, exact Settings UI setup
recognition, creation through the installed production launcher, authenticated
Play -> Starting -> Running -> Aurora main menu -> normal Quit -> exit 0 ->
Ready. The GUI game sequence was not performed. Publication cannot proceed
without it. Computer-use guidance requires `node_repl` for native UI actions;
that runtime was not exposed in this session, and the browser automation tool
states native computer APIs are disabled. No UI acceptance was fabricated.

Prior generic Discord presence/logo/master clear/restore evidence is retained.
Live gameplay world/server/address privacy, reconnect and stale-state clearing
were not newly accepted. Other deferred limitations from the original report
remain. Draft notes are in `RELEASE_1_1_0_NOTES.md`; no notes were published.

This report is intentionally committed as repository acceptance documentation,
consistent with the existing tracked phase acceptance reports. Original stopped
history is preserved above. Generated binaries, installer artifacts, screenshots,
and protected diagnostics are excluded from commits.

## Aggregate Unpublished-Range Audit — Passed

Reviewed on 2026-09-27: all 39 commits and the combined changes from
`1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf` through
`9b54e88efae2f24815febe08122aecb56922fd60`. This section supersedes the
previous statement that the aggregate audit was incomplete; historical evidence
above is retained. No concrete release blocker was found in the reviewed code.

- Filesystem: opaque registered instance IDs and native-derived paths remain
  authoritative. Instance deletion requires stopped state, matching confirmation,
  managed containment and a recursive link/reparse preflight. Mod removal uses
  a fresh native inventory, exact regular-file identity/hash and dependency
  blockers. Provider removal targets verified owned records. Transaction cleanup
  is confined to its exact staged/backup files; normal `.minecraft` is unused.
- Downloads: official metadata boundaries and expected-digest cache acquisition
  remain intact. Provider graphs are bounded to 64 nodes and acquisition to eight
  concurrent artifacts. Activation rehashes cache and staged bytes, rejects
  collisions and uses no-clobber promotion with rollback. Nested JAR/Mixin reads
  are bounded and do not extract or execute archive content. Public 2.1.3 pins
  match the previously independently verified immutable artifact.
- Authentication: OAuth/PKCE, downstream tokens and Windows Credential Manager
  remain native. Cosmetic requests expose bounded pixels only. Refresh redemption
  and account removal share the restoration gate; texture requests carry no
  bearer credential. Tokens do not enter frontend DTOs or ordinary persistence.
- Launch/process: managed Java and session validation precede structured spawn.
  Final checks share content/registry/process serialization. Arguments remain
  Rust-only; bounded output is redacted before logs and diagnostics. Bridge state
  is owned by the supervised child lifecycle and cleared on failure/exit.
- Content: provider, local and bootstrap ownership remain separate. Satisfaction
  does not adopt files; disabled/damaged artifacts do not satisfy active needs.
  Exact pins cannot silently downgrade. Restart reconstructs local requirements;
  shared dependencies and explicit retention survive graph removal. Preview
  revisions and native commit checks prevent stale in-process activation.
- Compatibility: metadata predicates, fatal `breaks`, resolved Java and declared
  Mixin requirements are checked without mod-specific exceptions. Root collisions
  remain blocked; nested capabilities remain descriptive. ClassNotFound warnings
  alone do not establish a failed launch. This remains a conservative checker,
  not a complete Fabric solver or universal compatibility guarantee.
- Bridge: exclusive IPv4 loopback binding, fresh UUID and 256-bit capability,
  constant-time authentication, bounded attempts/frames/timeouts, strict schema
  and monotonic sequences remain enforced. Snapshots replace whole identities;
  EOF and child exit clear them. No persistence, frontend secret or control channel.
- Discord/privacy: independent native IPC worker failures do not authorize or
  block Play. Master and extra details default off; address sharing requires
  server sharing and explicit address consent. Address-shaped display names are
  filtered while address sharing is off. The asset key is `aurora-logo`.
- Configuration: supported schema 1/2/3 documents migrate to 4 preserving their
  existing fields; new privacy fields default false. Registry/accounts remain
  separate. Malformed/future documents fail without speculative overwrite.
- Release: the build job supplies and asserts public Application ID
  `1553653987545317396` before the exact Tauri compilation producing installers.
  The publish job verifies transferred bytes without rebuilding. Authoritative
  version fields are 1.1.0; no updater or signing infrastructure was invented.

Process-local exclusion does not protect against arbitrary external filesystem
writers or another launcher process. Deferred nested selection and dynamic Mixin
behavior remain documented limitations. Passing this audit does not substitute
for installer installation or the required GUI game acceptance.

### Required manual acceptance before publication

Native GUI automation remains unavailable in this session. The previously
identified computer-use runtime limitation is retained without rediscovery.
Neither installer installation nor the complete game sequence is claimed passed.

1. In a disposable Windows environment, run the generated 1.1.0 MSI or NSIS
   installer (preferably test both). Confirm name/version/icon, successful install,
   expected installation directory and no unexpected file placement. Launch the
   installed executable and verify 1.1.0 plus production Discord configuration in
   Settings. Record the installer hash, installed executable identity and results.
2. Through that installed production launcher, sign in normally and create the
   pinned production instance without replacing JARs. Hash `aurora-2.1.3.jar`:
   `4bf78dc1ef8f18e124377575c508ca357327be1c230b203e9f8181bdcb9ebc81`.
   Record Ready -> Play -> Starting -> Running -> Aurora main menu -> normal Quit
   -> supervised exit 0 -> Ready, including gameplay identity clearing.
3. Uninstall the controlled installation and confirm program cleanup preserves
   unrelated files and existing user/game data. Do not recursively remove managed
   data as a substitute for uninstall acceptance.
4. If available, use disposable singleplayer/local multiplayer state to exercise
   each world/server/address toggle, pause/disconnect clearing and master
   clear/restore. This live gameplay Discord check remains an unverified limitation;
   prior accepted generic presence/logo/master evidence is unchanged.

Publication remains STOPPED until required steps 1–3 pass. No push, tag, release
dispatch or publication is authorized while those gates remain unperformed.

### Final regression and artifact verification — 2026-09-27

After the aggregate review: Rust library 538 passed / 20 ignored / 0 failed,
icon integration 6 passed / 0 failed; total **544 passed / 20 ignored / 0 failed**.
Frontend **116 passed / 0 failed**. Svelte/TypeScript **0 errors / 0 warnings**.
Rust formatting, all-target check, frontend production build and release version
contract **1.1.0** passed. Configured Tauri production build passed (optimized
compilation 2m 59s), producing both MSI and NSIS.

The existing release artifact verifier rejected the working bundle directories
because historical 0.1.0 and 1.0.0 installers coexist there. They were preserved.
Exact new 1.1.0 artifacts, executable and generated NSIS script were copied into
a uniquely created external temporary projection; the unchanged release verifier
then passed its metadata, shortcut-hook and artifact-manifest checks. This is
artifact inspection, not installer installation acceptance.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| aurora-launcher.exe | 10169344 | `7935bdb20776798d1fa6e1d88e83872ab874f5ce891a66d9b76bcba85e5ab7cc` |
| Aurora Launcher_1.1.0_x64_en-US.msi | 5771264 | `883f7615ecee19795e9f023c92f742c1d5b53d20df57a464463d5ee2a9f48b44` |
| Aurora Launcher_1.1.0_x64-setup.exe | 4212146 | `02a00dfb5c584cf271b6e96879c082e64ecb5a6c04f12f5f76ba64ac1ff25c54` |

Both installer product identities were accepted as Aurora Launcher 1.1.0 by
the release verifier. The exact build-tree executable contains the public Discord
ID and bridge-enabled 2.1.3 digest; it created a live native window and closed
normally with launcher exit 0. This does not establish Minecraft exit 0 or
installed-executable acceptance. Installer install/uninstall and GUI game states
remain unperformed. Prior cold native production installation/Ready/hash evidence
is retained without rerunning it.

Logs: `%TEMP%/aurora-finalgate-{rust,check,frontend,svelte,webbuild,tauri}.log`.
Artifact inspection location is recorded in
`%TEMP%/aurora-finalgate-artifacts.json`. These are external evidence, not tracked
release assets. Rebuilt installer hashes supersede earlier candidate hashes.

Remote refetch still resolved main to
`1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`. All 183 original tracked paths
and all 185 candidate tracked paths remain; all 607 protected untracked files
were rehashed unchanged. Only this report and candidate release notes changed.
The pre-existing Cargo.toml status entry has no content diff and was preserved.
Client HEAD remains `e978d0b5545e4991137e0fc1387e9897e752dfc7`, with no tracked
content diff; its existing Python cache remains untracked. No client build,
publication, deletion, repository cleanup, launcher push, tag or release occurred.

## Owner-reported manual acceptance and release-body correction

After the prior report, the owner stated that the two remaining manual gates had
been completed successfully and, when asked about the enumerated checks, replied
"everything checks out" and then "i have verified all of the above". This is
owner-reported acceptance of the installer and production-game checklist, not
independent agent observation. No installer format, test-environment details,
screenshots, logs, installed-file hash output or individual state timestamps were
provided. This report does not attribute any such measurements to the owner.

The resumed pre-publication inspection found and corrected a separate release
blocker: `tools/release/publish.ps1` had hard-coded Aurora Client **2.1.2** into
the public release body and omitted `RELEASE_1_1_0_NOTES.md`. It now loads the
versioned notes source and appends verified installer metadata. A local rendering
check using the exact built installer manifest confirmed the body includes 1.1.0,
Aurora Client 2.1.3 and the installer checksum, without the old 2.1.2 claim.
The PowerShell script parsed, the version contract remained 1.1.0, and
`git diff --check` passed. This changes release publishing text only; it does
not change the compiled launcher, production pins or installer bytes.

The owner's manual attestation is accepted as the manual gate report. Exact
installer format(s) used during that manual check were not supplied; public
distribution follows the established NSIS format. No agent-observed GUI result
is claimed. At this point the release is still unpublished pending the final
remote, repository-safety and workflow gates.

## Partial publication recovery — 2026-09-28

GitHub Actions run `36375843103` passed the `resolve` and `build` jobs, including
the established production tests and installer verification, but failed in the
`publish` job after creating a draft and uploading its assets. Release ID
`397949176` was the sole `v1.1.0` candidate, with target commit
`97b9849e2fb023af74baed01b6b79d15d3165d80`, `draft=true`,
`prerelease=false`, and no public tag ref. The release-by-ID API returned the
draft; the release-by-tag API returned HTTP 404 before publication. The prior
script relied on that draft-by-tag lookup and therefore stopped. It also
expected the exact uploaded filename with spaces, while GitHub stored it as
`Aurora.Launcher_1.1.0_x64-setup.exe`.

The existing installer asset was downloaded independently from GitHub asset ID
`594474444`: **4,212,306 bytes**, SHA-256
`d9e3b1cac40b0e73289a730b3d0215fe6e0fdfdcbefcb81d10d0ab8211ed5cf5`.
Its PE product identity is Aurora Launcher 1.1.0. The draft body names Aurora
Client 2.1.3 and contains that installer checksum. The release manifest asset
is also present. No draft or asset was deleted, recreated, or reuploaded.

The publication logic now selects releases from the authenticated releases
listing, requires a unique exact tag/commit/draft identity, and carries the
numeric release ID through draft verification and publication. It accepts only
the expected filename or GitHub's space-to-dot normalization, then checks
candidate uniqueness, size, advertised digest, and independently downloaded
SHA-256 bytes. A rerun distinguishes no release, matching draft, matching
public release, and conflicting release/tag states. Deterministic tests cover
the draft-by-tag 404 condition, both asset identities, corrupt bytes, duplicate
assets, conflicting drafts and tags, and an existing public release.

This is a release-tooling and documentation correction only. The audited
production launcher source and installer were not rebuilt or modified.
