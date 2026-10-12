# Minecraft ecosystem compatibility and evidence matrix

F0 baseline: `9890a9c8ba2b0fb64508df60d9ea8fd860a21f64`, reviewed 2026-10-10–11. This is a bounded evidence matrix, not a universal compatibility promise. **No game/runtime installation, authentication or launch was performed in F0.** Current source, prior acceptance and newly inspected upstream metadata are separate evidence tiers.

## Status definitions

The four status labels are used exactly:

| Status | Meaning here |
| --- | --- |
| **Verified in Aurora** | A matching exact combination has recorded real Aurora native acceptance, identified below. It is prior acceptance, not a fresh F0 rerun or a guarantee for all mods/hosts |
| **Supported by upstream, not yet implemented** | First-party metadata/source establishes the mechanism or exact upstream combination, but Aurora has no complete implementation for it. This does not claim a successful Aurora launch |
| **Research required** | Partial implementation/component acceptance or metadata exists, but full exact combination/host acceptance or relevant upstream behavior remains unproven |
| **Unsupported** | Current Aurora deliberately rejects the shape/configuration/platform, or the requested capability is incompatible. Some rows are possible future scope, not permanently impossible |

Host shorthand: **W64** Windows x86_64; **WA64** Windows ARM64; **W32** Windows x86; **L64** Linux x86_64; **L32** Linux x86; **LA64** Linux ARM64; **M64/MA64** macOS x86_64/ARM64. Only W64 has the cited full authenticated Aurora native evidence. Mojang Java availability on another host does not prove loader/native/mod/credential-store support. Linux/macOS OS-backed refresh credential stores are not implemented, preventing a full supported authenticated lifecycle there.

## 1. Representative game/loader combinations

Java below is the **official base-game** requirement, subject to separately proved loader/tool constraints. Exact tool JDK requirements must not silently replace it. OS column describes current Aurora evidence or a prerequisite, not upstream universal portability.

| Family / exact example | Minecraft | Java component / major | Installation and launch mechanism | OS evidence | Status | Limit / evidence |
| --- | --- | --- | --- | --- | --- | --- |
| Vanilla | 1.7.10 / 1.12.2 | `jre-legacy` / 8 | Legacy game argument string and classifier-based natives | None in Aurora | **Unsupported** | Current metadata boundary deliberately rejects legacy arguments/classifiers; official version JSON inspected |
| Vanilla | 1.16.5 | `jre-legacy` / 8 | Structured arguments, legacy native classifiers | None in Aurora | **Unsupported** | Native classifier/extract normalization absent; Java8 feed existence is insufficient |
| Vanilla | 1.17.1 | `java-runtime-alpha` / 16 | Structured arguments, legacy native classifiers | None in Aurora | **Unsupported** | Same base-shape rejection; exact component absent on ARM64 |
| Vanilla | 1.18.2 | `java-runtime-beta` / 17 | Structured arguments, legacy native classifiers | None in Aurora | **Unsupported** | Explicit current rejection, requires F8H |
| Vanilla | 1.20.1 | `java-runtime-gamma` / 17 | Current official normalized game plan | W64 target; no exact native evidence reviewed | **Research required** | Official metadata fits current nonlegacy native shape; generic implementation exists, no exact acceptance established |
| Vanilla | 1.21.1 | `java-runtime-delta` / 21 | Current official normalized game plan | W64 target; no exact native evidence reviewed | **Research required** | No evidence that this exact combination was launched in Aurora |
| Vanilla | 1.21.11 | `java-runtime-delta` / 21 | Dedicated vanilla plan, shared verified install and native launch | W64 | **Verified in Aurora** | `C2_ACCEPTANCE.md`: genuine menu, supervised exit 0, restart; no Fabric/Aurora files |
| Vanilla | 26.2 | `java-runtime-epsilon` / 25 | Current plan with natives subdirectory semantics | W64 component use via NeoForge | **Research required** | NeoForge's base-game acceptance does not prove a separate exact Vanilla launch |
| Fabric, representative older profiles | 1.16.5 / 1.18.2 | 8 / 17, exact base components above | Fabric Meta composition over historical base | None in Aurora | **Unsupported** | Base metadata rejected today; no historical Fabric test performed |
| Fabric, exact loader to be pinned | 1.20.1 / 1.21.1 | gamma 17 / delta 21 | Official Meta overlay, KnotClient, common/intermediary/loader libraries | W64 target | **Research required** | Generic Fabric adapter exists; choose and accept exact pins before verification |
| Fabric 0.19.5, Aurora off | 1.21.11 | delta / 21 | Current normalized Fabric plan, no mandatory API/Aurora | W64 | **Verified in Aurora** | `C2_ACCEPTANCE.md`: menu/exit 0/restart; optional API handled by normal provider ownership |
| Fabric, exact loader to be pinned | 26.2 | epsilon / 25 | Requires matching official loader/profile and tested plan | Not established | **Research required** | Do not infer support from NeoForge or modern version number |
| Quilt 0.31.0-beta.4 | 1.20.1 | gamma / 17 | Meta v3 overlay, Quilt KnotClient, hashed/intermediary/common libraries | W64 first target; other hosts untested | **Supported by upstream, not yet implemented** | Exact official profile inspected; enum representation only in Aurora, no executable adapter |
| Quilt, exact supported loader not accepted | 1.21.1 / 1.21.11 | delta / 21 | Separate official Quilt profile and metadata semantics | Not established | **Supported by upstream, not yet implemented** | Per-game official profile metadata exists; no general Fabric compatibility guarantee |
| Quilt | 26.2 and historical generations | epsilon 25 or generation-specific | Exact profile, native and mod compatibility still to establish | Not established | **Research required** | Loader/game catalog availability alone is insufficient |
| Forge 1.7.10-10.13.0.1150 | 1.7.10 | legacy / 8 | Legacy `install` + `versionInfo`, LaunchWrapper/FMLTweaker | No Aurora host acceptance | **Unsupported** | Current base arguments/natives and loader execution unsupported; official artifact inspected, later H4 candidate |
| Forge 1.12.2-14.23.5.2864 | 1.12.2 | legacy / 8 | Spec0 installer, embedded inheriting version JSON, legacy args, zero client processors in this sample | No Aurora host acceptance | **Unsupported** | Requires legacy argument/native support and exact generation adapter |
| Forge 1.16.5-36.0.0 | 1.16.5 | legacy / 8 | Spec0, ModLauncher, four client mapping/patch processors | No Aurora host acceptance | **Unsupported** | Current base/native and Forge adapter unavailable; not “same as 1.12” |
| Forge 1.18.2-40.3.12 | 1.18.2 | beta / 17 | Spec1, BootstrapLauncher/module arguments, six client processors | No Aurora host acceptance | **Unsupported** | Base native-classifier work plus mapping adapter required |
| Forge 1.20.1-47.4.26 | 1.20.1 | gamma / 17 | Spec1 six-client-step mapping/rename/patch pipeline; module-path BootstrapLauncher | W64 proposed first host | **Supported by upstream, not yet implemented** | Official artifact/hash/profile inspected; F1C target, current base shape suitable |
| Forge 1.20.1-47.4.10 | 1.20.1 | gamma / 17 | Spec1 six client steps, BootstrapLauncher / forgeclient | W64 native install, deep Ready and real Java17 validated | **Research required** | F1C implemented and offered exact recommended pin; authenticated menu/world/exit acceptance pending owner |
| Forge 1.20.4-49.2.0 | 1.20.4 | gamma / 17 | Spec1 five client steps, ForgeBootstrap / forge_client | W64 native install, deep Ready and real Java17 validated | **Research required** | F1C implemented and offered exact recommended pin; authenticated menu/world/exit acceptance pending owner |
| Forge 1.21.11-61.2.0 | 1.21.11 | delta / 21 | Spec1 three client steps, ForgeBootstrap / forge_client | W64 | **Verified in Aurora** | F1B owner-reported authenticated gameplay/normal exit/Ready; F1C repeated native install/deep validation, not another gameplay run |
| Forge 1.21.1-52.1.16 | 1.21.1 | delta / 21 | Spec1 ForgeBootstrap, three client processors | W64 proposed first host | **Supported by upstream, not yet implemented** | Different bootstrap/pipeline from 1.20.1; separate fixture/acceptance required |
| Forge 26.2-65.1.3 | 26.2 | epsilon / 25 | Spec1 ForgeBootstrap/forge_client, one client patch processor | W64 proposed first host | **Supported by upstream, not yet implemented** | F1B research pin; Forge is represented but absent from executable catalog |
| NeoForge 21.1.1 | 1.21.1 | delta / 21 | Spec1 six-client-step mapping chain, BootstrapLauncher | W64 proposed first host | **Supported by upstream, not yet implemented** | Official profile inspected; current NeoForge discovery/execution deliberately gates it out |
| NeoForge 1.20.x generations | Exact MC/version required | Usually generation-dependent 17 or 21; resolve document | Version-specific installer/profile conventions | Not established | **Research required** | Do not extrapolate from 21.1.1 or map Forge IDs automatically |
| NeoForge 26.2.0.88 | 26.2 | epsilon / 25 | Current verified installer + PROCESS_MINECRAFT_JAR; NeoForge Client main class | W64 | **Verified in Aurora** | `PHASE_K_NEOFORGE.md`: AppleSkin, authenticated menu, repeat launch, exits 0 and restart |
| NeoForge 26.3.0.41-beta | 26.3 | epsilon / 25 in prior Phase K record | Existing 26.x adapter, exact prerelease pin | W64 installation evidence | **Research required** | Phase K records install/runtime acceptance; do not promote it to full launch acceptance from 26.2 |
| Aurora 2.1.2 + Fabric 0.19.5 + API 0.141.6+1.21.11 | 1.21.11 | delta / 21 | Optional exact Aurora SHA-256 tail on Fabric game | W64 | **Verified in Aurora** | `C2_ACCEPTANCE.md` completion: Aurora initialized, genuine menu, supervised exit 0 |
| Aurora 3.0.0 + Fabric 0.19.5 + API 0.141.6+1.21.11 | 1.21.11 | delta / 21 | Current reviewed production source; exact SHA-256 Client/API artifacts | W64 component/creation evidence | **Research required** | Client correction proves Ready creation/runtime and shipped bridge interoperability; no matching full authenticated game acceptance established in that report |
| Aurora + Vanilla / Forge / NeoForge / Quilt | Any | Not applicable | No compatible Aurora release authority | None | **Unsupported** | Backend exact optional-Aurora compatibility restricts Aurora to reviewed Fabric combinations |
| Old alpha/beta game types, arbitrary inheritance or unknown metadata shapes | Historical/unrecognized | Not inferred | No supported normalization | None | **Unsupported** | Deliberate rejection; future historical slices do not imply these are enabled |

The examples include old and modern Forge artifacts on purpose. They are not asserted to be newest or recommended versions. Explicitly unsupported rows may move through research and implementation in F8H; they cannot become Verified from a metadata fetch.

## 2. Java/runtime platform matrix

Source: current [official Mojang runtime index](https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json), inspected in F0. Entries refer to availability, **not newly installed/validated runtimes**. `beta` and `gamma` are both Java17 but different required components; never substitute one for another. Each real selection still requires exactly one matching entry and expected SHA-1 manifest/files.

| Runtime component / major | W64 | W32 | WA64 | L64 | L32 | M64 | MA64 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `jre-legacy` / 8 | Available, 8u51 | Available, 8u51 | Absent | Available, 8u202 | Available, 8u202 | Available, 8u74 | Absent |
| `java-runtime-alpha` / 16 | Available | Available | Absent | Available | Absent | Available | Absent |
| `java-runtime-beta` / 17 | Available | Available | Absent | Available | Absent | Available | Absent |
| `java-runtime-gamma` / 17 | Available | Available | Available | Available | Absent | Available | Available |
| `java-runtime-delta` / 21 | Available, 21.0.7 | Absent | Available, 21.0.7 | Available, 21.0.7 | Absent | Available, 21.0.7 | Available, 21.0.7 |
| `java-runtime-epsilon` / 25 | Available, 25.0.1 | Absent | Available, 25.0.0 | Available, 25.0.1 | Absent | Available, 25.0.1 | Available, 25.0.1 |

| Host/lifecycle | Status | Actual limit |
| --- | --- | --- |
| W64 managed Java21 for cited 1.21.11 instances and Java25 for NeoForge26.2 | **Verified in Aurora** | Prior C2/Phase K validation/launch evidence, not all game/runtime versions |
| W64 Java8/16/17 general managed plan | **Research required** | Parser/feed can represent these; exact runtime/game acceptance must be performed; old base shapes may block game before runtime selection |
| W64 exact gamma Java17 runtime for F1C Forge1.20.1/1.20.4 | **Research required** | F1C acquired and verified exact manifest/files plus bounded real java-version diagnostic; both Ready. Full authenticated game acceptance pending; no beta/gamma substitution |
| WA64, W32, L64/L32, M64/MA64 exact runtime installation | **Research required** | Explicit mapping and feed availability are necessary, not full native acceptance; Windows links and Unix mode/link rules also apply |
| Linux/macOS full authenticated launcher lifecycle | **Unsupported** | OS-backed credential store deliberately unavailable today; no plaintext fallback |
| LA64 runtime resolution | **Unsupported** | No current explicit Mojang platform mapping; no fallback to x64 |
| Missing exact component/platform, arbitrary system Java/custom executable | **Unsupported** | Exact failure; Java path configuration/system lookup is not implemented |

## 3. Content and pack scope

| Capability | Current status | Limits / evidence |
| --- | --- | --- |
| Modrinth mods on accepted Fabric1.21.11 | **Verified in Aurora** | Existing provider/lifecycle acceptance; exact SHA-512, native compatibility and local ownership still required for every file |
| Modrinth NeoForge26.2 AppleSkin | **Verified in Aurora** | Phase K exact AppleSkin artifact/version, source identity, game initialization and restart |
| Local mods on supported family | **Research required** | Inventory, toggles, bounded parsing and local removal exist; arbitrary mod compatibility cannot be guaranteed; current Neo TOML semantics have identified gaps |
| Local resource/shader ZIP management | **Research required** | Existing native inventory/remove/enable behavior; a shader file does not install or verify Iris/Oculus/another rendering loader |
| Modrinth Fabric pack install/update workflows: Sodium Booster2.0.0 and2.1.3→2.2 | **Verified in Aurora** | Component/workflow evidence only: Phase I install/restart on1.21.11/Fabric0.19.3; Phase J update/divergence on26.2/Fabric0.19.3. Minecraft launch explicitly not run in those phases; not universal pack/game compatibility or generic import/export |
| NeoForge `.mrpack` and generic non-Fabric packs | **Unsupported** | Parser/resolver explicitly rejects current unsupported loader keys before creating an instance |
| CF discovery and authorized automatic content acquisition | **Supported by upstream, not yet implemented** | Approval/key/use/cache model unresolved; current API/CDN requirements apply; no authenticated API acceptance in F0 |
| CF restricted/manual file acquisition | **Research required** | Compliant proposed explicit file grant/hash flow requires written policy confirmation and expected SHA-1; never bypass restrictions |
| CF manifest-v1 ZIP packs | **Supported by upstream, not yet implemented** | Source-backed manifest references/overrides; Aurora schemas/executor require extension; exact loader/submission eligibility separately verified |
| General local CF/mrpack import, reviewed legal export, source transfer | **Research required** | Proposed work; current exact remote Modrinth pack install is not proof of these operations |
| Cross-provider name-based adoption/automatic source switching | **Unsupported** | Violates exact identity, provenance and ownership requirements |
| MD5-only/fingerprint-only managed acquisition | **Unsupported** | No acceptable expected cryptographic digest under proposed policy; local SHA-256 observation does not repair missing publisher evidence |
| General crash-safe repair/history rollback | **Research required** | Explicit retry/reinstall, runtime replacement and pack recovery exist, but no universal durable transaction or user-world rollback |

## 4. Exact first-party game document evidence

F0 fetched each listed URL from the pinned official version manifest and verified the bytes against its published SHA-1 before inspecting shapes. Hashes below are **metadata document SHA-1**, not client JAR hashes or SHA-256 cache IDs.

| Minecraft | Metadata SHA-1 / direct URL | Shape finding |
| --- | --- | --- |
| 1.7.10 | [ed5d8789ed29872ea2ef1c348302b0c55e3f3468](https://piston-meta.mojang.com/v1/packages/ed5d8789ed29872ea2ef1c348302b0c55e3f3468/1.7.10.json) | Legacy arguments; 4 libraries with native/classifier/extract shapes |
| 1.12.2 | [832d95b9f40699d4961394dcf6cf549e65f15dc5](https://piston-meta.mojang.com/v1/packages/832d95b9f40699d4961394dcf6cf549e65f15dc5/1.12.2.json) | Legacy arguments; 5 such libraries |
| 1.16.5 | [fba9f7833e858a1257d810d21a3a9e3c967f9077](https://piston-meta.mojang.com/v1/packages/fba9f7833e858a1257d810d21a3a9e3c967f9077/1.16.5.json) | Structured arguments; 16 such libraries |
| 1.17.1 | [e0e7ab5ed6f55bbd874ef95be3c9356d67e64b57](https://piston-meta.mojang.com/v1/packages/e0e7ab5ed6f55bbd874ef95be3c9356d67e64b57/1.17.1.json) | Java16; 16 such libraries |
| 1.18.2 | [334b33fcba3c9be4b7514624c965256535bd7eba](https://piston-meta.mojang.com/v1/packages/334b33fcba3c9be4b7514624c965256535bd7eba/1.18.2.json) | Java17 beta; 16 such libraries |
| 1.20.1 | [760a390e336b2b6202ec5906757cee3a929a7c85](https://piston-meta.mojang.com/v1/packages/760a390e336b2b6202ec5906757cee3a929a7c85/1.20.1.json) | Java17 gamma; none of these legacy native shapes |
| 1.21.1 | [cedfc3b6dcbca34e2b478d498bf1d56a8fa2f404](https://piston-meta.mojang.com/v1/packages/cedfc3b6dcbca34e2b478d498bf1d56a8fa2f404/1.21.1.json) | Java21 delta; current shapes |
| 1.21.11 | [6827f8e998ee6add1b3acff9286b6c95ebeebdc1](https://piston-meta.mojang.com/v1/packages/6827f8e998ee6add1b3acff9286b6c95ebeebdc1/1.21.11.json) | Java21 delta; current shapes |
| 26.2 | [d367f3dfbc0b3e14688df2311359deb609b234e3](https://piston-meta.mojang.com/v1/packages/d367f3dfbc0b3e14688df2311359deb609b234e3/26.2.json) | Java25 epsilon; component-specific native path templates |

Installer hashes, generation observations and official Maven URL conventions are in [audit sections 8–9](FORGE_CURSEFORGE_F0_AUDIT.md#8-forge-research-distinct-installer-generations). Quilt exact research profile and dated primary sources are in the same audit. A spec number, available runtime, matching family name or download success never changes a row to Verified.

## 5. Acceptance evidence and promotion rules

| Evidence | What it proves | What it does not prove |
| --- | --- | --- |
| `C2_ACCEPTANCE.md`, live sections and completion | Exact W64 Vanilla/Fabric1.21.11 and historical Aurora2.1.2 authenticated menus/exits/restart | Current Aurora3.0.0 full game acceptance, other MC versions/hosts or arbitrary mods |
| `PHASE_K_NEOFORGE.md`, acceptance and completion | Exact W64 NeoForge26.2.0.88 + AppleSkin authenticated launch/repeat/exit/restart; separate 26.3 beta install | Older NeoForge pipeline, all26.x versions, NeoForge packs or Forge |
| `PHASE_I_ACCEPTANCE.md`, `PHASE_J_ACCEPTANCE.md` | Exact reviewed Fabric Modrinth pack installation/update/reconciliation paths, including separately disclosed fixture-derived UI states | Minecraft launch was explicitly not run; no proof of CF packs, generic exports, every pack or every crash/power-loss condition |
| `CLIENT_3_0_0_PRODUCTION_CORRECTION.md` | Current3.0.0 artifact identity, isolated Ready creation/runtime reuse and shipped bridge interoperability | Full authenticated3.0.0 game/menu playthrough; that report explicitly records native boot limitation |
| `RELEASE_1_4_2_ACCEPTANCE.md` plus live F0 GitHub checks | Current source/release identities; actual public1.4.2 publication and authority advancement | A new F0 installed self-update test or compatibility acceptance for unrelated loaders |
| `FORGE_F1B_MODERN_FORGE.md`, owner acceptance 2026-10-12 | Exact Forge1.21.11/61.2.0 authenticated launch, gameplay, normal exit and Ready reported by owner | Agent-independent gameplay observation or other Forge generations |
| `FORGE_F1C_COMPATIBILITY_EXPANSION.md` | Exact1.20.1/47.4.10 and1.20.4/49.2.0 native installation, verified outputs, Java17, deep Ready, launch assembly, interruption/retry; five existing-loader deep-validation regressions | Authenticated1.20.x gameplay, other hosts, arbitrary Forge builds, mods or historical generations |

## 6. F1C exact support gates (2026-10-12)

Discovery is broader than installation eligibility. Normal choices are the
three reviewed pins below. The four status labels above remain unchanged:
the two new rows stay Research required until owner gameplay acceptance.
The historical F0 47.4.26 example is not the selected F1C pin and is not offered.

| Exact pin | Discoverable | Recognized profile | Offered/installable | Deep Ready | Authenticated gameplay |
| --- | --- | --- | --- | --- | --- |
| 1.20.1-47.4.10 | Yes, official recommended | Yes, spec1 | Yes | Yes, W64 | Pending owner |
| 1.20.4-49.2.0 | Yes, official recommended | Yes, spec1 | Yes | Yes, W64 | Pending owner |
| 1.21.11-61.2.0 | Yes | Yes, spec1 | Yes | Yes, repeated in F1C | Owner-confirmed in F1B |
| Other discovered Forge builds | Upstream-dependent | Not an acceptance claim | No | Not claimed | Not claimed |

Tier2 inspected pins 1.19.2-43.5.0,1.18.2-40.3.0,1.16.5-36.2.34 and
1.12.2-14.23.5.2859 are research artifacts only. The first two have six-step
spec1 BootstrapLauncher chains; the latter two use distinct spec0 generations.
Historical native/argument normalization remains deliberately unavailable.

To promote a game/loader row: record exact build/game/loader/component/manifest/host, verify installed state and trust, perform a real native authenticated launch and normal supervised exit, restart/revalidate, inspect relevant mod/native/pack behavior and preserve user sentinels. A separately verified component/workflow row proves only that named operation, never an unperformed game launch. Attach evidence and note scope. Failed, skipped, synthetic-session or component-only results stay clearly bounded. Only explicitly accepted rows should be advertised as verified support.
