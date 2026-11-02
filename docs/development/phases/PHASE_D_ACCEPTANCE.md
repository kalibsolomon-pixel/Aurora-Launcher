# Final Phase D corrective acceptance — 2026-09-26

Phase D remains unpublished. This final pass replaces the flat player presentation, supplies installed provider artwork, and corrects a generic nested-module conflict rule. Earlier account chooser/dialog acceptance remains valid. Spxcterr remains active; both existing account records are preserved. No credentials, authorization codes or launch arguments are included in this report.

## Git and repository preservation

Starting HEAD: `4905206e1fe404dff3464d3e19dd6580a07b4e8f`. Fetched origin/main and merge-base: `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`; starting ahead/behind **7/0**. The seven preceding Phase D commits, including `27220fa` and `4905206`, remain intact without amendments. Final fetch confirmed the same remote. New commits are recorded below; the documentation commit's hash is provided in the final response. Final ahead/behind after the six new commits is **13/0**. Nothing is pushed.

- `12c1363`: static 3D player beside the Home launch card.
- `2d4656e`: installed provider artwork and typed native lookup.
- `fd9ccf1`: generic install collision correction and structured conflict presentation.
- `0598d1d`: inventory warnings use the same root/nested distinction. `a6231a5`: declared-conflict diagnostics preserve version constraints.

Before each commit the original 153 tracked paths and all 606 protected diagnostic files were audited. No baseline file is missing; diagnostic bytes remain unchanged and untracked. No source deletion, dependency addition, lockfile change, identifier change or generated build/game/runtime staging occurred.

## Home and player

The functional instance card sits center-left, containing the integrated name/chevron picker, native configuration, status, Play, Manage Instance and exit information. Its former player column is gone. The substantial player stands separately in the right Home canvas without a second card or interactive control.

A purpose-built software canvas projects textured cuboid faces with perspective, depth sorting, face culling, shading, nearest texels and alpha. It draws only when validated skin/model data changes; there is no animation loop, WebGL context or engine dependency. Rust supplies validated RGBA skin pixels and authoritative classic/slim metadata. Classic arms are four pixels wide and slim arms three; legacy skins mirror the original limbs and omit nonexistent body overlays. Modern skins include distinct limbs, hat and outer body layers. The body is slightly angled, the head turns toward the content, and opposite arms/legs use a restrained static walking pose. Missing or malformed imagery uses a local 3D default player without affecting readiness or Play.

Actual production Home and the open picker were inspected at 2048×1152 desktop dimensions without overlap or obvious overflow. The decorative player hides below the existing narrow-layout threshold so functional controls retain priority. Extensive window-size checks remain waived by the user.

## Installed artwork

Persisted provider provenance already contains provider, project ID and version ID. Installed rows now use the native inventory's providerManaged classification and Modrinth project ID to request official project metadata through `get_modrinth_project_artwork`. No filenames, usernames, local mod IDs or frontend URLs identify projects. The native policy reuses Browse's HTTPS/CDN restrictions: `cdn.modrinth.com`, `/data/`, no userinfo, custom port or fragment.

The frontend shares a bounded 128-entry promise cache with request deduplication, a ten-minute successful lookup lifetime and a one-minute failure lifetime. Browser image caching supplies image reuse. Request/image failures quietly show the existing M/J fallback; inventory and ownership never depend on artwork. Local, Unknown and required artifacts without Modrinth provenance do not acquire invented provider identity.

The release displayed actual AppleSkin, Mod Menu and Placeholder API artwork. Fabric's `fabric.mod.json` icon declaration exists, but safely extracting/decoding embedded local images would introduce another archive/image boundary. That optional secondary source is deferred; Local/Unknown retain the safe glyph.

## Exact Sodium false conflict

The original selected production instance was reproduced safely before activation: Minecraft **1.21.11**, Fabric **0.19.5**, Aurora **2.1.2**. Its eleven mod files and provider document remained unchanged after the rejected old-release attempt. Native metadata inspection found no existing top-level Sodium and no target filename duplicate.

The old provider activation rule combined every incoming root/nested Fabric identity and rejected any intersection with a required artifact's root/nested identities. Sodium bundles nine Fabric modules also bundled by the required Fabric API JAR:

`fabric-api-base`, `fabric-block-view-api-v2`, `fabric-lifecycle-events-v1`, `fabric-renderer-api-v1`, `fabric-rendering-fluids-v1`, `fabric-rendering-v1`, `fabric-resource-loader-v0`, `fabric-resource-loader-v1`, `fabric-transitive-access-wideners-v1`.

That nested-to-nested overlap was incorrectly treated as replacing a launcher-owned top-level artifact. The generic correction allows shared nested declarations to remain Fabric loader resolution inputs. Incoming root/root, root/nested and nested/root collisions remain blocked. Required artifact validation, provider hashes/provenance, exact filename containment/collision rules, compatibility checks, mutation locks and activation rollback remain authoritative. There is no Sodium/project-ID exception and no adoption, overwrite or removal of protected/local content.

Live Installed inventory also exposed a descriptive warning based on different versions of those shared nested modules. The final inventory correction applies the same distinction: multiple roots or a root plus a nested declaration warn; nested-only declarations do not become fabricated top-level conflicts. The existing nested fixture now proves differing nested versions remain descriptive and that adding an actual root copy still warns for both artifacts. Declared-conflict messages also retain the metadata version constraint and state that Fabric evaluates applicability, instead of treating the mere presence of an ID as proof that its installed version conflicts. A fixture with an API outside the declared break range verifies this honest diagnostic; no version resolver or admission bypass is introduced.

Native preflight acquires expected-digest artifacts into the trusted cache, reads bounded metadata and returns structured conflicting mod ID, filename, ownership and reason without activating instance content. Details renders those native fields and disables blocked previews. A quick-install collision records a blocked state with Details instead of leaving a misleading arrow. Browse's Installed state continues to use actual provider provenance, separately from local identity collisions. Activation repeats its safety checks. Genuine local/protected collision details are covered by deterministic native fixtures and compiled presentation tests; no artificial local JAR was added to the user's instances solely for a screenshot.

## Sodium live acceptance

Controlled target: **Aurora performance acceptance**, instance `6cd0cc88e87b43159051ab7b52edcdf3`, Minecraft **1.21.11**, Fabric **0.19.5**, Aurora **2.1.2**. Baseline: five mod files and seven provider records across mods, resource packs and shaders. Existing content was snapshotted before this test.

Official Modrinth project **AANobbMI**, version **rkdTcxoT**, version number **mc1.21.11-0.8.14-fabric**, file **sodium-fabric-0.8.14+mc1.21.11.jar**. Official version metadata selects Minecraft 1.21.11/Fabric; the archive's loader requirement is satisfied by 0.19.5 and its Fabric API lower bound by installed 0.141.6. Modrinth declares no extra dependencies for this release. The native reviewed provider path proposed and installed exactly one file.

Published SHA-512 and actual installed bytes:

`04c43f9e8534b87a52c42ffd51b0e344d4ef92dc9cc52da33d13af6a18bf74b05380d2027f1e0db982698d0aff274c42741c1e21b313b0cd75ed3af005fc98d9`

Recorded local SHA-256 and actual installed bytes:

`fd2619eff5da6b9ba6304b8b72ac3fe132c30e0bd5d675e4581eda417a272f5d`

Provenance recorded Modrinth/project/version/expected file hash/local hash, client-only Fabric compatibility and explicit retention. Browse became Installed; native inventory classified Sodium as Managed. The controlled mod directory contained six files, with no second Sodium artifact.

The supervised native launch started process 22164. Its game log confirms `Loading Minecraft 1.21.11 with Fabric Loader 0.19.5`, `sodium 0.8.14+mc1.21.11`, `Loaded configuration file for Sodium: 36 options available, 0 override(s) found`, and `Aurora Client ready.` The user confirmed the title screen and performed normal Quit. The menu was also captured visually. The launcher then reported **Exit code 0**, **Exited**, content/runtime **Ready**, and Play available.

The UI tool could not target the game's foreground window reliably; normal Quit was therefore performed by the user. A menu image obtained while the game occluded the launcher is explicitly indexed as game evidence, not Home evidence. This limit does not replace the native process/log/exit checks.

After the final production rebuild and boot, Installed showed the authoritative Sodium artwork, Managed classification, and version-qualified diagnostic with no shared nested-module duplicate. The existing provider removal preview named only the test Sodium JAR; its approved native lifecycle removed exactly that file. Cleanup restored all five original mod files and the complete seven-entry provider document byte-for-byte, including resource packs/shaders. Production retains its eleven original mod files and byte-identical provider document. No duplicate Sodium or stale staging remains. Home restored the original `413e831999bd444a8e0df2d22fc6d134` selection and Ready; accounts, registry and appearance match their snapshots. The successful game launch preceded only descriptive inventory diagnostic changes; final boot, inventory and cleanup used the final MSI/NSIS release.

## Verification

| Check | Result |
|---|---|
| Full Rust suite | **483 passed / 16 ignored / 0 failed**: 477 library + 6 integration |
| Frontend suite | **102 passed / 0 failed**, 13 suites |
| Svelte/TypeScript | **0 errors / 0 warnings** |
| Rust formatting / all-target check | Passed |
| Frontend production build | Passed |
| Tauri MSI/NSIS production build | Passed; 1.0.0 MSI and NSIS artifacts |
| Version contract | **1.0.0**, passed |
| Real production boot | Passed from the repository release executable |
| Whitespace / preservation audits | Passed |

Six new native cases cover shared nested activation, local/root identity collision, protected-root collision, read-only preflight, restricted artwork lookup and version-qualified conflict diagnostics. The full prior ownership, tamper, filename, compatibility, C1/C2 transition, capability, launch, nested parser, avatar and OAuth tests remain. Frontend additions cover cuboid/raster UVs, slim/legacy/overlays/fallback, outside-card composition, artwork identity/cache/failure and native conflict details. The final warning correction re-runs the full Rust suite and release build; frontend sources are unchanged after their passing checks.

The review of `9c967ed` remains unchanged: native capability gates unsupported lifecycle reads while local inventory stays available. This pass preserves that gate and does not broaden supported platform/content activation. Account selection still uses the earlier explicit Microsoft chooser; refresh remains noninteractive. Both accounts are retained.

## Screenshot and evidence index

Evidence root: `C:\Users\kalib\AppData\Local\Temp\aurora-phase-d-final-evidence` (outside Git).

- `home-3d-original.jpg`: substantial angled active skin outside the functional card at desktop dimensions.
- `home-3d-picker.jpg`: integrated picker open with the player outside the card.
- `installed-provider-artwork.jpg`: actual provider icons and unchanged Required/Managed ownership presentation.
- `before-sodium-rejection.txt` / `.jpg`: old-release rejection on the original instance; text retains the transient toast.
- `sodium-installable-preview.jpg`: native compatible one-file preview with no false blocker.
- `sodium-browse-installed.jpg`: provider installation changes Browse to Installed.
- `sodium-installed-artwork.jpg`: final-release Sodium artwork, Managed ownership and native version-qualified declaration.
- `sodium-only-removal-preview.jpg` / `sodium-removed.jpg`: native one-file cleanup preview and absent test mod.
- `home-3d-restored-final.jpg`: final release, original selection restored, Ready and active Spxcterr.
- `sodium-game-menu-observed.jpg`: actual Aurora/Minecraft menu observed while it occluded the launcher; not launcher evidence.
- `sodium-exit-zero-ready.jpg` / `.txt`: native process exit 0 and returned readiness.
- `sodium-installed-provenance.json`, `sodium-installed-hashes.json`, `sodium-root-cause.json`: exact identity, hashes and nested-module diagnosis.
- `rust-tests-final.txt`, `rust-check-final.txt`, `frontend-tests.txt`, `frontend-check.txt`, `production-build-final.txt`: final automated/build results.
- `restoration-proof.json`: five/seven controlled baseline, eleven production mods, byte-identical provider/account/registry records and unchanged appearance.
- Baseline inventories/hashes and preservation audit scripts: non-secret proof of retained repository/instance state.

## Deferred scope

No Forge, NeoForge, Quilt, CurseForge, modpack compatibility framework, cross-process locking, durable crash recovery, animation engine or unrelated feature work began. Embedded Local/Unknown JAR icons remain deferred. No publication or push is authorized or performed.
