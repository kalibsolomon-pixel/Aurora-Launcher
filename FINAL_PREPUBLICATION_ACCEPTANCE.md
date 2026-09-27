# Final pre-publication UX and content lifecycle acceptance

Local acceptance on 2026-09-26 (America/New_York). Nothing was pushed, published, installed over the existing installed launcher, amended, squashed or rebased. This extends the existing Phase D work; `PHASE_D_ACCEPTANCE.md` remains its historical record. Current policy is documented at the beginning of `ARCHITECTURE.md`.

## Git

Starting HEAD: `793ae3466f8800544b8fdfbb3652203aedd2f1a3`. Fetched origin/main and merge-base: `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`. Starting ahead/behind: **13/0**. All thirteen existing unpublished commits remain intact.

New implementation commits:

- `99d99c3059a7d7e3b26c2b422c50ed559af93317` — Make Aurora bootstrap content user controllable with verified mod toggles.
- `5ed2a7e45b7a5da243d4236638c845c0e95f4466` — Add contained native deletion for stopped registered instances.
- `79949fd423d4cb68fa869b3200e455ad2db12e66` — Refine Home picker and default compatible creation to Aurora Client.
- `800cc242848544cde2c12996f3df8feb6a3a170e` — Present compact mod rows and refresh actual Aurora content state.
- `56cabda87f9b085627e9350d10949091a7955064` — Expand instance settings with maintenance, deletion and wider composition.

`be5e741d05f8789608a8138f086aaf180c350afe` contains this acceptance record and aligned architecture/design/README documentation. A seventh documentation-only commit updates the final-state audit for the user's continued manual account/mod additions; its hash and final HEAD are supplied in the final delivery, avoiding a self-referential commit hash inside the commit. Final ahead/behind after that commit is **20/0**, subject to the final remote audit. No push or publication is authorized or performed.

## Aurora ownership redesign

Previously the exact Aurora Client and Fabric API installation artifacts were permanently Launcher Required/Protected. Registered-instance bootstrap provenance now records original installation identity while permitting normal user disable, re-enable and removal. The concrete installed release pin remains available for deliberate reinstall and three-way game/release consistency; it does not claim a removed or disabled mod is active.

Migration is an idempotent interpretation of strict existing `aurora-installed.json` evidence. No artifact or ownership document is duplicated or rewritten on inventory, startup, validation or Play. Exact original bytes, size and expected SHA-256 establish Launcher Bootstrap identity at the original or `.disabled` name. Modified or ambiguous bootstrap files remain Unknown with actions blocked. Retained former bootstrap evidence follows the same verified user-control rule. Legacy unregistered internal fixtures retain their old required-file behavior; application mutations require real registry membership.

Provider files remain Provider Managed only with verified durable provider provenance. A legitimate provider replacement keeps its provider identity; manual files are not fabricated into Modrinth records. Unknown, linked or unsafe files are never adopted or overwritten. Strict malformed/unknown-schema documents still fail deliberately.

Play and read-only validation do not restore missing Aurora. Fabric can run without Aurora, while known active dependency failures block readiness or unsafe mutations. Explicit Settings reinstall is the restoration path: it uses reviewed artifacts, verified stores, contained staging and no-clobber activation. Disabled bootstrap counterparts must first be re-enabled; modified conflicts require inspection. A verified compatible provider Fabric API replacement is preserved rather than duplicated or relabeled during restore. C2 preview/fingerprint/rollback behavior remains explicit.

## New-instance behavior

The concise checkbox is **Aurora Client**. Compatible Fabric selections default on; Vanilla and unsupported combinations stay off. Pending compatibility prevents creation. Explicit off survives selecting Vanilla and returning to Fabric. Existing instances are never enabled by this default. Native omitted-choice handling also uses reviewed compatibility; an explicit false wins.

Exact reviewed production identity:

- Channel `stable`; Aurora Client `2.1.2`; Minecraft `1.21.11`; Fabric Loader `0.19.5`; Java major `21`.
- Aurora JAR: 2,450,086 bytes; SHA-256 `55ac97f7494daa3866bb3b4aa8d23e49b240fe5ced00fbf1742f7214fc77c52a`.
- Fabric API `0.141.6+1.21.11`: 2,426,039 bytes; SHA-256 `bdff7fd7e220085cfad2ff9b1f40dde6534ae0b96cf378f97a374bc54cb9ed0f`.
- Authority remains the checked-in reviewed `src-tauri/production/aurora-releases.json`; no release service, automatic upgrade or guessed provider identity was added.

Live creation produced Ready instance `b84c28a73c2a4997a238524c08b232fb` named **Final UX disposable**, with precisely these pins and artifacts. The path used the opaque identifier, never the display name. This instance was deleted during the authorized acceptance session.

## Mod toggling

Disabled files use `name.jar.disabled`. Provider records retain canonical enabled filenames, hashes, project/version identity and dependency edges; validation resolves one exact active or inactive counterpart. A strict schema-1 `mods-disabled.json` receipt records disabled-byte SHA-256 for restart integrity. Changed receipt bytes become Unknown and cannot be activated. Opaque entry tokens also include current hash/size/mtime evidence and are checked again before mutation.

Local, bootstrap and verified provider JARs can toggle. Active root/nested Fabric requirements and recorded provider edges block disabling/removing their dependencies; re-enable checks known missing requirements. Alternative active declarations can satisfy identity requirements. Version expressions remain descriptive at the existing bounded metadata boundary; this does not introduce a new dependency solver. Provider removal also checks active local/bootstrap dependents outside its graph at preview and commit. Disabled provider removal is supported; updating a disabled target asks the user to re-enable it first.

Mutation is process-local and no-clobber: same-directory hard link, removal of source name, atomic receipt commit, then rescan. Ordinary commit/rescan errors restore the previous name, bytes and receipt; deterministic injected-failure coverage verifies rollback. Instance-owned files are never linked to shared cache content. Cross-process coordination and a durable crash journal remain deferred.

Live disabling Aurora and AppleSkin preserved all eleven checked controlled-instance artifact/provenance files, accounting for their disabled names. Restart retained both disabled states. Re-enable restored all eleven original names/bytes/provenance checks. Fabric API first showed the active AppleSkin/Aurora dependency blocker, then successfully disabled and re-enabled once those dependents were inactive.

## Installed Mods redesign

Compact rows prioritize artwork, name/version and quiet author metadata. Real installed Modrinth artwork remains backed by verified provider records; bootstrap entries keep honest local fallback artwork. The switch and overflow sit at the right. Filename, size, provenance and dependencies are disclosed in Details; provider check/update/removal actions remain available on demand. Enabled bootstrap/provider rows no longer carry permanent Required/Protected/No toggle clutter. Unknown/blocked actions still explain their actual reason. Running mutations show that changes apply on the next launch.

## Instance deletion

The narrow native command receives only validated instance identity and exact confirmed display name, runs on a blocking worker, and derives the fixed managed root. It holds content, registry and stopped-process boundaries; Ready registry membership is required and Starting/Running children are refused. Full-tree preflight rejects symlinks, Windows reparse points/junctions, non-directory roots and redirected managed directories before recursive removal. Shared cache, runtime, account and unrelated-instance paths are outside the target.

Selected deletion atomically clears selection to None; no arbitrary replacement is chosen. Filesystem failure retains the row and remaining files, attempts selection restoration and reports failure. Registry-write failure after successful root removal retains the row and reports failure; retry safely completes the registry step for that absent exact root. Tests cover name mismatch, busy/malformed state, Starting/Running, invalid roots, selected-instance preservation and registry-commit failure/retry.

The live disposable root and registry row were absent after deletion, selection was None, and every **5,337** pre-existing shared cache/runtime file path remained present. Manual UI input completed that deletion and then continued deleting the older acceptance instances. The agent stopped UI input, reported the concurrency, and the user explicitly instructed: **“Use the captured successful launch; leave UI to me.”** The agent did not restore those intentionally deleted acceptance instances or interfere with subsequent UI preferences.

## Settings

Instance Settings exposes existing real General, Performance, managed Java and Display fields in a two-column editor, alongside Content and Maintenance tools. Content links to Mods/packs/shaders and the native mods folder action; Maintenance exposes validation, managed Java checking/install and a deliberately disclosed configured-content reinstall. The separate deletion danger area explains worlds/mods/packs/config/log retention consequences and requires the exact current name in a modal. Native enforcement remains authoritative even if a stale UI submits an action.

The original configured release and current active/disabled/missing/modified Aurora content are separate visible facts. Nothing invents system/custom Java discovery, syncing, export/import, wrapper commands, additional loaders or unsupported settings.

## Spatial design

| Screen | Composition |
| --- | --- |
| Home | Readable 1480 px bound with balanced launch/player columns; integrated custom listbox preserves the existing static 3D player. Card overflow allows the complete popover to remain visible. |
| Instances | 1500 px bound with practical creation and registry columns; fields use available width without stretching one giant input. |
| Instance Workspace | 1500 px contextual content width; tabs, readiness and content regions share the existing shell. |
| Installed Mods | Compact full-width aligned rows with identity and actions; secondary technical details collapse. |
| Modrinth Browse | Two result columns above 1150 px; narrower windows return to one column. |
| Settings | Global sections share a 1360 px composition; instance editor/tools occupy separate regions and stack at responsive breakpoints. |

Pandora screenshots informed grouping and density only. Aurora's existing surfaces, typography, spacing/accent tokens, brand, focus treatment and shell remain authoritative. Live wide-window Home, Instances, Workspace, Mods and instance Settings were inspected; a narrow Settings layout was also inspected. Global Settings/Browse composition was built and type-checked but was not separately captured in this final UI session. The Home listbox passed observed mouse and Down/Enter selection plus Escape dismissal/focus return. The 80-option/long-name rendering test passed. Home/End/typeahead/outside/Tab handlers are implemented, but these were not all separately exercised live.

## Live acceptance and final state

1. Existing registered bootstrap entries became user-controllable through evidence-only migration; original file hashes/provenance were preserved.
2. Active Fabric API disable showed **AppleSkin requires fabric api; Aurora Client requires fabric api** and left files untouched.
3. Aurora and AppleSkin disabled and remained disabled across production-launcher restart. A real Fabric menu was reached without either mod in the captured Minecraft log; normal Quit returned Ready with captured exit code **0**.
4. Fabric API toggled after its active dependents were disabled. All three mods were then re-enabled with original bytes and names.
5. The enabled production-like Aurora launch reached the actual Aurora menu. The log contains `aurora 2.1.2`, AppleSkin and **Aurora Client ready.** Running/duplicate-Play exclusion was visible. Normal Quit returned Ready. The enabled log was copied at the menu and does not contain a final shutdown footer; the retained explicit exit-code screenshot belongs to the disabled Fabric run.
6. Ordinary Aurora removal left its JAR absent and showed missing content. Restart left it absent while Fabric remained Ready; no automatic restoration occurred.
7. Explicit Settings restore returned Aurora to active with its original JAR SHA-256. Ten of eleven original controlled artifact/provenance checks remained byte-identical; only `aurora-installed.json` received the expected new explicit-install timestamp/installation ID. Provider state, Fabric API and user mods were unchanged.
8. New default/compatibility/explicit-off checks and creation succeeded. Disposable deletion completed with the shared-resource preservation results above.
9. The user then manually deleted the three older acceptance instances, restored selection to the primary `1.21.11` instance, and changed appearance to OLED/custom `#7300d1`. The agent honored the instruction to leave the UI to the user and used the already captured successful launch instead of creating or launching further instances.

Primary preservation uses the takeover snapshot, including the user's intervening mod changes, rather than an older pre-acceptance snapshot. At the end of agent UI work all **18** checked primary/account files matched that snapshot. During the user's continued manual work, the account registry grew from two to three accounts and provider state grew from twelve to eighteen entries. The final audit therefore has **16 byte-identical files**: all fourteen original JARs, the original Aurora installation document and disabled-state receipt. The two changed documents contain the user's additions: both original account records remain exactly intact with the same selected account, and all twelve original provider project/version identities, filenames and hashes remain present. The agent did not overwrite or roll back either document. Primary instance identity, release and prior user mods were preserved. The remaining instance is the user's primary Ready instance; selected instance is `413e831999bd444a8e0df2d22fc6d134`. Appearance changes, new accounts/mods and acceptance-instance deletions are deliberate user changes, not agent cleanup to be rolled back. No extra disposable instance remains.

The enabled Minecraft log contains nonfatal existing content diagnostics: AppleSkin's ModMenu integration error, its absent optional JEI mixin target, and an out-of-range game brightness option. The actual Aurora menu still loaded. These user-content diagnostics were not silently modified or claimed fixed by this launcher pass.

## Tests and builds

| Check | Result |
| --- | --- |
| Rust library | 488 passed, 16 ignored, 0 failed |
| Rust integration | 6 passed, 0 ignored, 0 failed |
| Rust total | **494 passed, 16 ignored, 0 failed**; doc/main targets had zero tests |
| Frontend | **102 passed, 0 failed**, 0 skipped/cancelled |
| Svelte/TypeScript | **0 errors, 0 warnings** |
| Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` passed |
| Rust all targets | `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` passed |
| Production frontend | Static production build passed, including the final picker clipping fix |
| Tauri | Optimized application and both installer bundles built successfully |
| MSI | `C:\Dev\aurora-launcher\src-tauri\target\release\bundle\msi\Aurora Launcher_1.0.0_x64_en-US.msi` |
| NSIS | `C:\Dev\aurora-launcher\src-tauri\target\release\bundle\nsis\Aurora Launcher_1.0.0_x64-setup.exe` |
| Version contract | Launcher/package/native/installer **1.0.0**; separate Aurora Client **2.1.2** |
| Production boot | Exact built release executable booted repeatedly with real persisted data and account session |

The final post-build code adjustment was a documentation-only Rust module comment; format and all-target check passed again. No executable behavior changed after the tested production build. Signing in was not re-automated and no credential or launch argument was emitted as evidence.

## Screenshots

Every path below is under the local evidence directory `C:\Users\kalib\AppData\Local\Temp\aurora-final-pass-20260926-204057\screenshots\`. Images are raw returned captures, not generated mockups. Screenshots 12 and 28 reflect the manual-input portion of the authorized session; 12 shows the actual typed-name guard for the older controlled acceptance instance, rather than pretending it is the earlier disposable confirmation. Some screenshots share a capture to prove two related requirements.

| Evidence path | What it proves |
| --- | --- |
| `01-home-closed-picker.png` | Integrated closed picker and balanced Home. |
| `02-home-open-picker.png` | Styled popover, all four original rows visible, selected marker. |
| `03-home-3d-composition.png` | Existing 3D player retained beside the launch card. |
| `04-new-instance-aurora-default.png` | Compatible Aurora Client checkbox on by default. |
| `05-installed-mods-compact.png` | Compact installed row hierarchy and switches. |
| `06-aurora-disabled.png` | User-controllable Aurora disabled. |
| `07-enabled-mods-artwork.png` | Real provider artwork on enabled compact rows. |
| `08-fabric-api-dependency-blocker.png` | Active dependency refusal with real dependent names. |
| `09-ordinary-mod-disabled.png` | AppleSkin ordinary provider toggle. |
| `10-expanded-instance-settings.png` | General/Performance/Java/Display plus tools. |
| `11-delete-danger-area.png` | Deliberate deletion warning separate from maintenance. |
| `12-delete-confirmation.png` | Exact-name guard while the user manually cleaned up acceptance instances. |
| `13-broader-instances-page.png` | Creation and registry columns at wide size. |
| `14-broader-instance-workspace.png` | Wide real readiness/configuration workspace. |
| `15-large-window-installed-mods.png` | Wide compact installed list. |
| `16-fabric-disabled-mods-menu.png` | Real Minecraft/Fabric menu without Aurora and AppleSkin. |
| `17-disabled-launch-exit-zero.png` | Ready after supervised normal exit, code 0. |
| `18-disabled-after-restart.png` | Persistent disabled state after production boot. |
| `19-fabric-api-user-control.png` | API disabled after dependents were inactive. |
| `20-running-duplicate-play-disabled.png` | Running status and duplicate Play disabled. |
| `21-reenabled-aurora-menu.png` | Actual Aurora menu after re-enable. |
| `22-aurora-removal-confirmation.png` | Ordinary bootstrap removal confirmation. |
| `23-aurora-removed.png` | Missing row/file after removal. |
| `24-removed-after-restart.png` | No automatic restoration on restart. |
| `25-explicit-restore-workflow.png` | Deliberately disclosed Settings restore action. |
| `26-restored-aurora-active.png` | Successful explicit restore and active content state. |
| `27-explicit-opt-out-preserved.png` | User off choice survived Vanilla→Fabric round trip. |
| `28-disposable-deletion-complete.png` | Disposable absent from the instance list before further user cleanup. |

Other evidence at `C:\Users\kalib\AppData\Local\Temp\aurora-final-pass-20260926-204057\` includes repository baseline/status/audit files, test/build logs, disabled/enabled Minecraft log excerpts, hash comparisons and deletion preservation results. No tokens or child argument arrays were collected. Evidence and bundles are outside Git staging; no generated assets are committed.

## Repository safety

Initial tracked baseline: **159** files. Protected initial untracked diagnostic files: **607**. Each commit was preceded by baseline presence/hash and staged name-status/deletion/generated-file audits. **Zero** baseline tracked files disappeared, **zero** protected diagnostics changed, **zero** tracked deletions were staged, and **zero** generated build/game/runtime/screenshot paths were staged. Root documentation, manifests, lockfiles and build configuration remain present. Dependency and lockfile additions were unnecessary.

The final full status, diff check, baseline comparison and remote fetch are saved in the local evidence directory and summarized in delivery. All original diagnostic files remain untracked. New tracked files are only the implemented deletion domain and this acceptance record; there is no unexpected repository mutation.

## Deferred work

No Forge, NeoForge, Quilt, CurseForge, modpack installation, automatic dependency-conflict resolution, cloud sync, import/export, unrelated launcher update, cross-process mutation locking or durable power-loss/crash journal work began. No next roadmap phase was started. This remains a local pre-publication review stop.
