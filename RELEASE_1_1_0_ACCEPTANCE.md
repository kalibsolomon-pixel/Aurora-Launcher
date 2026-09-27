# Aurora Launcher 1.1.0 release gate — stopped

Date: 2026-09-27. No release is accepted or published by this audit.

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
