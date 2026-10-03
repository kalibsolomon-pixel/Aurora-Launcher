# Aurora Launcher 1.4.0 — production release preparation

Prepared locally on 2026-10-03. **READY FOR OWNER SIGNING SETUP; DO NOT RELEASE YET.** This is preparation for a real production release, not a diagnostic distribution. Nothing was pushed, tagged, uploaded or published. No production key or password was generated or configured. No installer was built, installed or executed in this task. Owner signing setup, exact signed-build acceptance and publication remain separate tasks.

## Source and version authority

- Starting branch: `main`; exact starting HEAD/local main: `98b4214e93a86025a252a10e12ae9c6b346632dc`.
- Recorded `origin/main`: `20d7d5cdc9805c1cdbcd35217021355c1a4c745f`; local main was 46 ahead / 0 behind. These are measured local refs, not a claim that the remote was fetched during preparation.
- Preparation branch: `codex/release-1.4.0-preparation`, created at that exact main commit. Main, origin/main and existing tags remain unchanged.
- Initial inventory: 369 tracked files and 15,050 protected untracked files. Evidence is in the new `.zcode-diag/release-1.4.0-preparation/` directory; it is not release content. No original tracked file was deleted.

| Location | Role | Prepared value |
| --- | --- | --- |
| `package.json` | Launcher version authority | 1.4.0 |
| `package-lock.json` root and root package | Synchronized npm metadata | 1.4.0 |
| `src-tauri/Cargo.toml` | Native package version | 1.4.0 |
| `src-tauri/Cargo.lock` Aurora Launcher package | Synchronized native lock metadata | 1.4.0 |
| `src-tauri/tauri.conf.json` | Reads `../package.json` | Derived 1.4.0 |
| Native status / About / updater comparison | Tauri package information / native package version | Derived 1.4.0 |
| NSIS/MSI metadata and release asset records | Tauri build and post-build inspection | Expected 1.4.0; exact signed bytes pending |

`npm run verify:version` verifies the synchronized sources. Product name `Aurora Launcher`, executable `aurora-launcher.exe`, identifier `com.aurora.launcher`, managed-data identity and installer scope are unchanged. Historical 1.3.1 and older acceptance records, diagnostic versions, test fixtures and historical installer files were preserved. Aurora Client remains **2.1.5**; its production catalog and `aurora-releases.json` authority are unchanged.

## Current official updater requirements

Research used the [official Tauri 2 updater guide](https://v2.tauri.app/plugin/updater/), [official locked CLI signing source](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-cli/src/helpers/updater_signature.rs), and the installed official updater sources corresponding to the lockfiles (CLI 2.11.4; updater plugin 2.13.1). The separate online configuration reference timed out; configuration requirements were checked against the guide and local types. No updater dependency or runtime security architecture was changed.

- Updater signatures are mandatory. The private signing input is `TAURI_SIGNING_PRIVATE_KEY` (path or key content); its optional Tauri password input is `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Actual environment variables are required; a `.env` file does not provide these inputs to the signing process.
- `bundle.createUpdaterArtifacts: true` selects Tauri 2 updater artifacts. On Windows these are the normal NSIS `.exe` / MSI `.msi` and their `.sig` files; no separate legacy ZIP is required. `v1Compatible` is not selected.
- `plugins.updater.pubkey` takes public key **content**, not a filename. Aurora also compiles that same trimmed content through `AURORA_UPDATER_PUBKEY` for its Rust-owned updater builder. The temporary release configuration contains only the public key and artifact-generation flag.
- Production discovery uses HTTPS. The static manifest has a semantic `version` and a `platforms` map with `windows-x86_64`, HTTPS `url` and literal encoded `signature` content. Notes and an RFC3339 `pub_date` are optional. This preparation omits `pub_date` to keep output deterministic before the actual publication time exists.
- Official Windows installation uses the installer selected by the manifest. Aurora retains the official NSIS path and existing passive/restart behavior; exact shutdown/install/relaunch is a later acceptance requirement. No custom executable replacement is introduced.

The locked CLI signs the payload and authenticated filename; it does not emit an authenticated `version:` comment. The locked plugin checks a signed version when present and otherwise accepts the normal legacy comment. The release verifier therefore verifies payload, key ID, authenticated filename (which contains 1.4.0), and any supplied authenticated version, while Windows metadata inspection separately checks the product version. This matches the accepted Phase L conditional behavior; **do not claim unconditional signed-version binding** or silently require a different signature dialect.

## Workflow audit and prepared ordering

`.github/workflows/launcher-release.yml` remains **workflow_dispatch only**. Pushes, tags, merges, schedules and ordinary builds cannot publish updates. A new release must be dispatched from main with its exact reviewed 40-character current main SHA and matching version; explicit same-release recovery may use an ancestor. Existing release/tag collisions fail deliberately.

1. Read-only resolve: verify source/version, format and exact release state. NSIS is mandatory; `installer_format=nsis` or `both` is accepted, never MSI-only.
2. `build`, protected by environment `launcher-production`: run frontend, Rust and release-tool checks. Only the signing build step receives private secrets. Create a public-only temporary override, build official Tauri artifacts, inspect Windows versions and shortcut guard, verify signatures, hash exact bytes and generate the private manifest.
3. Transfer `aurora-launcher-1.4.0-nsis` (or `-both`) as a private workflow artifact, retained for seven days. Owner reviews and accepts these exact bytes before approving publication; do not rebuild accepted installers.
4. `publish`, separately protected by the same environment: reverify transferred objects using frozen source tooling, create/verify a draft, upload immutable selected installers/signatures and `release-assets.json`, verify the inventory, publish, then download and verify every public asset **without authentication**.
5. `expose_update`, dependent on successful publication and separately protected: verify the public release identity, exact inventory, every public byte and latest-release identity again, then expose `launcher-update.json` **last**. Its global concurrency group serializes authority jobs across versions.

GitHub environment names do not automatically require review. Owner must configure actual required reviewers and appropriate branch restrictions before any dispatch. Build/publish/authority approval must remain intentional; production source integration and any push also require separate authorization.

## Artifact names and metadata contract

| Artifact | Local Tauri output | Expected GitHub asset |
| --- | --- | --- |
| NSIS / updater installer | `Aurora Launcher_1.4.0_x64-setup.exe` | `Aurora.Launcher_1.4.0_x64-setup.exe` |
| NSIS updater signature | `Aurora Launcher_1.4.0_x64-setup.exe.sig` | `Aurora.Launcher_1.4.0_x64-setup.exe.sig` |
| Optional MSI | `Aurora Launcher_1.4.0_x64_en-US.msi` | `Aurora.Launcher_1.4.0_x64_en-US.msi` |
| Optional MSI signature | `Aurora Launcher_1.4.0_x64_en-US.msi.sig` | `Aurora.Launcher_1.4.0_x64_en-US.msi.sig` |
| Immutable release inventory | `release-assets.json` | Same name under `v1.4.0` |
| Sole mutable discovery manifest | `launcher-update.json` | Same name under `launcher-updates` only |

Names are expected from configuration; no 1.4.0 installer bytes have been constructed or accepted here. GitHub's existing asset normalization changes spaces to dots. Signature files contain Tauri's encoded minisign signature; manifest signature is that file's content, not its path.

The sole public discovery URL remains:

`https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/launcher-updates/launcher-update.json`

Tooling deterministically generates `{ version, notes, platforms: { "windows-x86_64": { url, signature } } }` from the exact verified NSIS artifact/signature, version and checked-in notes. The installer URL is fixed to this repository, tag `v1.4.0` and expected NSIS name. No stable/beta/nightly feeds or frontend-controlled artifact inputs exist.

`release-assets.json` remains an immutable build/provenance inventory: version, source SHA, selected installer format/architecture/size/SHA-256, signature size/SHA-256, platform and public-key fingerprint. It contains no secrets and is not an update feed. The private workflow artifact additionally contains the generated discovery manifest; the versioned public release does not publish a competing copy of that manifest.

## Idempotency and recovery

- Build reruns are private until independently reviewed. Signatures may differ between builds; acceptance belongs to exact bytes. Never overwrite an existing public installer/signature with a rebuilt object.
- Release/tag collision recovery uses the exact existing release ID and source. Published assets are verified, never replaced. An incomplete existing draft fails safely and requires human investigation; the preparation does not authorize deleting/recreating it.
- An identical existing authority manifest is a no-op. Same-version different bytes, malformed authority and downgrades fail. A newer public latest release prevents an older run from exposing metadata.
- Authority publication requires an existing owner-initialized public `launcher-updates` release with body marker `Aurora Launcher production update authority.`, no prerelease status and no unrelated assets. Owner must initialize it in a separately authorized task with `make_latest=false`; the preparation neither creates it nor supplies fake update content.
- Failures before authority exposure retain the existing feed and immutable assets. Replacement deletes only the exact proven **mutable manifest asset**, then uploads its replacement; no versioned installer, signature, tag or release is deleted. This is not atomic and can temporarily return 404. A failed replacement leaves discovery unavailable, not a feed advertising unverified bytes. Retry only the final job using the same accepted workflow artifact after inspection; never rebuild or rewrite a historical release as recovery.
- Workflow concurrency does not prevent every external/manual race. No other writer should modify this authority during the controlled publication; rereads detect identity/content changes immediately before mutation.

## Security review

The focused release/runtime audit passes for preparation:

- No production private key or signing password was found in tracked sources, frontend or generated metadata; no real production key was created. Unit signing keys exist only in test-process memory. Preserved historical diagnostic files are not staged.
- Private inputs appear only in the protected Tauri signing step. Tooling prints generic validation errors, never key/password values or secret response bodies. The generated override includes only the public key.
- Rust owns the fixed HTTPS discovery endpoint and typed update action. The frontend supplies neither arbitrary URLs nor signatures. Native updater behavior is unchanged.
- Download/install remains official Tauri with signature verification before execution; no unsigned self-update path, shell/PowerShell self-replacement or signature bypass was added. PowerShell here orchestrates release packaging only.
- Missing compiled public key remains honestly unconfigured. Tests/development do not need production secrets. No HTTP production endpoint, channel feeds or polling were introduced.
- Authority exposure depends on verified signed immutable public assets and a manual gate. Client SHA-256 validation, dependencies, pins, account storage and user-data isolation were not weakened.
- Updater minisign signing is distinct from Windows Authenticode publisher signing. A minisign-signed installer may still show an unknown Windows publisher; no certificate or reputation claim is fabricated.

## Verification completed locally

Evidence logs live only in the scoped diagnostic directory.

| Check | Result |
| --- | --- |
| `npm run verify:version` | PASS, 1.4.0 |
| `npm run check` | PASS, 0 errors / 0 warnings |
| `npm test` | PASS, 221 tests in 16 suites; 0 failed/skipped |
| `npm run build` | PASS, frontend production build |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | PASS; existing warnings |
| `cargo test --manifest-path src-tauri/Cargo.toml` | PASS, 814 library + 6 icon tests = 820 passed; 0 failed; 29 deliberately ignored |
| `node --test tools/release/*.test.mjs` | PASS, 32 tests; 0 failed/skipped |
| Node/PowerShell syntax checks | PASS |
| Offline signature cross-check | PASS against preserved official Tauri-signed diagnostic installer using its public key only; installer not executed |

No `npm run tauri build` was performed. Avoiding it preserves historical local packaging outputs and leaves exact installer/version/signing/boot acceptance to the owner-configured build. This task produced no releasable installer and does not claim real application boot or installation acceptance.

## Release notes draft

See `RELEASE_1_4_0_NOTES.md`: Modrinth browsing and compatibility information; supported modpack installation and reviewed updates preserving local changes; managed content dependency checks/recovery; NeoForge alongside Fabric/Vanilla; one Client/Launcher Check for Updates and explicit Update/Retry; improved validation and state preservation. New compatible Aurora instances retain Client 2.1.5, existing pins remain, and nothing installs automatically. The draft is not published.

## Next-task production acceptance checklist

Use exact final signed artifacts and a disposable Windows user/profile or VM, never the developer's existing installation/data. Capture redacted evidence outside Git. Measure actual state-file inventories at acceptance time; do not infer counts from old reports.

### Before publication

- [ ] Record reviewed source SHA, synchronized 1.4.0 versions, signing public-key fingerprint, selected formats, exact artifact hashes/sizes and valid signatures. Compare the compiled public key to the owner's public key. Verify executable and NSIS/MSI product metadata and shortcut guards on final bytes.
- [ ] Download the private build artifact, preserve it and prevent publication approval until fresh install and upgrade acceptance pass. Do not rebuild between acceptance and publication. If seven-day retention expires, a new build requires new acceptance.
- [ ] Read-only inspect intended Client authority and immutable Client 2.1.5 entry/digest. No Client artifact/tag/manifest is changed. Verify the new Launcher manifest references exactly the accepted NSIS signature and expected immutable URL.
- [ ] Check real required reviewers/branch restrictions and owner-initialized dedicated authority. Review public asset verification, manifest-last dependency and immutable collision protection.

### Fresh install

- [ ] Install official NSIS in a clean disposable profile; verify executable, 1.4.0 status/About, icon, shortcuts, uninstall registration, current-user scope and normal production boot outside dev mode.
- [ ] Confirm managed root `%LOCALAPPDATA%\com.aurora.launcher` and separate installation path. Confirm `.minecraft`, system Java and developer data remain untouched.
- [ ] Sign in through the system browser; verify account selection/persistence and OS credential handling without exporting tokens/passwords. Restart and restore a usable session.
- [ ] Create a compatible Aurora instance using Client 2.1.5; verify concrete Minecraft/Fabric/dependency pins, managed Java and ready validation. Verify protected required mods and user-content separation.
- [ ] Observe Ready → Starting → Running, reach Minecraft main menu, reject duplicate same-instance launch, exit normally, return to Ready and restart the launcher with state preserved.
- [ ] If MSI is selected for publication, separately accept its machine-wide elevation, metadata, shortcuts and foreign-shortcut behavior in a disposable VM before approving `both`.

### Upgrade from production 1.3.1

- [ ] Record actual files/counts and SHA-256 baselines of nonsecret configuration, account references/active account, registries, concrete pins, game/content/provenance and modpack ownership/reconciliation records. Record settings semantically; never dump credential store secrets.
- [ ] Upgrade in place using the accepted owner installer. This bootstrap is installer acceptance, not a claimed retrospective self-update proof for an old build lacking the production key.
- [ ] Before ordinary use, compare preservation evidence; after boot, distinguish intentional supported schema persistence/session rotation from unexplained data loss. Fail on damaged/unknown documents, missing content or unauthorized pin changes.
- [ ] Verify existing Fabric and Client 2.1.5 instances, existing Modrinth content/modpack records and a supported NeoForge instance. Validate readiness without silent repair, re-download or repinning.
- [ ] Verify accounts/active account, instance selection, appearance, Borealis, widget arrangement/privacy, Discord settings, update state, recent history/activity, installed content and reconciliation ownership.
- [ ] Verify saves, user mods, config, resource packs, screenshots and logs are preserved. No user file is unexpectedly enumerated as damage, deleted or rolled back.
- [ ] Exercise normal game launch/exit and restart. Uninstall/reinstall only in the disposable profile with NSIS **Delete app data unchecked**; confirm managed data survives and owned shortcuts/registration do not duplicate.

### Updates and content

- [ ] Verify one Check for Updates action covers Client and Launcher; no Stable/Beta/Nightly selector. Current release produces None Available, reset after five seconds. Startup discovery remains bounded/silent when current; no polling or auto-install.
- [ ] Offline checks yield concise Retry behavior without raw URLs, token leakage or unexpected state mutation. Reconnect and retry successfully.
- [ ] Inspect the real Client `aurora-releases.json` and real Launcher `launcher-update.json` sources. Verify missing-key development behavior remains unconfigured, while the final 1.4.0 production build uses the genuine public key.
- [ ] Exercise ordinary managed-content updates with dependency/provenance checks, Modrinth modpack ownership and reconciliation, local-file conflicts and supported NeoForge content; compare user-state baselines before/after. No unrelated user content is changed.
- [ ] Test wrong signature/key, payload drift and conflict failures using isolated diagnostic tests only; never publish fake production versions or replace the public authority to force a test. Real Client update application waits for an independently authorized genuine newer Client release if none exists.
- [ ] After separately authorized publication, unauthenticated public hashes/signatures match accepted private bytes, authority points to them, and installed 1.4.0 safely reports current. Capture progress, cancellation/failure and Retry evidence where supported.

## Two-layer self-update proof

Layer 1 proves the genuine 1.4.0 key is embedded, its installer/signature verify, the real HTTPS manifest is checked, and equal 1.4.0 is rejected as an update. **1.4.0 → 1.4.0 cannot prove installation; do not falsify the version/feed to make it happen.**

Layer 2 waits for a separately authorized, scoped genuine 1.4.1 correction release. From installed production 1.4.0: Check for Updates → explicit Update → download/verify signed 1.4.1 → official installer → relaunch as 1.4.1. Recheck account/active selection, all concrete pins, managed content/provenance/modpack ownership, settings, widgets, history, user content and normal game lifecycle. This actual public upgrade is the final updater proof before Launcher freeze. No 1.4.1 or website work is started here.

## Remaining publication gates

Owner-generated production key/password and protected Actions configuration; review/integration of these local commits and separately authorized push; final signed build and exact-byte Windows acceptance; owner initialization of the dedicated authority without making it latest; explicit public release approval and final manifest approval. No preparation result substitutes for those gates.

## Owner-only signing setup — next manual task, not executed here

1. On the owner's trusted Windows machine, use this repository's installed Tauri CLI. Choose a private, access-controlled directory outside the repository and diagnostics. Check for an existing production pair first; never overwrite it. In PowerShell, the following example creates a new pair only after the owner intentionally runs it:

   ```powershell
   Set-Location C:\Dev\aurora-launcher
   $auroraSigningDirectory = Join-Path $env:LOCALAPPDATA 'AuroraSigning'
   $auroraSigningKeyPath = Join-Path $auroraSigningDirectory 'aurora-launcher.key'
   if ((Test-Path -LiteralPath $auroraSigningKeyPath) -or (Test-Path -LiteralPath ($auroraSigningKeyPath + '.pub'))) {
     throw 'A signing pair already exists; inspect it rather than overwriting it.'
   }
   New-Item -ItemType Directory -Path $auroraSigningDirectory -Force | Out-Null
   npm run tauri signer generate -- -w $auroraSigningKeyPath
   if ($LASTEXITCODE -ne 0) { throw 'Production key generation failed.' }
   ```

2. Enter a strong password at the CLI's interactive prompt; do not pass it as a command argument, use `--ci`/`--force`, write it into `.env`, or share it in chat. Keep the private `.key`, password and secure offline backup owner-controlled. Losing this key prevents signing updates trusted by installed users; key rotation requires a separately reviewed migration.
3. In the repository's GitHub Settings → Environments → **launcher-production**, configure required reviewers, preferably prevent self-review, and restrict release deployment to reviewed main. Use the environment's secret/variable UI or an owner-controlled secret manager; no agent secret upload is needed.
4. Add **environment secrets** `TAURI_SIGNING_PRIVATE_KEY` containing the encrypted private `.key` file's content (a local owner path is not usable on a hosted runner), and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` containing its password. Add the **public environment variable** `AURORA_UPDATER_PUBKEY` containing the trimmed encoded `.key.pub` file content, not a path. Only the public key is compiled or written into temporary build configuration. Do not place any of these private values in committed files, issue bodies, logs or chat.
5. Confirm the intended public key fingerprint independently. The existing workflow consumes these exact inputs only during its protected signing step; ordinary tests need none. **Do not dispatch a release during key setup.** In a subsequent authorized build/acceptance task, use version 1.4.0, the then-current exact reviewed main SHA, and NSIS (or both after MSI acceptance). Review the signed private artifact before approving publication and the single manifest last. Initialize the dedicated authority only under that later authorization.

**AURORA LAUNCHER 1.4.0 PREPARATION COMPLETE — READY FOR OWNER PRODUCTION SIGNING SETUP — NOTHING PUSHED — NOTHING PUBLISHED — DO NOT RELEASE YET.**
