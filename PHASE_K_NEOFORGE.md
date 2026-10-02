# Phase K — NeoForge loader support

Phase K makes NeoForge a first-class loader of ordinary Aurora instances
inside the existing normalized installation, validation, and launch
architecture. There is no parallel "NeoForge launcher": the composed
`GameInstallPlan` gained a loader-generic slot (`LoaderPlan`) whose NeoForge
variant is planned, acquired, installed, validated, and launched through the
same boundaries as Vanilla and Fabric.

## Authoritative NeoForge model (verified against live artifacts)

Research notes and evidence: `.zcode-diag/phase-k-neoforge/research-notes.md`.
Everything below was verified against `maven.neoforged.net` artifacts and a
real launcher-style boot of Minecraft 26.2 + NeoForge 26.2.0.88.

- **Discovery** — the repository's release listing for
  `net.neoforged:neoforge`
  (`https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge`).
  Versions are bare NeoForge versions; the Minecraft version is encoded by
  the leading components: three components `A.B.C` with `A` in `{20, 21}`
  address Minecraft `1.A.B`; four components `A.B.C.D` address Minecraft
  `A.B.C`, or `A.B` when `C` is zero. Anything else (the 2025 April-Fools
  line) is unattributable and filtered out, never guessed. Prerelease
  suffixes (`-beta`, `-alpha`) mark unstable builds; ordering within one
  Minecraft version is the trailing build counter, never lexical. The old
  promotions API is gone; there are no recommended labels anymore.
- **Installer** — an ordinary repository artifact with official SHA-1
  sidecars, embedding `install_profile.json` (identity, data tokens,
  processor definitions, installer libraries) and `version.json` (the launch
  profile: main class, JVM/game arguments, libraries — each with SHA-1 and
  size) plus `data/client.lzma` (the binpatch bundle).
- **Client install** — the current generation (26.x line) executes one
  client-side processor (`PROCESS_MINECRAFT_JAR`) producing
  `net.neoforged:minecraft-client-patched:<version>`. The previous
  generation (1.20.x–1.21.x lines) needs a six-step mapping/rename/patch
  chain including a Mojang-mappings download; Phase K deliberately scopes
  discovery and installation to the 26.x generation and reports older
  Minecraft versions as having no compatible NeoForge version (a documented
  limitation, not a fabricated incompatibility).
- **Launch** — classpath = Mojang platform libraries + NeoForge launch
  libraries + the vanilla client jar last; main class from `version.json`
  (`net.neoforged.fml.startup.Client` on 26.x); NeoForge JVM/game arguments
  appended after Mojang's; FML locates the universal jar and the patched
  client through `-DlibraryDirectory`, so both are materialized in the
  managed libraries tree but are not classpath entries.
- **Java** — NeoForge publishes no loader Java floor of its own; Mojang's
  version document stays authoritative (Minecraft 26.2 requires the
  `java-runtime-epsilon` major-25 component, which the managed runtime
  pipeline resolves and installs automatically).
- **Integrity** — every NeoForge network artifact carries an official
  SHA-1 (inline for libraries, sidecar for installer/universal), so the
  whole acquisition chain is digest-verified through the SHA-1 store,
  exactly like Mojang metadata. The processor-generated patched client is
  recorded honestly as a `LocallyGenerated` observation (a local SHA-256
  consistency reference), never as an official verification.

## Normalized loader architecture

- `LoaderKind::NeoForge`, `LoaderConfiguration::NeoForge { policy }`, and
  `PlatformPin::NeoForge { version }` already existed as representations;
  Phase K gave them executors and removed nothing else from their meaning.
- `GameInstallPlan` (`src-tauri/src/fabric/plan.rs`) now carries
  `loader: Option<LoaderPlan>` with `LoaderPlan::{Fabric, NeoForge}`; the
  container stays where it always was, and `fabric_loader()` /
  `neoforge_loader()` accessors preserve the independently meaningful
  per-loader boundaries. `GameLibrary::NeoForge` joins the composed library
  set with `LibraryProvenance::NeoForge`; library merge/dedup/conflict rules
  are shared (`merge_library_sets`).
- Loader contributions to launch arguments are new with NeoForge:
  `GameInstallPlan::effective_jvm_arguments()` /
  `effective_game_arguments()` return Mojang's planned groups followed by
  the loader's unconditional groups; `LaunchPlan::from_game_plan` consumes
  the effective views. Fabric/Vanilla behavior is unchanged (no loader
  arguments).
- The processor model lives in `src-tauri/src/neoforge/`:
  `metadata.rs` (DTO boundary: listing API, sidecar digests, installer
  documents), `plan.rs` (`NeoForgePlan` normalization and composition),
  `processors.rs` (bounded executor), `mods.rs` (`neoforge.mods.toml`
  parsing), `versions.rs` (Maven version-range semantics).

## Security boundary

- **Installer boundary** — Aurora never runs the NeoForge installer. The
  verified installer artifact is read as a metadata document, and its
  processor definitions are executed as narrowly modeled build steps.
- **Processor restrictions** — arguments are substituted only from the
  normalized plan: built-in tokens (`SIDE`, `MINECRAFT_JAR`, `INSTALLER`,
  `ROOT`, `LIBRARY_DIR`, `MINECRAFT_VERSION`) and profile data tokens
  (literal / installer-embedded file / Maven artifact reference). Unknown
  tokens, bracket references in raw arguments, and token fragments are
  rejected. Every resolved path must stay inside the staged game tree or
  the processor scratch directory (`require_within`); parent-directory
  components are rejected outright. Installer-embedded inputs are extracted
  under scratch, never beside the executable.
- **Execution** — the exact managed Java diagnostic executable, argument
  vector only, empty environment (plus the OS floor variables), bounded
  working directory, 600-second wall clock, captured and bounded
  diagnostics. Nonzero exit, timeout, missing expected output, or an empty
  generated artifact fails the installation.
- **Filesystem effects** — a before/after snapshot of the staged tree
  detects every produced file (recorded as generated manifest entries) and
  any destruction of already-materialized managed files. Nothing a
  processor writes outside staging is captured or activated, and staging is
  disposable on failure.
- **URL trust** — all remote URLs come from native trusted metadata
  resolution (pinned repository endpoints, verified documents); the
  frontend supplies only version strings, never URLs, digests, or paths.
  Library URLs must be HTTPS (loopback only for the documented test
  transport).
- **Frontend trust boundary** — `LaunchSpec` stays Rust-only; the
  create/plan commands accept only `{minecraftVersion, loader, auroraEnabled}`;
  no installer URL, Maven URL, path, processor command, Java executable,
  main class, or classpath crosses the boundary.
- **Redaction** — processor arguments carry no secrets; the game process
  keeps the existing redaction pipeline (`auth_access_token` and friends).

## Transaction

The existing staged model is unchanged: plan → acquire (SHA-1/SHA-256
stores) → materialize into `.install-staging/game` → extract natives →
process (NeoForge only) → staged validation → manifest written last → one
promoting rename. A failing processor leaves no complete-looking
installation, preserves the verified shared cache and unrelated instances,
and retry reuses cached inputs. One installation per instance at a time.

## Installed state and schema changes

- `installed-game.json` stays schema 2; `from_json` accepts the
  `PlatformPin::NeoForge` platform tuple, and `NativesRecord` gained an
  optional `subdirectory` field (serde-defaulted, skipped when absent) for
  modern Minecraft's `${natives_directory}/java` convention. Existing
  Vanilla/Fabric manifests are byte-compatible.
- `ArtifactTrust` gained `LocallyGenerated { observed_sha256 }` for
  processor-generated artifacts — validated by hash re-verification like
  observations, never reported as digest-verified.
- The instance registry needed no schema change: `PlatformPin`/`LoaderConfiguration`
  already carried NeoForge identity (schema 5 round-trips it).

## Minecraft metadata adjustment

Minecraft 26.x documents append a subdirectory to their own
`java.library.path` template (`${natives_directory}/java`). Minecraft
planning now derives the subdirectory from the document at plan time
(`MinecraftInstallPlan::natives_subdirectory`), extraction writes there,
and the manifest records it. Historical documents (flat template) are
unchanged; conflicting or unsafe templates are rejected.

## Validation, repair, and readiness

`validate_instance` admits NeoForge pins and cross-checks registry
identity, the game manifest's exact Minecraft/NeoForge versions, the
Aurora artifact layer (never applicable — Aurora is Fabric-only), and
content state. Validation re-hashes every managed file per its recorded
trust, including `LocallyGenerated` observations. Corruption produces a
repairable damaged state; retry/reinstall re-acquires missing pieces from
the verified cache and re-runs processors deterministically. A NeoForge
instance is Ready only after full validation passes; NeoForge never
becomes the default loader.

## Java requirements

Mojang's document is the authority (component + major). Because processor
execution needs Java at install time, the lifecycle resolves and ensures
the managed runtime *before* the game installation for NeoForge plans and
passes the diagnostic executable into the install context. Launch later
revalidates the same runtime through the existing pipeline. No system Java,
`PATH`, or `JAVA_HOME` is used anywhere.

## Content compatibility

- `PlatformPin::provider_loader()` returns the per-family Modrinth slug
  (`fabric` / `neoforge`); provider queries, file compatibility, updates,
  and dependency resolution all flow from this backend-owned identity.
  Vanilla still has none.
- Mod metadata is normalized into the shared loader-neutral `ModMetadata`
  model (formerly `FabricModMetadata`; JSON shape unchanged). NeoForge
  instances read `META-INF/neoforge.mods.toml` (legacy `META-INF/mods.toml`
  accepted): `required`→depends, `optional`→recommends,
  `incompatible`→conflicts.
- Relation semantics are family-dispatched: Fabric predicates for Fabric,
  Maven ranges (`[26.2,)`, `[3,]`, exact, `1.21.*` wildcards) for NeoForge.
  Builtin ids: `minecraft`, `java`, and `fabricloader` (Fabric) or
  `neoforge`/`forge` (NeoForge).
- Local file recognition attributes the instance's own loader family.
- Aurora Client, Fabric API bootstrap, and managed required mods remain
  Fabric-only domains.

## Modpack boundary

NeoForge `.mrpack` is explicitly unsupported in Phase K: `mrpack` parsing
rejects the `neoforge` dependency key with `pack_unsupported_loader` and a
message naming the deferral; Modrinth pack browsing stays Fabric-only; the
pack resolver refuses NeoForge pack versions before any instance exists.
No partial NeoForge pack installation is possible. Fabric packs (Phase I)
and their reconciliation (Phase J) are unchanged.

## Version discovery UI

Instance creation offers Vanilla / Fabric / NeoForge from the backend
capability list. Selecting NeoForge reveals an exact-version picker fed by
`list_neoforge_versions` (newest first, stability marked); the default
policy is newest stable. Aurora is not offered for NeoForge instances
(backend rejects the combination). Version selection appears only when
NeoForge is selected; no Home/Settings/navigation redesign.

## Acceptance

Deterministic acceptance: A-CN all PASS (785 lib tests + 6 others, 0
failures; 29 ignored are pre-existing live/benchmark gates plus the new
live drift check). Full verification: `npm run check` 0/0, `npm test`
178/0, `npm run build` exit 0, `cargo fmt --all -- --check` clean,
`cargo check --all-targets` 0/0, `cargo test` all green, `npm run tauri
build` exit 0.

Live acceptance (disposable diagnostic root
`C:\Users\kalib\AppData\Local\Temp\aurora-phase-k-root`, confirmed by
the backend's own startup line): discovery, creation, and installation of
Minecraft 26.2 + NeoForge 26.2.0.88 (pinned and automatic) and Minecraft
26.3 + NeoForge 26.3.0.41-beta; managed `java-runtime-epsilon` (Java
25.0.1) installed by the runtime pipeline at install time; AppleSkin
(`appleskin-neoforge-mc26.2-3.0.10.jar`, Modrinth `EsAfCjCV`) installed
from the provider with exact provenance; restart persistence with no
re-downloads; truthful incompatibility rejection for Minecraft 1.21.11.

### Minecraft launch acceptance

The first implementation pass was blocked by authentication: the isolated
diagnostic environment had no Microsoft account, and production account
data was deliberately not accessed. The completion pass closed this item
through Aurora's normal Microsoft flow (owner-interacted browser sign-in
inside the diagnostic environment; no credentials copied, injected,
printed, or captured; the refresh credential lives only in the OS-backed
Credential Manager).

- Authentication completed through the normal Aurora flow; account
  "Spxcterr" recognized; the `authentication_required` blocker disappeared
- Pre-launch: content Ready, `java-runtime-epsilon` Java 25.0.1 Ready,
  instance Ready, AppleSkin present
- Lifecycle observed from real Aurora state: Ready -> Starting (10:10:58)
  -> Running (process 19252, started 10:11:21, executable under the
  diagnostic managed runtime)
- NeoForge confirmed from the game log: "Found mod file
  neoforge-26.2.0.88-universal.jar"; mod list "NeoForge 26.2.0.88
  (neoforge)"; AppleSkin "Found mod file
  appleskin-neoforge-mc26.2-3.0.10.jar" and "AppleSkin 3.0.10+mc26.2
  (appleskin)"
- Genuine Minecraft 26.2 main menu with the NeoForge badge (screenshot
  evidence)
- Duplicate launch: while Running no Play control exists (replaced by the
  Running indicator); readiness reports the supervised process; exactly
  one Java process; the existing process stayed supervised
- Session injection: "Setting user: Spxcterr" in the supervised log; no
  token patterns in any log
- Normal Quit -> exit code 0 -> Ready restored; no orphan Java process
- Second-launch sanity: Ready -> Starting (10:19:41) -> Running (process
  26148) -> NeoForge 26.2.0.88 + AppleSkin init confirmed -> normal exit 0
  -> Ready; no unexpected downloads
- Launcher restart: account still signed in, exact MC 26.2 / NeoForge
  26.2.0.88 persist, runtime valid, AppleSkin present, deep validation
  passed, no reinstall or redownload

Verdict: **COMPLETE**. Evidence: `.zcode-diag/phase-k-neoforge/`
(index in `evidence-index.md`).

## Limitations (genuine)

- Only the 26.x NeoForge generation is offered; NeoForge for Minecraft
  1.20.x–1.21.x (the six-processor mapping chain) is deliberately not
  planned or installed and is not offered by discovery.
- Processor outputs are regenerated on retry/reinstall (deterministic);
  verified inputs come from the content-addressed cache, but generated
  artifacts have no cross-install reuse store.
- `extract-libraries-to` side files a processor writes into the staged
  libraries root are recorded as generated observations rather than
  individually planned files.
- NeoForge mod metadata supports one `[[mods]]` entry per jar (multiple
  entries are rejected as ambiguous, mirroring the Fabric duplicate rule).

## Deferred

NeoForge `.mrpack` support and pack reconciliation (bounded follow-up),
Forge, Quilt, cross-device sync, renderer compatibility advisories, Aurora
Client update infrastructure, storage dedup/reflink, generic modpack
providers, CurseForge/Prism import, background auto-updates, and release
work remain out of scope.
