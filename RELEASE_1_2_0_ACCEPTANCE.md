# Aurora Launcher 1.2.0 release acceptance record

Date: 2026-09-28. This record covers the pre-publication release-candidate
gates for Aurora Launcher 1.2.0 (scope: E1 + E2 + E3 + Aurora Client 2.1.5
bridge-v2 activation, plus release-process hardening). Publication itself
follows the established draft-first immutable procedure and is recorded
separately; nothing here claims a published release.

## Baseline and repository safety

Starting state: local HEAD and `origin/main` both at
`eed3c9983c1e0f99c8d945cd838218a4b746ecc7`, 0 ahead / 0 behind, 206 tracked
paths. The pre-existing working-tree entry for `src-tauri/Cargo.toml`
(line-ending-only, empty content diff) and the 609 protected local
diagnostic/evidence files were preserved byte-for-byte. No `git clean`,
reset, restore, recursive cleanup, or line-ending normalization occurred at
any point. Historical 0.1.0/1.0.0/1.1.0 installers coexisting in the bundle
output directories were preserved.

## Release scope audit (Gate 2)

Public 1.1.0 resolves to tag `v1.1.0` → commit
`97b9849e2fb023af74baed01b6b79d15d3165d80` (release ID 397949176, NSIS
setup plus `release-assets.json`, draft=false, prerelease=false). The
fifteen commits from there to `eed3c99` categorize completely:

- Release/process hardening (1): `33713a9` partial-release recovery — the
  publish state machine (`release-state.mjs` + deterministic tests),
  draft-by-listing selection, asset name normalization; recorded in
  `RELEASE_1_1_0_ACCEPTANCE.md`.
- E1 gameplay history (2): `572b151`, `762c886`.
- E2 widgets and Quick Launch (7): `8688a48`, `edef685`, `7406fa4`,
  `086eb59`, `114919e`, `2ecf393`, `b7b5abc`.
- E3 cosmetics (4): `382437a`, `87b5274`, `1b0fe72`, `8196d07`.
- Aurora Client 2.1.5 activation (1): `eed3c99`.

No unexplained functional change exists in the range.

## Acceptance states (Gates 3–4)

- E1 `PHASE_E1_ACCEPTANCE.md`: accepted.
- E2 `PHASE_E2_ACCEPTANCE.md`: PASSED; visual acceptance passed via owner
  manual inspection of all ten states (owner attestation recorded verbatim).
- E3 `PHASE_E3_ACCEPTANCE.md`: PASSED; agent-native inspection of the
  combined E2+E3 window plus owner manual attestation of the remaining
  states; live cosmetic mutation deliberately not performed.
- 2.1.5 activation `AURORA_CLIENT_2_1_5_ACTIVATION.md`: accepted with
  independent public download verification, JAR audit, live acquisition
  through the native pipeline, and the public-artifact bridge-v2 interop
  smoke.
- Production trust re-verified for this release: `src-tauri/production/
  aurora-releases.json` first stable entry is 2.1.5 with exactly
  2,468,545 bytes and SHA-256
  `fdc344e28c95a93b84af4b4fb1e61f9d3cc5774339f753f45f603c901df324d6`;
  the public `v2.1.5` release still resolves to tag target
  `a5497f680a38883a40afc2011fe418c59821029b` with that exact asset digest.
  `BRIDGE_ARTIFACT_SHA256` equals that digest and the eligibility tests
  prove: correct 2.1.5 hash trusted; wrong hash, 2.1.3 digest, unknown
  artifact, and absent digest all denied; 2.1.3 is not reclassified as
  bridge v2.

## Version collision check (Gate 5)

Authenticated GitHub state at preparation time: exactly one release
(`v1.1.0`, public) and exactly one tag ref (`v1.1.0`). No `v1.2.0` tag,
release, draft, or installer asset exists anywhere. The workflow's
collision check re-verifies this at publication time.

## Release workflow immutability (Gate 6)

The current workflow already enforces the mandatory sequence: read-only
resolve on the exact audited SHA (equal to `origin/main`, version contract,
collision check), Windows build from that exact SHA with the full test suite
and exact-value production Discord configuration, reviewer-gated publish
(`launcher-production` environment) that transfers and re-hashes the same
bytes, creates a draft with explicit `target_commitish`, verifies draft
assets byte-exactly, publishes the existing draft once, and re-downloads the
public bytes for read-only verification. The hardened state machine carries
deterministic tests (`release-state.test.mjs`).

One stale hardcode was corrected before preparing 1.2.0: `publish.mjs`
required release notes to mention Aurora Client **2.1.3**. The notes
identity check now derives the current production client version from the
first entry of `src-tauri/production/aurora-releases.json` and lives in
`release-state.mjs` as `verifyNotesIdentity` with deterministic tests
(pass/stale-client/wrong-launcher-version). 14/14 release-state tests pass.

## Version and notes (Gates 7–8)

`1.2.0` synchronized in `package.json`, both root fields of
`package-lock.json`, `src-tauri/Cargo.toml`, and the `aurora-launcher`
entry of `src-tauri/Cargo.lock`; `tauri.conf.json` already derives from
`package.json`. `npm run verify:version` passes: 1.2.0. No source file
hardcoded 1.1.0. `RELEASE_1_2_0_NOTES.md` records the user-facing changes
without overclaiming; it satisfies the identity contract (launcher 1.2.0,
current production client 2.1.5).

## Release-candidate test suite (Gates 9–10)

| Check | Result |
| --- | --- |
| Rust library tests | 562 passed, 21 ignored, 0 failed |
| Icon integration tests | 6 passed, 0 failed |
| Frontend tests | 130 passed, 0 failed |
| Svelte/TypeScript | 0 errors, 0 warnings |
| `cargo fmt --check` | passed |
| `cargo check --all-targets` | passed |
| Frontend production build | passed |
| Tauri production build | passed (MSI + NSIS emitted) |
| Version contract | 1.2.0 |

Counts equal the recorded production-activation baseline exactly. The
targeted regression areas (config migration, instance registry, account
persistence and reauthentication, credential integration, Minecraft/Fabric
install, Java runtime acquisition, Aurora acquisition and exact-pin trust,
local mods and provider lifecycle including Modrinth, Home widget
persistence, E1 history, E2 widgets, Quick Launch validation, E3 preset and
cape persistence, account switching, Discord privacy filtering, Play and
duplicate-launch guards, release/version metadata) are all covered by the
passing suite above.

## Visual sanity gate (Gate 11)

The exact release-candidate executable (1.2.0, public Discord application
ID and 2.1.5 bridge digest verified embedded) was launched against the
existing launcher-managed data. Captured evidence (kept local/uncommitted
under `%TEMP%/aurora-1_2_0-visual/`): Home at 1168×847 with the full widget
layout; Skin Manager populated (current account skin · Classic, one local
preset, model and import controls) side-by-side with Cape Selector
populated (three owned capes, active cape shown disabled/Selected, Disable
action); Home at the enforced logical minimum 720×520 (900×650 physical at
125% DPI); Settings at that minimum size. The E2 widgets displayed their
honest empty states because no bridge-v2 gameplay history exists on this
machine yet — recorded accurately rather than fabricated. No clipping,
overlap, broken scrolling, unreadable text, incorrect ellipsis, missing
artwork, broken previews, widget overflow, or minimum-size regression was
found. After the session every launcher state file (config, accounts,
instances, skin presets) was re-hashed byte-identical; the owner's live
installed 1.1.0 process and its running session were never touched.

## Install and upgrade smoke (Gate 15) — performed

The owner ended the live 1.1.0 session and authorized performing the smoke
on this machine, and chose the public asset set MSI + NSIS +
`release-assets.json`. The complete installer cycle then ran against the
real per-user 1.1.0 installation and launcher-managed data root:

- **Upgrade 1.1.0 → 1.2.0 (NSIS, silent, in place)**: installed exe became
  exactly the release-candidate 1.2.0 binary (10,482,176 bytes,
  ProductVersion 1.2.0). `config.json`, `accounts.json`, `instances.json`
  and skin presets were re-hashed byte-identical after installation; the
  Start shortcut survived; the desktop shortcut was (re)created by the
  documented silent-install default with the slot empty and unowned. The
  upgraded install launched, reported **Aurora Launcher 1.2.0**, showed the
  signed-in account, the selected instance **Ready** still pinned to
  Aurora 2.1.2 (existing pins are never silently upgraded), and the E2/E3
  widget layout. Screenshots kept local under
  `%TEMP%/aurora-1_2_0-visual/06-…` and `07-…`.
- **Uninstall (silent)**: program executable, uninstaller registration
  (HKCU), Start and desktop shortcuts were removed;
  `aurora_launcher_lib.dll` initially remained because WebView2 teardown
  still held it seconds after app close — the lock was verified transient
  (the file opened exclusively immediately after) and is an environmental
  timing artifact, not an installer defect. The launcher-managed data root
  was preserved untouched.
- **Fresh install (NSIS, silent)**: installed 1.2.0 with correct
  registration (DisplayName Aurora Launcher, DisplayVersion 1.2.0,
  Publisher aurora), shortcuts, launch verified reporting 1.2.0 with the
  preserved account and Ready instance. The 1.2.0 installation was left in
  place on this machine.
- **MSI**: a quiet per-machine install fails fast with Windows Installer
  error 1925 (elevation required), as expected for its machine-wide scope;
  no elevated interactive install was forced on the owner's desktop. A
  non-mutating administrative extraction (`msiexec /a`) succeeded and
  unpacked the exact release executable (`PFiles\Aurora Launcher\
  aurora-launcher.exe`, 10,482,176 bytes), proving database and payload
  integrity. Interactive elevated MSI acceptance remains an owner step, as
  RELEASING.md already records for the MSI format.

## Publication record — 2026-09-28

Published through the hardened draft-first procedure: collision check clean
(`v1.2.0: create`), draft created with explicit target
`2b68091a0837b395f12c3b0ccec8bd2d08a18e42` (an empty-draft rediscovery
hiccup from GitHub listing lag was resolved through the designed
existing-draft path — no release was deleted or recreated), the approved
NSIS, MSI and `release-assets.json` uploaded once, verified byte-exactly
inside the draft, and the existing verified draft published exactly once
(**release ID 398171262**,
https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/v1.2.0).
The lightweight tag `v1.2.0` was created by publication at exactly
`2b68091a0837b395f12c3b0ccec8bd2d08a18e42` and verified independently.
One local-only packing note: Windows PowerShell 5.1 wrote a UTF-8 BOM into
`release-assets.json` (CI's pwsh does not); the BOM was stripped before
upload so the published manifest is BOM-less, 891 bytes, SHA-256
`a6c96021b6b30e5ae7ea823c53c06dca823e109ae7fb475cc752700a77429ed7`.

Post-publication verification was read-only: public metadata (draft false,
prerelease false, title and exact target SHA), GitHub's advertised asset
digests, and fresh unauthenticated downloads of all three assets re-hashed
byte-exact against the approved values (GitHub stores asset names with
spaces normalized to dots; the manifest agrees with the downloaded bytes
under that documented normalization). The publicly downloaded NSIS setup
silently reinstalled over the existing 1.2.0 installation and the installed
launcher booted the full Home layout and closed cleanly. As with 1.1.0, the
bare build-tree exe, the MSI payload exe, and the NSIS-delivered exe differ
in Tauri bundle-type bytes and PE timestamps; installer-byte identity is
carried by the hash-pinned installers, not exe byte-equality. Launcher 1.1.0
(release, tag, both assets) and the Aurora Client 2.1.5 public release were
re-verified untouched. No force push occurred; working tree, tracked set
(208), and the 609 protected untracked diagnostics are unchanged; no
installer or screenshot was committed.
