# Phase F1C — Forge compatibility expansion

Review date: 2026-10-12. Host: Windows x86_64. **Verdict: PARTIAL.**

## 1. Executive verdict

Minecraft 1.20.1/Forge 47.4.10 and Minecraft 1.20.4/Forge 49.2.0 now install
through the existing shared composition, verified stores and supervised
processor runner. Both passed real isolated installation, published output
verification, deep Ready validation, managed Java 17 validation and launch
assembly against real installed files. Interrupted processing and retry passed.
Authenticated menu, disposable-world gameplay and normal exit for both new
versions require owner confirmation. No game was spawned using a fixture
session. Completion criteria have not been reduced.

F1B is COMPLETE following the owner's explicitly reported authenticated
Forge 1.21.11/61.2.0 gameplay, normal exit and return to Ready with no issues.
That acceptance was recorded in a documentation-only commit before F1C work;
it is owner evidence, not agent-observed gameplay.

## 2. Starting branch and HEAD

Branch `codex/client-3-production-authority`, starting HEAD `35b21bd`.
Preflight recorded status/history, 824 tracked paths, critical documentation,
manifests, lockfiles, native configuration and hashes of pre-existing untracked
files under `.zcode-diag/phase-f1c-compatibility/`. The large existing diagnostic
tree was preserved. Mandatory architecture/audit/roadmap/matrix/F1A/F3/F1B
documents and the actual metadata/planning/install/runtime/launch/lifecycle
boundaries were inspected. Baseline frontend checking passed; Rust library
baseline was 913 passed, 0 failed, 29 ignored.

## 3. Local commits and files changed

| Commit | Scope |
| --- | --- |
| `1652a3e` | F1B owner acceptance documentation only |
| `a9554bd` | Verified official 1.20.x profile/version/Mojang fixtures and assessment before parser changes |
| `9c23c74` | Typed profile compatibility, mapping expectations, input/archive safety, Java/launch fixtures and processor regression tests |
| `0687a83` | Reviewed-choice lifecycle boundary and explicit isolated native acceptance harness |
| `4a8619a` | Acceptance-discovered Forge identity label and actual rendered-Svelte regression |
| Closing documentation checkpoint (this commit) | This report, architecture, compatibility matrix and evidence index |

Changed production areas: `forge::{metadata,plan,processors}`, `game::plan`,
normalized Minecraft metadata/plan, native Forge choice command and instance
lifecycle selection; one existing Instance Details label. Tests and upstream
fixtures accompany those boundaries. No dependency/lockfile/package/Tauri
identifier/authentication/production release changes. Final hashes are in
local Git history; there is no push, release or publication.

Exact changed paths, grouped for review:

- Documentation: `ARCHITECTURE.md`, `FORGE_F1B_MODERN_FORGE.md`,
  `FORGE_CURSEFORGE_COMPATIBILITY_MATRIX.md`, this report and
  `.zcode-diag/phase-f1c-compatibility/EVIDENCE.md`.
- Rust: `src-tauri/src/application.rs`, `src-tauri/src/instances/lifecycle.rs`,
  `src-tauri/src/game/plan.rs`, `src-tauri/src/minecraft/metadata.rs`,
  `src-tauri/src/minecraft/plan.rs`, and `src-tauri/src/forge/{mod,metadata,plan,processors}.rs`.
- Rust tests: `src-tauri/src/forge/compatibility_tests.rs` and
  `src-tauri/src/forge/processor_chain_tests.rs`.
- Fixtures: `src-tauri/src/forge/fixtures/README.md`,
  `1.20.1-47.4.10-install_profile.json`, `1.20.1-47.4.10-version.json`,
  `1.20.4-49.2.0-install_profile.json`, `1.20.4-49.2.0-version.json`,
  `1.20.1-mojang.json`, `1.20.4-mojang.json` in that directory.
- UI: `src/lib/launcher/widgets/InstanceDetailsWidget.svelte` and
  `src/lib/launcher/productSurfaces.test.ts`.

## 4. Supported exact version matrix

| Minecraft / Forge | Discovery | Format | Installable | Deep Ready | Gameplay |
| --- | --- | --- | --- | --- | --- |
| 1.20.1 / 47.4.10 | Official recommended | Spec 1, six client steps | Yes | W64 passed | Pending owner |
| 1.20.4 / 49.2.0 | Official recommended | Spec 1, five client steps | Yes | W64 passed | Pending owner |
| 1.21.11 / 61.2.0 | Official exact pin | Spec 1, three client steps | Preserved | W64 repeated | F1B owner-confirmed |
| Other Forge builds | Broad official discovery | Not implicitly accepted | Not offered | Not claimed | Not claimed |

The new pins are implemented install choices, not advertised as
gameplay-verified rows. Official latest builds47.4.26/49.2.8 were not silently
substituted for the reviewed recommended pins. Missing recommendation does
not invent stability. An unknown pin/format fails before executable use.

## 5. Official sources and exact installer identities

Sources: [official promotions](https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json),
[official MavenForge](https://maven.minecraftforge.net/net/minecraftforge/forge/),
the pinned Mojang piston-meta version manifest and its version-document URLs.
Installer acquisition verifies the official `.sha1` sidecar expectation through
the existing SHA-1 store; observed SHA-256 is a local identity, not a signature.
No unofficial mirror, frontend source URL or borrowed metadata policy is used.

| Exact installer | Bytes | Published SHA-1 | Observed SHA-256 |
| --- | --- | --- | --- |
| [1.20.1-47.4.10](https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-installer.jar) | 6,078,070 | `66bfea9963bfa60d88bab6b2750e74a958392715` | `1912760b4cb6b803d8a826de603c9076b1da71ec2765e9a1f8c1ca78f65278e3` |
| [1.20.4-49.2.0](https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.4-49.2.0/forge-1.20.4-49.2.0-installer.jar) | 5,535,650 | `01615b3e329bf23b1207de18b4c6f14cd897fde5` | `3a0fc9007762da3dc01e4bad13f88b480a53ac4dd306f342064d2b92e7497fb3` |

Mojang version JSON expected SHA-1:1.20.1
`760a390e336b2b6202ec5906757cee3a929a7c85`;1.20.4
`868b02c961f819c9af215d6c628cb0a08d7591df`. Raw Mojang fixtures preserve
verified bytes; Forge JSON is pretty-printed with only GUI base64 icon omitted.
Operational fields/order/hashes remain intact.

## 6. Installer-profile differences

Assessment was committed before parser extension. Spec 1, side-mapped data,
quoted literals, embedded client patch, Maven coordinates, receipts and
supervision were reusable. New normalization explicitly distinguishes whole
literal, data-token and bracketed Maven-input arguments. Inline MCP ZIP
coordinates resolve to managed verified libraries. Nested mandatory unknown
fields, invalid sides and unresolved fragments fail; validation is stricter.

1.20.1 uses `_comment_`, older BootstrapLauncher, jar splitting and several
unhashed intermediates. Its `PATCHED_SHA` must be honored despite a missing
processor outputs map. 1.20.4 has sanitized mappings and published expectations
for all five outputs. Tool-only libraries (including older Guava) cannot
collide with or enter the game classpath. Generated FML resources retain exact
library paths even when absent from the launch-library list.

## 7. Java runtime requirements

Both verified Mojang documents require `java-runtime-gamma`, major17.
Resolved game composition remains authoritative for game, Forge and processors;
no distinct tool Java is required by these selected profiles. W64 exact runtime
manifest SHA-1 `89ce85ccb518c62e18b4b58d63399ba2d9611426`, 403 files,
91,160,397 bytes, was acquired through expected-SHA-1 storage, deeply validated
and checked with real bounded `java -version`. Subsequent runs reused it.
No Java 21 hardcoding, system Java, architecture fallback or user override was
introduced. Existing mismatch/override and other-loader runtime tests passed.

## 8. Processor-chain behavior

| Pin | Declared client execution order |
| --- | --- |
| 1.20.1-47.4.10 | MCP_DATA → DOWNLOAD_MOJMAPS → MERGE_MAPPING → jarsplitter 1.1.4 → ForgeAutoRenamingTool 0.1.22:all → binarypatcher 1.1.1 |
| 1.20.4-49.2.0 | MCP_DATA → DOWNLOAD_MOJMAPS(sanitized) → MERGE_MAPPING → ForgeAutoRenamingTool 1.0.6 → binarypatcher 1.2.0 |

Only client steps execute, in declared order under F1A. Every required output
is present/bounded/hashed before a dependent step. All resolved inputs,
including inline ZIP/data inputs and the private reverified installer, are
checked for modification. Timeouts, cancellation, bounded stdout/stderr,
process-tree cleanup and durable receipts reuse the shared runner.

Upstream Installer PostProcessors/ClientInstall and installertools source were
inspected. The 1.20.1 raw mapping downloader performs network activity without
its own digest check. Composition attaches Mojang's normalized official
client-mappings SHA-1; Aurora verifies the resulting file before merge.
The 1.20.4 sanitized result verifies against Forge's published expected hash.
This is a narrowly reviewed processor network behavior, not general permission
for unverified artifacts or arbitrary download tasks.

## 9. Published output verification

| Pin / output | Expected SHA-1 |
| --- | --- |
| 1.20.1 raw Mojang mappings | `6c48521eed01fe2e8ecdadbd5ae348415f3c47da` |
| 1.20.1 slim | `de86b035d2da0f78940796bb95c39a932ed84834` |
| 1.20.1 extra | `8c5a95cbce940cfdb304376ae9fea47968d02587` |
| 1.20.1 patched client | `4d8a9a63dc16a45d7fc5c54c627234f601d0cc17` |
| 1.20.4 MCP mappings | `a9852bd9d5b3cd76530c1b32c9d12d3937f8c980` |
| 1.20.4 sanitized Mojang mappings | `e05e02372cad860b65b1f77c045cc8acac2a5bdf` |
| 1.20.4 merged mappings | `4cf91698cd6c2e08dbee0160518d0284c7bf9a46` |
| 1.20.4 SRG client | `f8ebc98b830ae8456b3cb10e07123d7fbffa6484` |
| 1.20.4 patched client | `4cfcb4bdbed70a0a1c44c1c0a7a40d8eca0177e2` |

All recorded expectations were checked during native installation. 1.20.1 MCP mappings,
merged mappings, SRG intermediate and jarsplitter sidecars have no publisher
hash and remain `LocallyGenerated`, with local SHA-256 consistency records.
No locally observed digest becomes publisher verification.

## 10. Launch-spec differences

1.20.1: `cpw.mods.bootstraplauncher.BootstrapLauncher`, `forgeclient`, explicit
module path/open/export directives and FML identity arguments. 1.20.4:
`net.minecraftforge.bootstrap.ForgeBootstrap`, `forge_client`, current bootstrap
shape. Parent inheritance must exactly match the selected Mojang game.
The managed vanilla filename `client.jar` is appended to the older bootstrap
ignore list, preserving upstream semantics with Aurora's managed basename.

Tests cover deterministic classpath with vanilla client last, exclusion of
processor-only dependencies, actual generated library paths, assets/natives,
working directory, managed executable, exact arguments, individual auth
substitution and secret redaction. Native assembly additionally used real
installed manifests/files and a fixture session solely to construct a Rust-only
LaunchSpec. It did not authenticate, spawn Minecraft or prove gameplay.

## 11. Instance lifecycle and UI

The existing Forge command and lifecycle resolver now expose only reviewed
exact candidates, while upstream discovery stays complete. Existing create /
Installing / install / deep validate / Ready behavior is reused. The opt-in
native harness retains seven isolated instances for review; retry recognizes
an existing Installing record rather than duplicating it. Native UI boot found
one Forge-as-Fabric label in Instance Details, corrected with a rendered-Svelte
regression. No creation redesign, unrelated widget behavior, account changes,
provider changes or customization changes. Aurora remains Fabric-exclusive.

## 12. Schema compatibility and recovery

Registry 5, installed-game 2 and receipt 1 remain unchanged. No migration or broad
persisted extension. Unknown/malformed state remains a deliberate failure.
Actual processor interruption left registry Installing, receipt processing /
processors incomplete and no staged completion manifest. The exact observed
managed Java child terminated with the runner. Retry completed and restored
deep Ready; instance-root user sentinel bytes survived warm reinstall/retry.
No F3R cross-process locking or old-tree restoration claim is made.

## 13. Automated verification

| Exact command | Result |
| --- | --- |
| `npm run check` | Passed,0 errors/0 warnings |
| `node --test --test-concurrency=1 "src/lib/**/*.test.ts"` |266 passed,0 failed,0 skipped,116.819s |
| `npm run build` | Passed production static frontend build |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Passed |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | Passed;4 pre-existing warnings |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` |923 passed,0 failed,30 ignored,54.30s |
| `cargo test --manifest-path src-tauri/Cargo.toml --release` |923 unit +6 integration passed,0 failed,30 ignored; unit54.36s |
| `npm run tauri build -- --no-bundle` | Final production native build passed in 4m13s; no installer/publication |

Default debug `cargo test` encountered a pre-existing owner's running debug
executable lock. That process was preserved; the full release-profile test
suite supplied integration coverage without replacing it. Default parallel
`npm test` stalled during SSR setup; the identical suite passed sequentially
(baseline 265 tests). This command adjustment is disclosed, not a test removal.
The local build supplied an empty updater public-key field through TAURI_CONFIG
to satisfy plugin configuration; it does not establish production updater trust
or change checked-in configuration. Isolated UI builds use custom-protocol
static assets and a private WebView data directory.

New deterministic tests cover exact choice/recommendation/missing installer,
official chain fixtures, invalid shapes/sides/arguments, inherited launch
requirements, absent mapping expectations, output verification, ordered
five/six-step compiled Java fixtures, server filtering, intermediate failure,
missing output, digest mismatch, changed input, corrupt private installer,
timeout/cancel, traversal/link/oversized archive/no-clobber and Windows junction
rejection before spawn. Existing F1A/F3 and loader/lifecycle tests were retained.

## 14. Native installation results

Retained isolated root:
`C:\Users\kalib\AppData\Local\Temp\aurora-f1c-acceptance-feaa62d6-705c-467e-99dc-e186e4d08201`.
Production managed roots and credentials were not used.

| Instance | ID | Machine result |
| --- | --- | --- |
| F1C 1.20.1 forge | `83b00dab27374bc7be67b78dc91e6140` | Official artifacts,6 processors, output digests, Java 17, deep Ready, real-file launch assembly passed |
| F1C 1.20.4 forge | `8a1042662b8d4db5b166e82b1f8e15f5` | Official artifacts,5 processors, output digests, Java 17, deep Ready, real-file launch assembly passed |

All seven native cases passed (404.24s initial complete run;288.67s final
recovery/revalidation run). The harness is explicitly ignored in deterministic
tests; official-network acceptance is opt-in and retains its owned temp root.
The actual desktop launcher booted against this root with isolated WebView
storage. Play correctly requires sign-in; no password or login automation.

## 15. Authenticated gameplay results

F1B: owner confirmed Forge 1.21.11/61.2.0 launch, gameplay, normal exit and
Ready with no further issues. F1C 1.20.1/47.4.10 and1.20.4/49.2.0: **pending**.
The isolated Launcher remains available. Owner should sign in normally, select
each clearly named F1C instance, confirm the Forge menu versions, create a
disposable world, verify rendering/movement/input, exit normally and confirm
Ready. Do not use existing owner worlds. No synthetic session is gameplay
evidence, and the report remains PARTIAL until both confirmations exist.

## 16. Existing-loader regression results

Fresh isolated native installs and repeated read-only deep Ready passed for
Forge 1.21.11/61.2.0, Vanilla1.21.11, Fabric 1.21.11/0.19.5,
NeoForge 26.2/26.2.0.88 and Aurora 3.0.0 on Fabric 1.21.11/0.19.5.
All retained regression/unit suites passed. No new authenticated game launch
was performed for these five cases; prior accepted gameplay remains separately
attributed. F1A supervision and F3 shared composition remain shared boundaries.

## 17. Performance observations

| Measurement | 1.20.1 | 1.20.4 |
| --- | --- | --- |
| First completed measured install |49.164s, resumed prior acquisition |44.491s, new game using shared warmed cache |
| Final cache-warm reinstall |38.729s |44.474s |
| Verified cache hits / network payload on warm reinstall |3714 /0 downloads /0 bytes |3915 /0 downloads /0 bytes |
| Asset count |3575 |3788 |
| Native-file launch preparation including runtime ensure/deep validation |5.624s |5.551s |

An uninterrupted empty-cache cold timing was **not captured**: the initial
acquisition was interrupted and the later game shared the isolated cache.
These numbers must not be represented as cold benchmarks. 1.20.1 retry processor
phase lasted 19.698s (processing7.523s→validation27.221s); warm staged validation
includes processor time,25.772s/31.513s respectively, not a pure processor
benchmark. Managed Java acquisition was successful but its isolated cold
duration was not separately timed; subsequent 403-file reuse validation was
about 0.5s. Existing bounded/concurrent artifact acquisition was not changed.
No hashes or deep validation were skipped for performance.

## 18. Security limitations

Processors execute with user authority and are **not sandboxed**. Digest
expectations are published over HTTPS and are not release signatures; SHA-1
retains its existing limitations. The reviewed network mapping tool may contact
upstream during processing, but required output verifies before use. Private
installer/input integrity, link-aware staging, bounded reads/output, required
output hashes, supervised child cleanup and receipts remain enforced.
No universal hostile-code confinement, cross-process exclusion, arbitrary
historical compatibility, mod compatibility or non-W64 acceptance is claimed.
An empty diagnostic updater key does not authorize an update or publication.

## 19. Preservation verification

All 824 original tracked paths remain present.15,169 pre-existing untracked
files were byte-hash checked with zero unexpected changes; baseline and final
records are retained. Critical docs/manifests/lockfiles/frontend/native config
exist. Name-status/deletion diffs were inspected before focused commits; no
tracked deletion, generated binary/game/runtime staging or dependency rewrite.
Production Launcher 1.4.2, owner instances/worlds/auth/cache/release metadata and
the pre-existing running debug launcher were preserved. Only this phase's
exact own runner was terminated for interruption testing. No cleanup/reset,
production install, push, release, signing or publication occurred.

## 20. Older-generation investigation only

| Inspected official pin | Spec / client steps | Main class / Java generation | Published installer SHA-1 | Finding |
| --- | --- | --- | --- | --- |
| 1.19.2-43.5.0 |1 /6 | BootstrapLauncher /17 | `0a45a1d96388403c5f16556df81165ba4d96987a` | Mapping/split chain resembles1.20.1; exact base/native/runtime acceptance still required |
| 1.18.2-40.3.0 |1 /6 | BootstrapLauncher /17 beta | `d267e3dbd2df4e6849bdb2a81aa0b920c354efa6` | Historical classifier natives rejected by existing base boundary |
| 1.16.5-36.2.34 |0 /4 | ModLauncher /8 legacy | `ab306b654c44d659ce69da5d4f87590b66dc91e8` | Different profile/launch and historical native requirements |
| 1.12.2-14.23.5.2859 |0 /0 | LaunchWrapper /8 legacy | `6b3d5be8b58d5385fda9c4d285367f4cb6e697b2` | Legacy argument/FMLTweaker generation; zero processors is not compatibility |

All official installers were SHA-1 checked before profile/source inspection.
Library/digest provenance, inheritance, generated-output and recovery needs
differ by generation. Existing stores/receipts could be reused only after
separate exact planning/metadata/native normalization and acceptance work.
None is implemented or offered. No F8H or speculative legacy adapter added.

## 21. Remaining owner decisions

Confirm authenticated disposable-world gameplay and normal return to Ready
for both new pins. Report actual versions and any launch/runtime symptoms.
Choose whether to authorize a separate older-generation slice after F1C
acceptance; no authorization is assumed. No schema or production release
decision is required or taken by this implementation.

## 22. Recommended next phase

Complete the two owner gameplay checks, record evidence and update this
verdict/matrix only after confirmation. Then review F3R durability or a narrowly
authorized historical slice as separate work. Do not enable additional builds
from catalog appearance alone. Local review is the stopping point.
