## Current authority operator contract

Follow `UPDATE_AUTHORITY.md`. Immutable installers and the separately gated Git authority ref are distinct publication operations. Release 402692668 / tag launcher-updates is retired evidence. This source correction is intended for later 1.4.1 manual bootstrap; never replace accepted public 1.4.0. Earlier preparation text remains historical where it assumes mutable release assets.

# Aurora Launcher release baseline

## Accepted 1.4.0 and historical preparation

Aurora Launcher **1.4.0** is now the accepted public immutable release, built
from `a9b1954fb389ca3ee4a560235a63ac13ab32a399`. Aurora Client remains **2.1.5**.
[RELEASE_1_4_0_PREPARATION.md](RELEASE_1_4_0_PREPARATION.md) and
[RELEASE_1_4_0_NOTES.md](RELEASE_1_4_0_NOTES.md) retain their original preparation
record. Use [UPDATE_AUTHORITY.md](UPDATE_AUTHORITY.md) for current gates,
recovery and the required manual 1.4.1 bootstrap; never replace accepted 1.4.0.

## Historical 1.1.0 candidate record

The synchronized local candidate version is 1.1.0. New compatible production
instances select the verified immutable Aurora Client 2.1.3 entry; historical
2.1.2 pins remain resolvable. The release build job supplies public Discord
Application ID `1553653987545317396` and rejects a missing or different value
before compiling. The asset remains `aurora-logo`. Development builds may omit
Discord configuration.

The acceptance report records outstanding publication gates. No 1.1.0 installer
has been published. The 1.0.0-specific format restrictions below describe the
initial release baseline; 1.1.0 publication still requires acceptance of its
selected installer format in a disposable environment. Existing installer files
must be preserved during local verification, even when older versions coexist
in the build output directory.

Aurora Launcher and Aurora Client are separate products. Launcher releases live in this repository. The initial release baseline bundled Client 2.1.2; the current prepared Launcher retains Client 2.1.5 and historical pins. Publishing a new Aurora Client artifact alone does not change launcher availability or any existing instance pin.

## Version and identity

`package.json` is the launcher version source. Tauri reads that file through `src-tauri/tauri.conf.json`; the native application status and About page display Tauri's resolved package version. `tools/release/verify-version.mjs` checks the npm lockfile, `Cargo.toml`, and `Cargo.lock` against it. `npm run build` and `npm run check` run this check, and the release workflow repeats it with the requested version. The executable and both installer metadata versions are checked after packaging.

Aurora Launcher **1.0.0** is the selected first public version. The inherited `0.1.0` was a development version and remains rejected by the release validator. For future launcher versions, update `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock` together. Run `npm run verify:version` and the full suite below. Do not use `npm version` with its default tag behavior. Launcher tags use `v<launcher-version>` in this launcher repository; Aurora Client 2.1.2 does not set the launcher version.

The Windows product name is **Aurora Launcher**, the executable is `aurora-launcher.exe`, and the stable Tauri identifier is `com.aurora.launcher`. Current installer `Manufacturer`/registry publisher text is `aurora`, derived from the identifier; no publisher certificate or explicit signing configuration exists. Preserve this identity and the checked-in icons. The installer uses the external Aurora icon, while the UI uses the transparent internal mark.

## Windows installers

`npm run tauri build` on Windows emits x64 NSIS (`Aurora Launcher_<version>_x64-setup.exe`) and x64 MSI (`Aurora Launcher_<version>_x64_en-US.msi`) because `bundle.targets` is `all`. **Only the NSIS x64 setup executable is selected for public Aurora Launcher 1.0.0 distribution.** The release workflow builds both for verification, but its resolve job rejects `msi` and `both` for 1.0.0. Dispatch 1.0.0 with `installer_format=nsis`; MSI remains available for internal builds and future decisions.

| Behavior | NSIS | MSI / WiX |
| --- | --- | --- |
| Default scope | Current user, `%LOCALAPPDATA%\Aurora Launcher` | Machine-wide, requires elevation |
| Start shortcut | Current user's Programs root | Common Programs `Aurora Launcher` folder |
| Desktop shortcut | Default checked on interactive finish page; created in silent mode | Common Desktop |
| Uninstall | Per-user uninstall entry and `uninstall.exe` | Windows Installer product registration |
| Existing same-name shortcut | Preinstall guard refuses a foreign target; uninstall checks the target before removal | WiX owns the authored common shortcuts as installer components; foreign-slot behavior needs isolated acceptance before choosing MSI for publication |

The NSIS preinstall hook checks the product-named user Start and desktop slots before Tauri's stock shortcut creation; a foreign or unreadable existing link stops installation before copying files. The stock NSIS uninstaller removes a shortcut only when it targets the installed executable. Settings can manage its own per-user desktop shortcut only when ownership is proven; it reports installer-owned Start shortcuts without changing them. The MSI uses WiX components for common shortcuts, so its foreign-slot behavior must be tested in an isolated environment before MSI is selected as a public format. The launcher-managed data root (`%LOCALAPPDATA%\com.aurora.launcher`: configuration, accounts reference, caches, runtimes, and isolated instances including user content) is outside the installation directory. **Default NSIS uninstall and MSI uninstall preserve it.** The stock interactive NSIS uninstaller also offers an unchecked **Delete app data** option; selecting it recursively deletes that data, including instances. Leave it unchecked during acceptance and make that consequence explicit in distribution guidance. Reinstall should reuse and validate preserved data. Do not delete `.minecraft` or instance content merely to make uninstall look clean.

Neither format has configured **Windows Authenticode publisher signing**. Windows may show an unknown-publisher and SmartScreen/reputation warning; updater minisign signing does not replace Windows publisher authentication. Authenticode is a separate owner decision. Do not add a fabricated certificate or private key to the repository. If a trusted publisher identity is obtained, sign before final updater signatures/artifact metadata/hash recording and verify Authenticode on the exact transferred bytes. Ordinary tests/development require no signing credentials. The prepared 1.4.0 workflow separately requires genuine updater minisign inputs.

## Manual release workflow

`.github/workflows/launcher-release.yml` has only `workflow_dispatch`. Before invoking it:

1. Audit and normally push reviewed history to `main`; do not force-push or move a tag.
2. Verify the synchronized release version, then accept the exact signed production NSIS installer on a disposable Windows user/profile or VM before approving publication. Keep existing developer installation and launcher data intact.
3. Configure a GitHub Actions environment named `launcher-production` with required reviewers (preferably disallow self-review). GitHub does not make a newly named environment reviewer-gated automatically. Ensure Actions can create releases with its scoped `GITHUB_TOKEN`; no broad PAT is needed.
4. Retain the owner-only signing inputs. Initialize and protect the distinct authority ref as described in [UPDATE_AUTHORITY.md](UPDATE_AUTHORITY.md), under separate authorization; historical release-asset initialization instructions are superseded.
5. Dispatch only after owner authorization from `main`, with the exact 40-character current reviewed main SHA, matching version and `installer_format=nsis` (or `both` after MSI acceptance). Historical 1.0.0 remains NSIS-only.

The read-only resolve job requires exact current main for a new release and rejects conflicting tags/releases; explicit recovery is bound to an existing release ID. The protected Windows build runs frontend, Rust and release-tool checks, builds official Tauri signed installers, inspects Windows product metadata and signatures, and records exact hashes/sizes. It transfers selected installers/signatures, immutable `release-assets.json` and generated `launcher-update.json` privately for seven days. Owner accepts those exact bytes before approving the separately protected publish job, which creates/verifies a **draft**, uploads immutable assets, publishes and verifies unauthenticated public bytes. The final protected `advance_update_authority` job verifies public signed assets again and publishes the sole discovery manifest **last**, under global authority concurrency. Publication never rebuilds or overwrites signed installers. Non-forced authority advancement can temporarily serve stale raw content; current recovery rules and gates are in UPDATE_AUTHORITY.md.

GitHub releases are the public release location. The workflow summary and release notes report version, source SHA, filename, format, architecture, size, and SHA-256. Workflow publication is never triggered by a push, pull request, schedule, or another workflow. Do not create the production tag or invoke the workflow until the developer makes the final publication decision.

## Local verification and historical initial-release acceptance

```sh
npm ci
npm run verify:version
npm test
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

For final acceptance, install the exact final installer in a clean, disposable Windows environment. Check the executable, installed version/icon, Start and desktop shortcuts, uninstall entry, and normal launch outside dev mode. With fresh launcher state, sign in through the system-browser flow, create a new stable Aurora Client 2.1.2 instance, confirm Minecraft 1.21.11 / Fabric Loader 0.19.5 / Fabric API 0.141.6+1.21.11 / managed Java 21, and verify both required mods are protected. Play, observe Aurora ready and launcher Running, confirm a duplicate launch is blocked, close the game, and observe clean exit. Restart the installed launcher and verify account/instance/selection/readiness persistence. Uninstall and reinstall the same installer; verify installed files and owned shortcuts are removed and restored without duplicate registration, while launcher data remains intact. Capture screenshots and logs outside Git, with secrets redacted. Do not treat a development executable or an older existing installation as final installer acceptance.

## Future launcher updates

Phase L implements official Tauri self-updates through one owner-published `launcher-update.json` and a compile-time minisign public key, plus fingerprinted SHA-256 Client transactions from one published `aurora-releases.json`; see [PHASE_L_PRODUCTION_UPDATES.md](PHASE_L_PRODUCTION_UPDATES.md). There are no public channels. One bounded startup discovery check is silent unless an update exists; a single explicit Settings action checks both domains and Update applies Client before Launcher. No polling or automatic installation exists. Missing signing configuration fails honestly unconfigured. The prepared workflow consumes owner-supplied signing inputs and verifies official signed NSIS outputs before separately gated manifest-last publication. No real key or manifest was configured/published during preparation. Commits/builds alone never become updates. Launcher and Client versions remain independent; installed newer never downgrades.

The historical initial-release provider roadmap has since advanced through the integrated content/modpack/update work and accepted 1.4.0 publication. Review this local authority correction before any push or separately scoped 1.4.1 correction/release work. The first normal production self-update acceptance is expected to be 1.4.1 → 1.4.2. This implementation starts no feature-freeze or website work.
