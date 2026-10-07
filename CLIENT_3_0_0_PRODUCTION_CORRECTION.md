# Client 3.0.0 production-source correction — 2026-10-07

The owner explicitly approved correcting the bundled production authority to
Aurora Client 3.0.0. New compatible instances in a corrected build select 3.0.0;
existing exact pins are preserved. This is local source acceptance, not a new
Launcher publication or replacement of an existing immutable installer.

## Starting state and scope

Started on clean tracked `main` at
`4575a0e3f1c28da6cfe072948fe91b60eeaf608f`. Local origin/main, remote HEAD/main
and merge-base all matched that SHA; ahead/behind was 0/0. Created
`codex/client-3-production-authority` for one scoped local commit.

Recorded 392 tracked files and all 15,143 protected non-ignored untracked paths,
including each protected file's size and SHA-256, before implementation.
Final comparison found no missing or changed protected files. Generated build
outputs were rebuilt through existing commands and are not release artifacts.
No credentials, environment files, binaries or generated directories are staged.

Changed files:

- `src-tauri/production/aurora-releases.json`: add verified 3.0.0 first; retain
  the original 2.1.5, 2.1.3 and 2.1.2 entries without changing their metadata.
- `src-tauri/src/distribution.rs`: verify all four releases and exact 3.0.0
  artifact identity, Minecraft, Loader, Java and Fabric API requirements.
- `src-tauri/src/instances/lifecycle.rs`: assert new production creation selects
  3.0.0 and older exact pins remain; make live isolated creation acceptance
  assert Client 3.0.0 and readiness.
- `src-tauri/src/instance_mods.rs`: historical ownership fixture selects its
  exact 2.1.5 entry instead of assuming it is the first catalog entry.
- `src-tauri/src/launch/activity_bridge.rs`: exact verified two-artifact allowlist;
  test both production digests and reject tampering/inactive/incompatible cases;
  assert normal exit 0 in the opt-in shipped-worker interoperability test.
- `src-tauri/src/launch/resolve.rs`: current opt-in game acceptance verifies the
  3.0.0 filename, digest and size from the catalog rather than an obsolete constant.
- `tools/release/release-state.mjs`: one deterministic current Client identity
  derived by stable semantic ordering of the bundled catalog; exact notes identity
  matching rejects version prefixes and suffixes.
- `tools/release/release-state.test.mjs`: current identity, mismatch, ordering and
  malformed catalog checks; original explicit historical identity fixtures remain.
- `tools/release/updater-release.mjs`: current candidate identity; existing
  authority records may name reviewed immutable catalog identities; installer URL,
  signature, digest, UTF-8 and provenance verification remain mandatory.
- `tools/release/updater-release.test.mjs`: current fixtures, incorrect Client
  rejection, historical authority compatibility and rejection before mutation.
- `tools/release/publish.mjs`: use the centralized identity rather than a second
  first-entry implementation.
- `tools/release/publish-manifest.mjs`: require current Client identity for new
  publication candidates while preserving existing authority inspection.
- `README.md`, `RELEASE_1_4_1_NOTES.md`, `RELEASE_1_4_0_NOTES.md`,
  `ARCHITECTURE.md`: current target and corrected-source behavior, with explicit
  guidance for the unchanged published installer.
- `CLIENT_3_0_0_PRODUCTION_CORRECTION.md`: this superseding review record.

No production ref, public manifest, immutable release, signing key, launcher
version, workflow or remote service was modified.

## Exact production artifact

The downloaded official [Client 3.0.0 artifact](https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/v3.0.0/aurora-3.0.0.jar)
is 3,077,377 bytes, SHA-256
`43f918207a86f91b01b35045309e89d2b040951722e5ac71d01d886beec9811c`.
Its embedded descriptor declares version 3.0.0, Minecraft `~1.21.11`, Java
`>=21`, Fabric Loader `>=0.16.0` and Fabric API. The source catalog entry matches
the already published Client update-manifest entry exactly: stable, Minecraft
1.21.11, Loader 0.19.5, Java 21 and Fabric API 0.141.6+1.21.11. Fabric API's
existing expected size and digest are retained.

The old catalog entries compare equal to the original baseline entries. Their
artifact URLs, sizes, hashes and dependency pins are unchanged. The production
creation selector still chooses the newest compatible semantic version; no
instance registry migration or silent update was introduced.

## Bridge compatibility proof

Both official JARs were downloaded independently and hashed. Client 2.1.5 retains
SHA-256 `fdc344e28c95a93b84af4b4fb1e61f9d3cc5774339f753f45f603c901df324d6`.
All eight compiled classes under `com/aurora/client/launcher/` in 3.0.0 are
byte-for-byte identical to 2.1.5. The release-tag source diff for that package
is empty. Client initialization still registers `LauncherActivityIntegration`.

Verified unchanged contracts:

- Bootstrap: `AURORA_ACTIVITY_ENDPOINT`, `AURORA_ACTIVITY_SESSION_ID`,
  `AURORA_ACTIVITY_CAPABILITY`, `AURORA_ACTIVITY_PROTOCOL`; literal IPv4 loopback,
  UUID session, bounded capability and explicitly selected protocol 1/2.
- Startup: one daemon connection attempt, bounded worker queue, initial MAIN_MENU,
  bounded hello carrying type/schemaVersion/sessionId/capability, exact accepted
  response, sequence-numbered newline-delimited UTF-8 activity messages.
- States: MAIN_MENU, SINGLEPLAYER and MULTIPLAYER; display names/address and v2
  worldSaveId/serverTarget fields retain their schema and privacy projection.
- Lifecycle: client started/world changed/join events project state; disconnect
  explicitly reports MAIN_MENU; stopping closes the bridge. No schema migration.
- Launcher authentication, constant-time capability comparison, frame bounds,
  deadlines, sequence validation and nonfatal failure remain unchanged.

The explicit Java/Rust v2 interoperability test was run against classes loaded
from each downloaded JAR, using the same external package-local harness compiled
with `--release 21` and the officially resolved managed Java 21 runtime. Both
passed with supervised exit 0, including validated world-save and normalized
server-target identities. Exact allowlist tests deny all other
digests; adding a future catalog version does not automatically grant capability.
Recent Servers and Playtime/history handling were not disabled or bypassed.

## Release-tooling compatibility

New packaging/publication candidates must name the current production Client
derived from the bundled stable catalog, now 3.0.0. Existing authority history
can still be inspected when its notes name a reviewed immutable older identity.
Unknown identities fail. A candidate naming the older Client fails before Git
publication mutations. Same-version drift/downgrades, wrong signatures/keys,
signed filename/version mismatches, wrong artifact bytes and provenance conflicts
continue to fail. No cryptographic verification was removed or made optional.

## Current documentation versus history

README's old candidate/production selection and current release-note selection
claims are corrected. Both 1.4.x notes use a dated superseding correction rather
than claiming old installer bytes changed. The current Architecture summary is
updated; earlier phase descriptions remain historical where superseded.

Preserved: `AURORA_CLIENT_2_1_5_ACTIVATION.md`, dated acceptance reports,
`CORRECTION_SOURCE_REVIEW.md`, Phase H/J/L acceptance evidence,
`RELEASE_1_4_0_PREPARATION.md`, historical sections of `RELEASING.md`, and older
1.2.0/1.3.0/1.3.1 release notes. Exact-pinned fixtures, update-comparison examples
and isolated visual-review state remain valid historical/test cases.

## Verification and limits

- Focused production creation and exact artifact support checks: passed.
- Release tools: 100 tests passed, including updater signing/provenance,
  historical authority advancement and mismatched current identity rejection.
- Full Rust: 833 unit tests and 6 icon integration tests passed; 29 existing
  opt-in tests remain ignored in the default run. Four existing test-build
  warnings remain; no unrelated automatic fixes were applied.
- Frontend: 229 tests passed, 16 suites; type check found 0 errors/0 warnings;
  production static frontend build passed.
- Rust formatting and all-target check: passed.
- Real official-source isolated creation: two new 3.0.0 instances reached Ready;
  warm reinstall and second-instance cache reuse passed. The readiness check
  verifies registry, game installation and exact artifact/dependency metadata.
- Real managed Java: resolved Java 21; cold provision and warm validation/reuse
  passed in that same new controlled root.
- Standalone shipped-worker interoperability for both JARs: passed. This proves
  protocol compatibility, not an authenticated full Minecraft gameplay session.
- Native optimized build and proper Tauri CLI no-bundle build: passed, with an
  external configuration selecting a distinct diagnostic app identifier/title.
- Native boot acceptance is blocked: the isolated executable exits before a
  targetable window with Windows exception `0xc0000409` (Application event 1000).
  Retrying the normal Tauri CLI build did not resolve it. No diagnostic app-data
  directory was created; the already running owner installation was not touched.
  No authenticated game/menu/server playthrough or normal game quit is claimed.

Evidence is outside Git in `C:\Dev\Aurora-Client-3-Authority-Evidence`, including
initial refs/inventories/protected hashes, downloaded JARs, per-class comparison,
creation/runtime/interoperability/full-test/build logs and final safety checks.
The newly generated temporary game/runtime folder is retained for owner review.
No normal launcher data, credentials or `.minecraft` files were copied or changed.

## Publication boundary

The current public Launcher is still 1.4.1. Its published installer retains its
original creation catalog; this local source correction cannot retroactively
replace its immutable bytes. Its users create a compatible instance, then use
Check for Updates and explicitly apply the already published Client 3.0.0 update.
Existing pins stay untouched until that explicit action.

Shipping the corrected creation default needs a separately owner-approved future
Launcher build/release. No push, merge, tag, release publication, authority-ref
advancement, workflow dispatch or deployment occurred in this correction task.
