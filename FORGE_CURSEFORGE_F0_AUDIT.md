# Forge, NeoForge, Quilt and CurseForge: F0 audit

Audit dates: **2026-10-10–11, America/New_York**. Source baseline: `9890a9c8ba2b0fb64508df60d9ea8fd860a21f64`, branch `codex/client-3-production-authority`, `C:\Dev\aurora-launcher`.

**F0 verdict: COMPLETE. Implementation is not authorized by this report.** This audit traces current behavior, investigates upstream mechanisms, and proposes subsequent work. It does not certify universal compatibility. The companion [architecture](FORGE_CURSEFORGE_ARCHITECTURE.md), [roadmap](FORGE_CURSEFORGE_ROADMAP.md), and [compatibility matrix](FORGE_CURSEFORGE_COMPATIBILITY_MATRIX.md) distinguish implemented behavior, prior acceptance evidence, current research, and proposals.

## 1. Executive findings

1. Aurora is already a general-instance launcher. `PlatformPin` represents Vanilla, Fabric, Forge, NeoForge and Quilt; the native capability catalog enables **Vanilla, Fabric and NeoForge**. Aurora Client is an optional, exact Fabric capability. Adding another mandatory Aurora-centric instance model would undo C1/C2.
2. NeoForge is implemented and has recorded authenticated Windows acceptance for **Minecraft 26.2 / NeoForge 26.2.0.88 / managed Java 25**, including AppleSkin, menu, normal exit and restart. Its 1.20/1.21 mapping pipeline and NeoForge packs remain deliberately unavailable. F2 should extend and verify it, not introduce a duplicate executor.
3. Shared installation, caches, managed Java, native extraction, launch assembly, provenance and pack reconciliation are substantial reusable foundations. The main structural defect is that loader-neutral composition lives in `fabric/plan.rs`, with some family-specific branching in shared consumers.
4. Forge requires distinct installer-generation adapters and processor plans. Official samples range from legacy LaunchWrapper to mapping/patch pipelines and modern ForgeBootstrap. It is not a renamed Fabric profile or interchangeable NeoForge version.
5. Processor execution needs hardening before expansion. Current output capture is not bounded during execution; staging snapshots record names rather than original bytes; symlink outputs can be followed; official processor output expectations are discarded. Structured arguments and a scratch working directory do **not** isolate arbitrary Java code from the user's filesystem.
6. Provider-neutral strings do not make the current schemas provider-neutral. Pack identity is restricted to Modrinth IDs, Fabric and mandatory SHA-512; pack-origin content expects a SHA-512-shaped file ID. CurseForge requires typed numeric project/file identities and expected SHA-1 support in the content adapter.
7. CurseForge has both a technical and a permission gate. Its published third-party terms restrict key sharing, caching and competing products. Written authorization covering Aurora's actual use and credential distribution is required. The July 2026 CDN authentication change makes an old anonymous-CDN integration unsuitable.
8. Historical Minecraft needs a separate bounded compatibility program. Official 1.18.2 and earlier samples use deliberately rejected native classifiers; 1.12.2 and 1.7.10 also use legacy argument strings. Java provisioning alone does not enable these versions. Official 1.20.1 uses current base shapes and is a realistic second Forge target.
9. Individual atomic writes and error rollback are not durable multi-file transactions. Cross-process mutation locks, a recovery journal around game replacement, and a plan receipt for cold offline Play are gaps to own explicitly.

## 2. Preflight, preservation and method

Root `AGENTS.md` and `ARCHITECTURE.md`, relevant C1/C2, compatibility, Modrinth pack/update, Phase K, Client correction and release records were read alongside implementation. Historical documentation contradicts later implementations: assertions that only Fabric executes, deletion is absent, production distribution is missing, content schema is 2, or registry schema is 3/4 are superseded by source. They were not rewritten in F0.

Before report creation:

| Item | Baseline |
| --- | --- |
| Tracked files | 809; no tracked or staged modifications |
| Nonignored untracked files | 15,144, preserved as developer property |
| Ignored file inventory | 41,393 paths |
| Protected examples | `.zcode-diag*`, `.zcode-perf`, `.zcode-icon-diag.py`, `.zcodeignore`, unusual root diagnostic filename, `docs/__pycache__` |
| Ignored examples | `node_modules`, `.svelte-kit`, `build`, `src-tauri/target`, `src-tauri/gen`, editor/diagnostic data |
| Critical files | Root documentation; npm/Cargo manifests and lockfiles; Svelte/Vite/TypeScript metadata; Tauri configuration, capabilities and `build.rs` present |

The external baseline is `C:\Users\kalib\AppData\Local\Temp\aurora-f0-audit-yrrax0o8\baseline.json`: exact status and inventories, SHA-256 of every tracked/nonignored-untracked file, and size/mtime of ignored files. Ignored binaries were not all rehashed; this limits the preservation proof accordingly. Research artifacts and metadata were read in memory; a research JSON summary is in that same external temporary directory. No installers or processors were executed.

Evidence levels used below:

- **Source:** behavior directly traced at the baseline commit. Test source is evidence of intended cases, not a fresh passing test run.
- **Prior acceptance:** checked-in dated reports, sometimes with existing local evidence indexes; not rerun in F0.
- **Upstream research:** current first-party documents, source, JSON and SHA-1-checked installer samples; not Aurora installation acceptance.
- **Proposal:** future design requiring implementation, tests and owner review.

No build, test suite, application boot, sign-in, account access, game installation, owner managed-data enumeration, release mutation, push or publication was performed. Documentation-only validation and read-only public/remote queries were performed.

## 3. Release completion and source identities

The preceding release **was published**, despite local recovery documents stopping at a prepared decision. Read-only GitHub API checks establish:

| Identity | Observed state |
| --- | --- |
| Remote `main` | `9890a9c8ba2b0fb64508df60d9ea8fd860a21f64`, matching local HEAD |
| Public Launcher release | `v1.4.2`, release ID `408878140`, nondraft, nonprerelease, immutable; published `2026-10-10T10:55:47Z` |
| Release source/tag | `cde51a00a6dbce47adbde06b361219defd963dfb`; local tag inventory had not fetched v1.4.2 |
| Recovery workflow | Run `38046513129`, successful, recovery HEAD `9890a9c`; original run `38044389751` failed |
| Release assets | Windows x64 NSIS EXE, `.exe.sig`, `release-assets.json`; all present |
| Installer | 11,785,232 bytes; GitHub-reported SHA-256 `2f5c9bc3cb9e7f9464151bef9835ffca26876aca9090cae7113f835fa761da2a` |
| Update authority head | `8b80de0cb1a1f8897bf42c548b2d9182581d4b98` |
| Public update manifest | Version 1.4.2, `windows-x86_64`, exact v1.4.2 EXE URL and nonempty signature |

The [public release](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/v1.4.2) and [unauthenticated authority JSON](https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-update-authority/launcher-update.json) were read. F0 did not download/reverify the installer signature or exercise an installed self-update; publication/authority completion is established, end-to-end update acceptance is not newly established.

Production identities remain separate:

- Launcher: npm/Cargo 1.4.2, Tauri identifier `com.aurora.launcher`; `updates/launcher.rs` owns the fixed signed Launcher update stream and unchanged signing key.
- Client creation source: `distribution.rs` includes `src-tauri/production/aurora-releases.json`. Current reviewed stable **Aurora 3.0.0**, Minecraft **1.21.11**, Fabric **0.19.5**, Java **21**, Fabric API **0.141.6+1.21.11**. Client JAR: 3,077,377 bytes, expected SHA-256 `43f918207a86f91b01b35045309e89d2b040951722e5ac71d01d886beec9811c`. Historical 2.1.5/2.1.3/2.1.2 entries remain exact pins.
- Client updates: `updates/client.rs::PRODUCTION_MANIFEST_URL`, normally `https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/release-manifest/aurora-releases.json`; a build-time diagnostic override exists. HTTPS discovery plus expected artifact SHA-256 is distinct from Launcher signing.
- Development: separate included `src-tauri/development/aurora-releases.json`, schema 1: stable 0.3.0 and beta 0.3.1-beta.1 on Minecraft26.2/Fabric0.19.5/Java25, nightly 0.4.0-nightly.20260915 on Minecraft1.21.11/Fabric0.19.5/Java21. Synthetic JARs are 57/64/65 bytes at explicit loopback port8765; their pre-known SHA-256 expectations remain fixture-only. These are not production offerings.
- Authentication: existing Aurora public-client registration and Rust-owned system-browser flow; this audit did not inspect credentials or use an account. Neither loader nor content work may substitute another application's registration.

## 4. Implementation map and ownership

All paths below are relative to the audited repository; symbols are stable lookup anchors at the recorded HEAD.

| Domain | Main source | Actual responsibility |
| --- | --- | --- |
| Native edge | `src-tauri/src/application.rs`, `lib.rs`; `src/lib/backend.ts` | Typed commands/events, summary DTOs, error mapping; TypeScript mirrors Rust manually |
| Paths/config | `paths.rs`, `config.rs` | Platform application-local root, validated derived paths, atomic known-schema configuration |
| Identity | `instances/mod.rs`, `platform.rs`, `settings.rs` | Registry, opaque IDs, installed platform/optional Aurora, desired settings, capabilities |
| Lifecycle | `instances/lifecycle.rs`, `transition.rs`, `deletion.rs` | Create/retry/install/validate/rename/select, reviewed Aurora transitions, confirmed bounded deletion |
| Mojang | `minecraft/{metadata,plan,rules,mod}.rs` | Official discovery and SHA-1 document validation; pure rules; normalized vanilla plan |
| Fabric/composition | `fabric/{metadata,maven,plan,versions}.rs` | Official loader adapter; currently also shared `GameInstallPlan`, `LoaderPlan`, `GameLibrary` |
| NeoForge | `neoforge/{metadata,plan,processors,mods,versions,mod}.rs` | Official exact discovery, verified installer parsing, narrow 26.x normalization, processors, TOML subset and Maven predicates |
| Acquisition | `downloads.rs`, `cache.rs`, `integrity.rs` | Streamed expected hashes, cache revalidation, SHA-256/SHA-1/SHA-512 and honest transport-observed provenance |
| Game execution | `install/{mod,state,assets,natives}.rs` | Normalized acquisition/materialization, defensive native extraction, staged marker and promotion |
| Java | `runtime/{metadata,plan,install,state}.rs` | Exact official runtime component/platform, verified files, shared install and validation |
| Auth | `auth/*` | Browser PKCE, Xbox/Minecraft exchange, entitlement, memory sessions, Windows refresh-credential store |
| Launch | `launch/{state,boundary,resolve,process,diagnostics}.rs` | Readiness, final locked validation, redacting Rust-only spec, exact-child spawn/supervision |
| Content | `instance_mods.rs`, `instance_content.rs`, `mod_compatibility.rs` | Bounded inventory, ownership, enable/disable, shared provider graph/transactions, local compatibility |
| Providers | `modrinth.rs`, `content_recognition.rs`, `content_updates.rs`, `installed_artwork.rs` | Sole implemented online provider, exact-hash recognition, update policy; artwork is cosmetic |
| Packs | `mrpack.rs`, `modpacks.rs`, `pack_state.rs`, `pack_activation.rs`, `pack_update.rs` | Exact Fabric Modrinth pack install, ownership, three-way updates and recovery receipts |
| Updates | `updates/{launcher,client}.rs`, `tools/release/*`, workflows | Separate Launcher/Client authorities, reviewed Client transactions and gated publication |
| Presentation | `src/lib/launcher/*`, `src/lib/components/*` | Existing Instances/workspace, Browse, settings, native progress/readiness; presentation only |

## 5. Instance lifecycle and schemas

`instances/mod.rs` uses **registry schema 5**, explicit migrations from 2/3/4, strict unknown-schema/malformed refusal and duplicate checks. `InstanceId` validates filesystem-safe identity; new IDs are opaque UUIDs, names never paths. Selection is referentially checked, rename only changes metadata, and stale editor originals are rejected under the registry lock.

`InstalledConfiguration` contains Minecraft version, tagged `PlatformPin`, optional `AuroraPin`. `InstanceConfiguration` contains desired loader/policy/game/Aurora settings plus memory/JVM/window preferences. Automatic selection resolves at installation and stores a concrete pin; it does not auto-upgrade installed instances. Aurora is restricted to exact compatible Fabric releases, not imposed on generic Fabric/Vanilla/NeoForge.

Creation saves `installing` before long work. Readiness requires deep validation before promotion to `ready`; saved Ready is not authority. Configuration changes preserve the installed identity and make it stale until deliberate installation. Aurora enable/disable uses fingerprinted reviewed transactions and separate retained-launcher ownership; do not merge these records into provider provenance.

Deletion **exists**, in `instances/deletion.rs`, with confirmation, registry/process/content exclusion and managed-root/reparse checks. It deletes the selected proven instance tree, not shared caches/runtimes or `.minecraft`. This is an existing explicitly destructive user action; F0 does not invoke or broaden it.

Current relevant schemas:

| Document | Current format | Limitation relevant to expansion |
| --- | --- | --- |
| Launcher config | 2 | No loader/provider expansion requires changing it |
| Instance registry | 5; reads known 2/3/4 | `PackRegistryIdentity::validate` accepts only Modrinth, 8-character project/version IDs |
| Installed game | 2; reads legacy 1 Fabric | Typed platform exists; generated-output recipe/output-set evidence is incomplete |
| Runtime installed state | 1 | Suitable exact file/link/runtime facts; no need to redesign for another major |
| Content managed state | 4; known earlier migrations | String provider fields; project-scoped edges; Pack origin file ID must look like SHA-512 |
| Installed pack | 2; reads 1 | Mandatory Fabric loader and SHA-512; components also mandate SHA-512 |
| Aurora/retained ownership | Existing separate documents | Preserve exact required/retained distinction and release authority |

Registry enum vocabulary already accommodates Forge/Quilt. New execution alone does not justify a registry bump; generic pack identity does.

## 6. End-to-end pipeline and reuse classification

Key trace: `instances/lifecycle.rs::create_instance` → configuration/capability resolution → `resolve_record_game_plan` → normalized Minecraft plus optional loader plan → `install_instance_components` → `install::install_game` → optional Aurora install → complete validation → Ready. NeoForge ensures managed Java before processors. Play resolves pinned plans/runtime/session, then `launch::boundary` rereads its snapshot and validates under content→registry→short process-state lock order before native spec/spawn.

| Stage | Classification | Evidence / extension needed |
| --- | --- | --- |
| 1. Version selection | Reusable with extension | Native official Mojang catalog; loader-specific exact compatibility and generation capabilities must narrow choices |
| 2. Mojang metadata | Reusable with extension | Pinned piston manifest; version JSON expected SHA-1; explicit old-shape rejection needs separately tested historical normalization |
| 3. Game planning | Reusable with extension | Independently meaningful vanilla plan; relocate shared composition from `fabric/plan.rs`; explicit family contributions |
| 4. Downloading | Reusable with extension | Pooled bounded transport; CF needs separately scoped credential/redirect policy and approval |
| 5. Integrity | Loader-independent and reusable | Expected SHA-1/256/512 kept distinct from observed hashes; CF content adapter must admit expected SHA-1 |
| 6. Cache | Loader-independent and reusable | Hits rehashed; filenames/URLs never trusted identity; CF cache use is conditional on approved terms |
| 7. Libraries | Reusable with extension | Deterministic order/provenance; explicit reviewed replacement/module-path/exclusion policy for Forge generations |
| 8. Natives | Reusable with extension | Contained ZIP extraction/duplicate policy reusable; historical classifiers and OS/arch rules need normalization |
| 9. Java | Reusable with extension | Exact managed component/platform; loader/tool constraints must agree and unavailable component must fail |
| 10. Installation | Reusable with extension | Shared verified staging/marker; Forge processors require normalized operations and harder execution boundary |
| 11. Installed state | Reusable with extension | Marker last; recipe/source/generated-output provenance and durable replacement need explicit format |
| 12. Validation | Reusable with extension | Deep read-only file/identity checks; extracted native output set currently not individually hashed |
| 13. Launch assembly | Reusable with extension | Strict placeholders, role-backed paths, final main class; generation-specific JVM/module arguments retained as structured data |
| 14. Spawn/supervision | Loader-independent and reusable | Absolute Java, structured vectors, exact child, redaction, same-instance exclusion |
| 15. Recovery/repair | Requiring substantial redesign for general repair | Explicit retry/reinstall and pack recovery exist; no general crash-safe repair/rollback history or cross-process lock |
| Optional Aurora tail | Aurora-specific | Exact release SHA-256, protected mods, transition and activity-bridge allowlists stay independent |
| Fabric adapter | Fabric-specific | Official Meta profile, loader/intermediary handling, Fabric predicates remain family-owned |

Acquisition is separate from materialization. Store objects are verified before copying into game staging; copies are rehashed. Installation consumes normalized plans, not raw network JSON. Marker-last and one rename establish completion for a new tree. Replacement moves the old tree aside before promoting the new one: normal errors can restore it, but a crash between renames needs a durable receipt. Do not describe this as an atomic multi-tree transaction.

## 7. Existing Fabric coupling

- `fabric/plan.rs` owns shared composition and `LoaderPlan::{Fabric,NeoForge}`. Moving those concrete types is justified; a plugin registry or universal Maven resolver is not.
- Lifecycle dispatch is explicit for Vanilla/NeoForge and otherwise requires Fabric. Add explicit exhaustive branches, never allow a future family to fall through Fabric.
- `PlatformPin::provider_loader()` currently supports Fabric/NeoForge. Provider slugs belong in each adapter's compatibility mapping, not the platform's universal wire identity.
- Fabric API protected dependency handling applies only to configuration-derived verified launcher requirements. Preserve generic Fabric API provider ownership when Aurora is off.
- `mrpack`, `modpacks`, `pack_state` and pack registry validation are still Fabric/Modrinth-specific.
- `mod_compatibility::satisfies_for_family` uses Maven ranges only for `neoForge`; all other strings fall through to Fabric. Future families require exhaustive typed dispatch.
- NeoForge optional dependencies normalize to Fabric recommendations, and NeoForge `incompatible` to Fabric advisory conflicts. Current readiness primarily enforces `depends`/`breaks`; this loses upstream semantics and must be corrected before broader compatibility claims.
- NeoForge metadata normalizes one `[[mods]]` entry; upstream permits multiple. The hand-written TOML subset rejects some otherwise valid TOML and ignores root `modLoader`/`loaderVersion`, feature and side/ordering semantics. It is not a complete NeoForge/Forge parser.
- Modrinth-specific command/DTO/component names and hard-coded Browse dispatch need a small provider facade; native instance context should remain authoritative.

## 8. Forge research: distinct installer generations

Discovery comes from [official Maven metadata](https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml), not CurseForge's loader catalog or a frontend URL. Pin the full official Forge artifact version and verify its exact Minecraft relationship; list order is not a reliable latest policy. Exact research samples below were fetched into memory and checked against the official adjacent `.sha1` before reading ZIP JSON. Sidecar and artifact share the same upstream trust channel; this is integrity/provenance, not an independent publisher signature.

Artifact URL convention: `https://maven.minecraftforge.net/net/minecraftforge/forge/<version>/forge-<version>-installer.jar`, with `.sha1` appended for the sidecar.

| Exact research sample | Profile / client launch | Client processing observed | Installer SHA-1 |
| --- | --- | --- | --- |
| 1.7.10-10.13.0.1150 | Legacy `install` + `versionInfo`; LaunchWrapper/FMLTweaker, legacy game argument string | No modern processor array | `3e211f03c133e55ea41fa2d6a60427b22dc34f17` |
| 1.12.2-14.23.5.2864 | Spec 0 wrapper with `version.json` inheritance, still legacy arguments | Zero client processors in this exact artifact | `b5ec0016c292830f2325e39417d1ec4d9e166cab` |
| 1.16.5-36.0.0 | Spec 0, ModLauncher, structured arguments | Four client processors; MCP data/split/remap/patch | `132b15ca0076284d31c1b6eecbcf48b90d74639c` |
| 1.18.2-40.3.12 | Spec 1, BootstrapLauncher, module-path/add-opens/add-exports | Six client processors of ten total; mappings, split, rename, patch | `d7f759dec5b52ddb342c3e12511da1674d0401bf` |
| 1.20.1-47.4.26 | Spec 1, BootstrapLauncher, module/bootstrap arguments | Six client processors of ten total | `27b38c97615d138101caa13c01e6bfda6176dea3` |
| 1.21.1-52.1.16 | Spec 1, `net.minecraftforge.bootstrap.ForgeBootstrap` | Three client processors of seven total | `076aad17209fe7f153887f7f3992506ab58472f5` |
| 26.2-65.1.3 | Spec 1, ForgeBootstrap, `forge_client` launch target | One client processor of five total, patching the vanilla client | `656512f29ec5e08b9585a7142f5c15d1e1c39954` |

These samples are representative pinned research identities, **not recommended latest releases**, and their JSON is not proof of functioning Aurora launches. Spec number alone does not identify behavior: both zero-processor and multi-processor profiles occur within spec 0.

The official [ClientInstall](https://github.com/MinecraftForge/Installer/blob/2.0/src/main/java/net/minecraftforge/installer/actions/ClientInstall.java) and [PostProcessors](https://github.com/MinecraftForge/Installer/blob/2.0/src/main/java/net/minecraftforge/installer/actions/PostProcessors.java) implementations distinguish acquisition, side-filtered tools, data substitution and output checks. Aurora should parse supported embedded profiles and run normalized client steps, not invoke the complete GUI/headless installer against `.minecraft` or let it edit launcher profiles.

Required normalization includes installer-embedded resources, validated bracketed Maven references (including `@zip`/`@txt`), literal versus filesystem token values, ordered tool dependencies/classpaths, explicit client/server side filtering, known `{ROOT}`, `{LIBRARY_DIR}`, `{MINECRAFT_JAR}`, `{INSTALLER}`, `{SIDE}` and data tokens, expected processor outputs where published, and a complete generated-file allowlist. Tool-only libraries must not leak into the game classpath. Inheritance means a loader overlay on the exact official vanilla plan, not reparsing loader JSON as a Mojang version document or arbitrary recursive inheritance.

Java requirements derive from the official base component/major plus evidence-backed loader/tool constraints. A processor requiring a different execution runtime must be modeled separately; silently choosing newest Java or downgrading the game is invalid. Client-only execution must exclude server processors and server launch scripts. Platform-dependent native/rule selection remains in deterministic planning. Initial targets should be Forge 26.2, then 1.20.1 after the mapping pipeline, with older generations separately gated.

## 9. NeoForge research and current scope

The existing adapter pins `https://maven.neoforged.net/releases/` and its official [versions API](https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge). `NeoForgeVersionEntry::installable_generation` deliberately restricts executable discovery to 26.x. Current implementation checks official installer/universal SHA-1 and embedded library expectations, then runs a narrow normalized processing vocabulary.

| Exact upstream sample | Observed mechanism | SHA-1 |
| --- | --- | --- |
| NeoForge 21.1.1 / MC 1.21.1 | Spec 1, BootstrapLauncher, six client processors out of ten, mapping/splitting/renaming/patching chain | `c419a63a0f49fa33d28980fc6b8974a7528eebcb` |
| NeoForge 26.2.0.88 / MC 26.2 | Spec 1, `net.neoforged.fml.startup.Client`, one client `PROCESS_MINECRAFT_JAR` step out of two total | `3b11639be22be8c01c9c3c115e53194f89c1dc48` |

URLs follow `https://maven.neoforged.net/releases/net/neoforged/neoforge/<version>/neoforge-<version>-installer.jar`. F0 checked both sidecar hashes and profiles. Prior Phase K acceptance covers actual 26.2 launch; its 26.3 beta entry has install acceptance, not the same authenticated launch proof.

Share bounded archive inspection, verified acquisition, normalized processor execution, output checks, library selection, staging and receipts. Keep discovery/version compatibility, profile-generation acceptance, token/data normalization, main class/arguments and mod semantics distinct for Forge and NeoForge. Neither numeric version scheme nor common installer vocabulary proves interchangeability.

## 10. Quilt implications

[Official Quilt Meta v3](https://meta.quiltmc.org/v3/versions/loader) exposes loader coordinates and hashes; game and exact game/loader profile endpoints are available. Exact profile `1.20.1/0.31.0-beta.4/profile/json` was inspected: main class `org.quiltmc.loader.impl.launch.knot.KnotClient`, inheritance of 1.20.1, common libraries plus Quilt loader/hashed mappings/Fabric intermediary, and a distinct launcher metadata generation. Per-game list ordering differed from the loader catalog; never select index zero as policy or invent a stable flag.

Quilt is a smaller adapter project than Forge's processor pipelines, but not a Fabric alias. Preserve expected hashes where published; represent digest-less transitive libraries honestly. Quilt's current metadata reader, not only an old RFC, must define dependency arrays, alternatives/conditions, `provides`, nested jars, mappings and optionality. Unknown semantic expressions must remain unknown, not compatible.

The [Quilt FAQ](https://quiltmc.org/en/about/faq/) says most Fabric mods can work on Quilt, not all; Fabric does not generally load Quilt-only mods, and Quilt does not make Forge/NeoForge mods compatible. It also records QSL discontinuation in December 2025. Do not prescribe QSL/QFAPI globally or promise every current game version from profile availability. Keep Aurora Fabric-only until its own release authority expressly supports another family.

## 11. Managed Java: actual range and limits

The planner is **not hard-coded to Java 21**. Official version JSON selects `component` and `majorVersion`, with Fabric/game constraints and optional Aurora compatibility assertion. Runtime metadata resolves an exact OS/architecture/component entry; files/manifests use expected SHA-1, cache hits rehash, installed state validates files/links/mode and optional absolute `java -version`. Windows uses managed `javaw.exe` for the game and `java.exe` for tools/diagnostics. No per-instance external executable override, system Java search, PATH change or JAVA_HOME change exists.

Current first-party version documents were SHA-1-checked in F0: 1.7.10/1.12.2/1.16.5 use `jre-legacy`/8; 1.17.1 `java-runtime-alpha`/16; 1.18.2 `beta`/17; 1.20.1 `gamma`/17; 1.21.1/1.21.11 `delta`/21; 26.2 `epsilon`/25. See matrix for exact document identities. Java 8/16/17 availability is not existing game-generation acceptance.

The current [Mojang runtime feed](https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json) offers Windows x64 and Linux x64 8/16/17/21/25; Windows x86 lacks 21/25; Windows ARM64 has gamma 17/delta 21/epsilon 25 but no legacy 8/alpha 16/beta 17; macOS ARM64 similarly lacks legacy/alpha/beta. macOS x64 offers those majors; Linux i386 has legacy 8 only. Linux ARM64 is unmapped. Availability is component-specific: ARM64 gamma does not authorize substituting it for beta. Versions/builds can change; exact resolved manifests remain authority.

Memory is 512–32768 whole MiB, default 2048. Rust generates the heap argument; parsed custom JVM vectors cannot override heap/classpath/executable policy. Loader-required module options must be planned separately from user options. The diagnostic has a timeout and sanitized 4096-character summary, but `runtime/install.rs::run_process` currently buffers `Command::output()` before truncation: it shares the capture-hardening concern, though executing a verified runtime with `-version` is a narrower threat surface than a downloaded processor.

## 12. Launch specification and compatibility authority

`LaunchPlan::from_game_plan` consumes exact effective main class, game/JVM groups, ordered libraries and platform. `LaunchSpec` stays Rust-only/nonserializable/redacting. Only template substitutions from validated managed paths and a real entitled session may supply auth/game/assets/natives/logging inputs. Unknown placeholders are hard errors. Classpath is manifest-role-backed in plan order with Minecraft client last; native extraction inputs and tool-only libraries are excluded, and user mods are not scanned into it.

Game directory and working directory are the isolated instance root. Environment preserves graphics/audio compatibility while removing Java injection and stale bridge variables. Only the spawned child handle is supervised; duplicate preparation/Starting/Running is process-local. Launch tokens never become DTOs, events, persisted plan receipts or diagnostics. A source/provider outage must not become a new dependency for unrelated launch paths.

Cold Play currently resolves official Minecraft/loader/runtime metadata again. Installed products are reused, but complete offline cold launch is not implemented. Persisting an immutable, validated normalized plan receipt could remove this dependency; it must contain templates and integrity/source facts only, never session-expanded arguments.

Compatibility should be layered in Rust: broad provider search filters → exact file/loader/game/environment checks → required dependency graph → verified artifact metadata inspection → installed active inventory → final readiness with resolved Java. Search tags and friendly names are never sufficient. Fabric predicates, Forge/NeoForge Maven ranges and Quilt expressions need family-owned evaluators, typed severity and uncertainty. Multiple mod IDs per file and nested provided capabilities must not imply multiple independently removable files. Resource/shader files do not prove a usable rendering mod exists. Native library conflicts belong in planning/extraction, not mod dependency guesses.

## 13. CurseForge API, access and download policy

**Publicly documented:** the [REST API](https://docs.curseforge.com/rest-api/) uses `https://api.curseforge.com`, `x-api-key`, project/file IDs, search/categories, descriptions/artwork/authors, files/release types/dependencies and download URL lookup. Loader enums distinguish Forge/Fabric/Quilt/NeoForge; file hashes use SHA-1 or MD5. SHA-256 is not supplied by that hash enum. Fingerprints are recognition aids, not cryptographic verification. Search pagination is bounded; do not assume unlimited results.

**External approval dependency:** [API-key application guidance](https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key) requires review and acceptance of third-party terms, considering author consent and service impact. No approved Aurora key, contractual permission, quota or authenticated API response was established in F0; none was requested or embedded.

**Material terms blocker:** the [published third-party agreement](https://support.curseforge.com/support/solutions/articles/9000207405-curseforge-3rd-party-api-terms-and-conditions), modified August 2024 and retrieved in F0, prohibits sharing the developer key, API-data caching, hiding access through proxies, and competing products; discretionary quotas may require a further paid agreement. Aurora's proposed mainstream launcher, client key delivery, metadata/provenance persistence and cache reuse require written clarification/permission. Do not interpret the separate Studios agreement as that permission, assume a backend proxy is allowed, or invent a universal request rate, commercial entitlement, attribution waiver, logo license or cache TTL. Owner/CurseForge approval is a release gate, not something code can solve.

**Current download change:** the [June 2026 announcement](https://blog.curseforge.com/introducing-api-key-authentication-for-curseforge-file-downloads/) makes `edge.forgecdn.net` require a valid API key from July 16, 2026; unauthenticated requests can receive 401. Prefer the documented header over query credentials. Exact authorized redirect/CDN hosts and key delivery remain approval questions. Never attach a CF key to the shared global client/default headers, Modrinth, Mojang, icon requests, user pages or arbitrary redirects.

Recommended failure states: access not configured/approved, unauthorized/revoked, permission restricted, manual file required, rate limited with bounded retry information, file unavailable, no acceptable expected hash, incompatible file, stale review and integrity failure. All remain independent of already installed content and unrelated Play. No automatic unlimited retries or silent alternate-provider acquisition.

If approved, resolve project→exact file and compatibility, inspect distribution permission/download availability, require published expected SHA-1 plus size, acquire through existing verified store, compute a separate local SHA-256, and stage through the shared content transaction. MD5/fingerprints alone cannot authorize managed installation. Expected SHA-1 verification is honestly labeled SHA-1, not upgraded to SHA-256 authenticity by computing a local hash.

Restricted files: present the official project/file page and reason; allow explicit user-selected local file only if the approved workflow permits it. Rust copies that granted file into untrusted staging and verifies the pre-known file SHA-1/size before store promotion. Missing required files keep a pack incomplete; optional omissions remain explicit. Do not scrape pages, automate login/cookies, watch the Downloads directory, trust filenames, construct CDN URLs from IDs, or bypass author restrictions. A file with no acceptable expected digest remains unmanaged/unavailable for managed installation.

## 14. Discovery and existing Modrinth comparison

`modrinth.rs` already resolves public v2 metadata anonymously, builds bounded required graphs, enforces exact Minecraft/loader/environment and published SHA-512, selects by publish time/release policy, and preserves provider identity. Explicit update checks and graph previews do not change installed versions automatically. Required/optional/embedded/incompatible semantics and exact version references remain adapter-owned.

CurseForge can expose mods/resource packs/shaders/modpacks through its project/category/file model. Resolve class/category IDs from approved official metadata rather than assuming all project types share a loader facet. Resource/shader compatibility differs from mod loading. Preserve per-provider release channels, numeric file IDs, distribution flags, permission failures, sorting and pagination. Icons/screenshots/descriptions need a sanitized display boundary and independently allowlisted image hosts; do not expand CSP to all provider-controlled URLs or reuse authenticated download headers for images.

Updates must pin exact file IDs, exclude incompatible/retracted candidates, preserve release policy and compare authoritative dates/identity rather than lexical version names. Modrinth version ID and CF file ID are different concepts. File-level metadata and post-download mod inspection are stronger evidence than project-wide game tags.

## 15. Packs: format, ordering, updates and portability

Current Aurora supports exact remote Modrinth Fabric `.mrpack` installation, SHA-512 archive/content verification, bounded `modrinth.index.json`, client/common override precedence, explicit optional files, independent instance creation, ownership and three-way pack reconciliation. NeoForge packs are explicitly refused. It is not yet a general local import/export engine.

The official [CF export guidance](https://support.curseforge.com/support/solutions/articles/9000197908-exporting-a-modpack-for-curseforge-project-submission?locale=pt) documents a ZIP root manifest plus overrides and references rather than bundling CF-hosted mods. Supported manifest-v1 fields are additionally evidenced by Prism's parser: `manifestType: minecraftModpack`, `manifestVersion: 1`, `minecraft.version`, `minecraft.modLoaders[{id,primary}]`, `files[{projectID,fileID,required}]`, pack name/version/author and an overrides folder. This source-backed observed format is not an official exhaustive JSON schema. Unknown loader formats and competing primary loaders must fail explicitly.

Manifest file references do not carry the expected hashes Aurora needs: approved API resolution must bind each project/file, type, hash, size and permission before acquisition. Overrides may include configuration and resource/shader files; placement must derive from a validated normalized relative path/type, not a remote filename or arbitrary archive folder. Submission eligibility guidance mentioning Forge/Fabric does not prove local NeoForge/Quilt manifest semantics or acceptance; clarify them separately.

Proposed ordering: inspect/bound archive → normalize exact game/loader and optional choices → resolve all file identities/permissions/hashes → review missing/manual/conflicting items → acquire verified products → install managed game/runtime into a new instance → stage pack components and allowlisted overrides → validate ownership and game/content → pack marker → Ready. Do not create a Ready instance from a partial download or infer install order from the ZIP listing.

Update using old authored snapshot + current bytes + new authored snapshot, preserving divergent config/saves and explicit user changes. Pack pins outrank ordinary provider update suggestions; standard update/remove cannot mutate pack-owned entries. Reuse `pack_update` reconciliation and recovery ideas after removing SHA-512/Fabric assumptions. The CF format itself does not define an atomic update or rollback protocol.

Import requires an explicit native file grant, bounded archive and hash observation, not a claimed official pack identity from a filename. Export must review references versus licensed redistributable overrides, exclude accounts/tokens/logs/cache/runtime/launcher state and user worlds by default, and require explicit content selection/rights. Cross-provider entries that cannot be represented legally must be reported, not silently bundled or relabeled. Private backups and distributable CF exports are different operations. CF sharing codes have their own app behavior; no general documented public sharing-code API was established.

## 16. Cross-provider provenance and ownership

Five dimensions must stay separate: **bytes**, **source**, **management owner**, **loader capabilities**, **requested dependency**. Equal names/slugs/mod IDs do not prove provider equivalence. Even equal bytes do not transfer ownership or update policy automatically.

Preserve Modrinth project/version/exact file hash; CF numeric project/file; local import with local hashes and no invented provider; pack source/version plus authored file reference; launcher-owned Aurora/Fabric API expected release identity. Hash algorithms retain their names and expected/observed provenance. A pack-owned file can have an original provider source or a user-approved exact alternative without changing who may remove it.

Replace project-scoped `requires` references with exact content-entry IDs and satisfaction evidence. Keep requested CF identity when an explicitly approved, exact-hash Modrinth alternative supplies it; record both source facts and the approval, never silently convert CF pack references into Modrinth ownership. Name-based recognition and automatic cross-provider updates are prohibited. Removal considers exact verified ownership, all required dependants and explicit retention, not a guessed provider reference count.

The [target architecture](FORGE_CURSEFORGE_ARCHITECTURE.md) specifies records, migrations, digest handling and transaction boundaries. Do not simply add `provider = curseforge` to schema 4 and hope the existing SHA-512-shaped constraints work.

## 17. Security and recovery findings

| Priority / finding | Source evidence | Consequence / required future work |
| --- | --- | --- |
| High: Java processor is same-user code, not a filesystem sandbox | `neoforge/processors.rs::run_one_processor`, `require_within` | Working directory/env clear/lexical checks do not prevent outside writes, network or access to user resources. Evaluate an OS-enforced boundary and explicit supported-host policy before wider execution claims |
| High: output bound is applied after collection | `processors.rs` around 410 and `diagnostics_tail`; runtime `install.rs::run_process` | `.output()` can allocate until timeout. Stream/drain with a real live cap, kill/reap policy, Unicode-safe redacted tails and bounded descendant handling |
| High: incomplete output/write model | `snapshot_tree` around 541; generated `std::fs::read` around 200 | Names alone miss in-place modifications; symlink new outputs may be followed and whole files read unbounded. Reject links/reparse points, hash immutable inputs, stream outputs with count/size bounds, strict allowed output set |
| High: published processor outputs omitted | `neoforge/metadata.rs::ProfileProcessor` has no outputs map | Preserve official output expectations; distinguish verified expected output from locally generated observation; do not fabricate missing hashes |
| Medium: unbounded manifest read before length check | `read_manifest_main_class` | Apply archive entry/read bounds before allocation; reject duplicates and malformed/wrapped headers deterministically |
| Medium: diagnostic slicing can panic on Unicode | `diagnostics_tail` indexes bytes in UTF-8 strings | Use character-safe truncation, bounded reader and tests with multibyte output |
| High: cache passed into same-user tool process | Processor `{INSTALLER}`/input resolution | A verified path is not immutable to arbitrary code. Use private copied/read-only input views, reverify, and keep cache outside allowed write surface |
| High: misleading dependency semantics | Neo TOML normalization + family fallback | Optional-if-present and incompatible constraints can escape fatal checks. Preserve family relation meaning and typed severity |
| High: replacement/crash and cross-process windows | `install`, `runtime/install`, content/transition locks | Error rollback is not crash recovery. Add scoped durable journal and OS locks; never clean an unknown tree |
| Medium: extracted native content proof incomplete | `install::validate_installed_game` | Archive hash + nonempty native directory does not prove exact extracted output bytes. Record/verify extraction outputs and collision decisions |
| External blocker: CF terms and credentials | Official third-party terms, application and CDN guidance | Approval-specific key delivery/cache/provenance/branding/quotas must be established before production integration |

These are source audit findings and design risks, not claims of an exploited installation. No malicious processor was executed in F0. Future failure tests must use exact disposable roots and safe isolated fixtures, never probe the owner's files.

Existing protections worth retaining: explicit loopback-only test transport; expected-hash store before activation; cache rehash; strict managed IDs; containment/reparse refusal; bounded archive extraction; manifest-last; exact process handle; session redaction; final spawn snapshot; fingerprinted approvals; malformed state refusal; independent accounts/instances/providers; no user-content cleanup on launch failure.

## 18. Mature launcher references and what not to copy

Read-only source snapshots: Prism `924fae10d1910d71673816ddb9b128abcdab69e2`; ATLauncher `8e9a71345bf35aee4e927a848930e946de331b30`. Both repositories identify GPL-3.0 licensing; source reuse needs license review. F0 copied no implementation.

- [Prism manifest parser](https://github.com/PrismLauncher/PrismLauncher/blob/924fae10d1910d71673816ddb9b128abcdab69e2/launcher/modplatform/flame/PackManifest.cpp): distinct format/provider parsing is useful; port semantics into normalized Aurora plans rather than importing its instance model.
- [Prism pack creation](https://github.com/PrismLauncher/PrismLauncher/blob/924fae10d1910d71673816ddb9b128abcdab69e2/launcher/modplatform/flame/FlameInstanceCreationTask.cpp): explicit optional/restricted-file review is useful. Its update comments identify override preservation complexity; Aurora's existing three-way ownership model should remain stronger than unconditional replacement.
- [Prism file resolution](https://github.com/PrismLauncher/PrismLauncher/blob/924fae10d1910d71673816ddb9b128abcdab69e2/launcher/modplatform/flame/FileResolvingTask.cpp): alternate-provider fallback exists there; Aurora needs explicit approval and dual provenance rather than silently replacing a source URL.
- [Prism 11.1.1 release](https://github.com/PrismLauncher/PrismLauncher/releases/tag/11.1.1) includes a CF overrides traversal fix (#6183). Archive entry checks alone are insufficient; the manifest's overrides-root field is also hostile input.
- [ATLauncher manual-download dialog](https://github.com/ATLauncher/ATLauncher/blob/8e9a71345bf35aee4e927a848930e946de331b30/src/main/java/com/atlauncher/gui/dialogs/BrowserDownloadDialog.java): clear browser-mediated pending-file UX is useful. Do not adopt filename/Downloads watching or MD5-only verification as Aurora authority.
- [ATLauncher repository](https://github.com/ATLauncher/ATLauncher) explicitly warns forks not to borrow its service credentials/infrastructure. No third-party key, Microsoft client ID, CDN, update authority or telemetry architecture should be copied.

Mainstream parity is an acceptance target, not a reason to replace Tauri/Rust authority, add a plugin service container, execute full installers, or silently change ownership.

## 19. Decisions, recommendations and source register

Recommended first work: shared processor/resource/output hardening and exact composition boundaries, preserving current 26.x NeoForge acceptance; then Forge 26.2 and 1.20.1 with distinct adapters, followed by older NeoForge. Provider/pack schema groundwork can proceed independently. CF discovery and downloads remain conditional on permission. Quilt and historical generations are separate capability slices, not a universal compatibility switch.

Owner decisions before implementation/release of affected features:

1. Supported processor hosts and OS-enforced isolation versus explicitly accepted official-tool trust; no claim that path substitution alone restricts Java effects.
2. Version priority: proposed Forge 26.2 → 1.20.1 → older NeoForge → bounded historical generations; demand for 1.12.2/1.7.10 changes cost substantially.
3. CF written approval for launcher use, credentials, caching/persisted provenance, CDN redirects, export/redistribution, quotas, commercial plans and attribution/branding.
4. Migration backups and downgrade policy: old executables cannot safely edit new schemas; retain old readers/pins and never auto-downgrade documents.
5. Pack import/export rights, user-file conflict policy and whether an explicit alternate-provider fulfillment is offered; no automatic substitutions.
6. Cross-platform release support requires secure credential stores and native acceptance; runtime metadata availability alone does not make Linux/macOS supported products.

External sources below were retrieved **2026-10-10–11 America/New_York**; moving branches are research snapshots, not permanent contracts. Direct installer/profile identities and Mojang SHA-1 document URLs in the matrix make the observations reproducible.

| Area | Primary references / scope |
| --- | --- |
| Mojang game | [Pinned version manifest](https://piston-meta.mojang.com/mc/game/version_manifest_v2.json); exact verified document URLs in matrix |
| Mojang Java | [Pinned all-runtime feed](https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json) |
| Forge | [Maven metadata](https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml), [installer profile source](https://github.com/MinecraftForge/Installer/blob/2.0/src/main/java/net/minecraftforge/installer/json/Install.java), ClientInstall/PostProcessors above, [current mod files](https://docs.minecraftforge.net/en/latest/gettingstarted/modfiles/), [1.12 metadata structure](https://docs.minecraftforge.net/en/1.12.x/gettingstarted/structuring/) |
| NeoForge | [Official Maven versions](https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge), [user guide](https://docs.neoforged.net/user/docs/), [mod metadata](https://docs.neoforged.net/docs/gettingstarted/modfiles/) |
| Fabric | [Official Meta](https://meta.fabricmc.net/v2/), [relationships/spec](https://docs.fabricmc.net/develop/loader/fabric-mod-json) |
| Quilt | [Meta loader catalog](https://meta.quiltmc.org/v3/versions/loader), [exact researched profile](https://meta.quiltmc.org/v3/versions/loader/1.20.1/0.31.0-beta.4/profile/json), [current metadata reader](https://github.com/QuiltMC/quilt-loader/blob/fe14f3161fd89f0b8acc6530e86f02c3920c78fc/src/main/java/org/quiltmc/loader/impl/metadata/qmj/V1ModMetadataReader.java), [RFC, secondary to implementation](https://github.com/QuiltMC/rfcs/blob/main/specification/0002-quilt.mod.json.md), [FAQ](https://quiltmc.org/en/about/faq/) |
| CurseForge | REST/API application/third-party terms/CDN/export links in sections 13–15; [sharing profiles](https://support.curseforge.com/support/solutions/articles/9000197912-sharing-profiles), [moderation](https://support.curseforge.com/support/solutions/articles/9000197279-moderation-policies) |
| Modrinth | [Official pack format](https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack), [API documentation](https://docs.modrinth.com/api/) and current local adapter |
| Models | [Official model catalog](https://developers.openai.com/api/docs/models), used only for roadmap agent recommendations |

Remaining uncertainties are deliberately bounded: no approved CF contract/key/API acceptance, no new Forge/Quilt/older-Neo installation, no new cross-platform/gameplay tests, no independent signature authentication of installer sidecars, and no proven OS sandbox design. They block affected implementation/production claims, not completion of this research phase.

### Final preservation result

Final measured comparison on 2026-10-11: **PASS**. All 809 tracked and 15,144 pre-existing nonignored untracked files match their original SHA-256 and size (15,953 hashed files; none missing or changed). All 41,393 ignored paths retain their recorded inventory, size and modification time; ignored file contents were not independently rehashed. Branch, HEAD and tracked inventory are unchanged. Tracked name-status and staged diffs are empty; critical root documentation, manifests, lockfiles and native/frontend configuration remain present. Exactly the four requested root Markdown documents are newly untracked, bringing nonignored untracked paths to 15,148. No existing file was staged, committed, removed or rewritten, and no generated game/build/runtime data was staged.

Documentation checks passed: all four files exist with balanced code fences and valid local document links; all 14 roadmap implementation subphases contain the 11 required planning fields. No application tests/builds were run for this documentation-only task. The external machine-readable result is `C:\Users\kalib\AppData\Local\Temp\aurora-f0-audit-yrrax0o8\final-verification.json`. Owner managed app data and `.minecraft` were not accessed or mutated; preservation of those external locations follows from the task's read-only operations, not an asserted scan of their contents. No push, publication, release mutation or installed-Launcher replacement occurred. F0 stops for owner review.
