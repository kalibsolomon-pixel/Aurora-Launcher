# Protected native preview release

## Native 2.0.0 release continuation

The owner has authorized publication of Native 2.0.0, including the reviewed
responsive-widget and icon improvements. See the [release record](../releases/NATIVE_2_0_0_RELEASE.md).
The preparation-only authorization statements below describe the earlier
readiness phase; current owner release instructions supersede them.

The workflow now reuses the existing `launcher-production` signing environment
and `launcher-update-authority-production` authority App environment, with the
separate `launcher-native-preview-publication` gate. The existing and newly
prepared native environments have the same owner reviewer, exact `main` branch
policy, and prohibition on administrator bypass. Existing secrets stay in
their protected environments; no new signing identity or secret copy is needed.

For the first native authority publication only, the explicit manual workflow
input `initialize_authority=true` with an empty `authority_parent` requests
initialization through the dedicated App. After public signed artifact
verification, it requires an absent native ref, validates the canonical empty
tree and exact zero-parent initialization commit, then creates the ref
atomically. It refuses an existing ref. Subsequent releases supply the observed
40-character authority parent and leave initialization disabled. No WebView ref
is initialized, replaced or advanced by this native procedure.

This prepares the existing native updater, standalone NSIS package and GitHub
release/authority protocol. It does not authorize a workflow dispatch, push,
publication, authority initialization or installation over an owner's preview.
The WebView workflow, `v<version>` releases and `launcher-update-authority` ref
remain unchanged. Current acceptance: [docs/releases/archive/NATIVE_RELEASE_READINESS.md](../releases/archive/NATIVE_RELEASE_READINESS.md).

## Identities and protected configuration

Native retains `com.aurora.launcher.native-preview`, Windows x86_64,
`nsis-per-user`, independent numeric version ordering, `native-v<version>` tags,
and `Aurora.Native.Preview_<version>_x64-setup.exe`. Discovery is the existing
proposed `launcher-native-update-authority/native-update.json` endpoint. Native
offers bind product/channel/Slint/platform/architecture/package/source/version,
expected size/hash and both updater signatures. Neither stream can consume the
other's package. Native releases are prereleases with `make_latest=false`, so
the WebView release selected by GitHub's latest endpoint stays unchanged.

The protected workflow is `.github/workflows/native-preview-release.yml`.
It is manual, bound to the exact reviewed current `main` SHA, and defaults to
private build artifacts without publication or discovery advancement. It never
rebuilds between signing, the publication gate and authority advancement.

Owner configuration still required (read-only inspection on 2026-11-01 found
only the two existing WebView environments, and no native authority ref):

| Environment | Configuration |
| --- | --- |
| `launcher-native-preview-signing` | Required reviewer, exact `main` deployment policy, no administrator bypass; public `AURORA_UPDATER_PUBKEY`; owner-managed `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` |
| `launcher-native-preview-publication` | Separate required reviewer and exact `main`; same public `AURORA_UPDATER_PUBKEY`; only this job receives ordinary Contents write |
| `launcher-native-update-authority-production` | Separate required reviewer and exact `main`; same public key; existing authority App's public `AURORA_AUTHORITY_APP_CLIENT_ID` and protected `AURORA_AUTHORITY_APP_PRIVATE_KEY`; ordinary token is Contents read |

Use the established signing identity. The SHA-256 of its trimmed encoded public
key is `bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`.
The signing job checks this fingerprint before compiling. Do not rotate the key
or copy secrets into repository variables, files, diagnostics or chat. Reuse the
existing protected signing and authority App process; a new App is not required.
Prevent self-review where a distinct owner-authorized reviewer is practical.

Apply the three layered ruleset principles in [docs/releases/UPDATE_AUTHORITY.md](../releases/UPDATE_AUTHORITY.md)
to the **native ref**, retaining WebView protection. History integrity forbids
rewind/deletion without an App bypass; writer restriction permits only the
dedicated authority App; creation restriction blocks unauthorized initialization
or recreation. Contents permissions alone cannot restrict a token to one ref.
Prove ordinary Actions credentials cannot modify either authority, and the App
cannot bypass history-integrity rules. These are owner configuration checks,
not something local fixtures establish.

Only after separate authorization, initialize the native ref with one root
commit, canonical empty tree `4b825dc642cb6eb9a060e54bf8d69288fbee4904`, and exact
message `Initialize Aurora native preview update authority`. The first
advancement creates a tree containing only regular `native-update.json`.
Current first-publication initialization is performed only through the explicit
protected workflow input described above.

## Controlled acceptance and publication

1. Review/integrate the intended source on `main` separately. Preserve or
   separately review the pre-existing uncommitted responsive-widget work; it is
   not silently included in the readiness commits. Synchronize the existing
   version metadata for the owner-chosen native version. Do not move an existing
   release/tag. Native and WebView ordering remain independent.
2. Configure the environments/rulesets above and enable immutable releases.
   Dispatch only with explicit owner authorization, exact current main SHA,
   numeric version and reviewed minimum installed native version. `publish`
   and `advance_authority` are independent explicit inputs and protected gates.
3. Build/sign produces six files: native installer, installer `.sig`, original
   payload, payload `.sig`, signed envelope, and `native-release-assets.json`.
   Hashes and sizes cover all five signed-content files; the transferred
   inventory is rederived and checked before publication. The compiler and
   packager reject non-x64/native identity or acceptance-feature executables.
4. While the publication job waits for its reviewer, download this run's exact
   private artifact. In a disposable Windows profile/VM, accept those exact
   signed bytes, installed identity/version, native restart, UI focus/DPI checks,
   copied data and recovery. Accept the current run's hashes: a later run rebuilds
   and must be accepted again. This document does not authorize replacing the
   owner's installed preview.
5. Approve publication only for the accepted current artifact. The adapter
   atomically creates the exact native tag at the reviewed source SHA, checks
   that ref again, creates a draft, uploads without replacement, rehashes every draft asset,
   publishes immutable native prerelease bytes, and verifies unauthenticated
   public downloads. An existing tag/release is a hard stop. It never changes
   WebView latest or its authority.
6. The separately protected authority job rechecks the immutable public release,
   source SHA and every public asset. Supply the exact observed native authority
   parent. It verifies the previous signed envelope, refuses downgrade and
   same-version drift, prepares Git objects, then advances only the native ref
   by non-forced fast-forward **last**. An uncertain write is inspected, never
   blindly resent. Seven bounded public reads allow raw CDN propagation;
   exhaustion leaves the ref intact as PUBLISHED but not VERIFIED.
7. Complete owner-authorized installed production-signing update acceptance
   through the fixed public native discovery endpoint. Local test-key acceptance
   does not establish production signing, GitHub execution or CDN acceptance.

For that installed update, a previous preview built without authority opt-in
cannot discover an update merely because the endpoint is later created. In the
disposable Windows profile, first manually bootstrap an authority-enabled baseline
(for example 1.4.2) from a protected private build. Then accept a strictly newer
owner-chosen version through the fixed public native endpoint. The baseline need
not itself be publicly released. Synchronize `package.json`, `package-lock.json`,
both Rust `Cargo.toml` files and both `Cargo.lock` files before the owner-chosen
version build; the executable/package metadata guard refuses divergence. Native
2.0.0 retains independent native version metadata while WebView stays 1.5.1. No
version bump or production installation is performed by this preparation.

The draft-before-publication sequence follows GitHub's
[immutable-release guidance](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases).
Updater file signatures use the existing [Tauri signing CLI](https://v2.tauri.app/reference/cli/).

Updater minisign signing verifies update bytes. **Authenticode remains
unconfigured.** Current executable/installer publisher status is `NotSigned`.
The owner must either explicitly accept unsigned Windows publisher distribution,
or arrange an authorized certificate/Trusted Signing process. If added, sign and
verify the executable before packaging, then sign/verify the installer **before**
updater signatures and final hashes. Do not reuse hashes/signatures after changing
either binary. No certificate enrollment or Windows security setting is changed here.

## Local signed lifecycle (no production authority)

`native/tools/test-signed-update.ps1` creates a new marked `aurora-native9-*`
temporary root and random package/uninstall identity. It builds optimized
acceptance versions 1.4.2 and 1.4.3 (parallel ThinLTO; the ordinary distribution
profile is unchanged), installs only its temporary source package,
then runs the actual native discovery/download/staging/signature/digest/revalidation/
exact-PID handoff/NSIS replacement/restart/state-load path. Progress and world
sentinel hashes are checked. The private Ed25519 test key stays in the bounded
loopback helper's memory; only its public key and signed bytes are retained.

`native-update-acceptance` is an explicit compile-time feature requiring the
marked root, random identity, exact loopback origin, separate test key and version.
Its data root is compile-time fixed, including against inherited runtime
diagnostic overrides. Automatic acceptance does not load accounts, credentials,
sync or Discord. The executable is clearly labelled local acceptance and is
refused by distribution packaging. Normal builds contain neither a runtime
signature bypass nor an endpoint/key override. No test key enters a distribution
artifact. The optional desktop mode also remains within that compiled identity.

## Data compatibility, rollback and repair

Preview coexists with WebView in its existing separate install/data/credential
namespaces. No automatic WebView migration or stable-host replacement is enabled.
Switching back to stable uses its untouched original data; native-only changes
remain in preview and are not promised to appear in stable.

Before a real migration or downgrade, close both hosts and games and preserve a
complete data backup, with a relative-path SHA-256 inventory. Verify the backup
against the source before testing a disposable copy. Keep instance roots, worlds,
mods, packs, shaders, icons, cache/runtime references and sync backups/history.
Never copy Credential Manager entries or use `.minecraft` as a migration target.

The same-schema additions are not universally backward compatible:

- NATIVE-10's registry `configuration.java` is rejected by the older WebView
  reader. A supporting launcher can reset custom selections to managed first;
  never strip JSON fields with an older client.
- Current optional widget `grid`/`position` fields are rejected by pre-widget-grid
  configuration readers. Restore the complete compatible configuration backup.
  Do not assume a visible size change removes those persisted fields; no seamless
  geometry downgrade/reset UI is certified here.
- PANDORA-4 schema-2 sync policy writes `overrides`, including an empty object,
  and adds `servers`. The actual pre-PANDORA-4 core at
  `1d7c30738569b29fd6a32f85352aadce4ea69813` rejects the new policy and refuses
  saving over it. Disabling syncing alone does **not** remove this boundary.
  Restore a known compatible sync-policy backup, or explicitly preserve the
  newer policy/history/backups in a separate recovery copy before allowing an
  older launcher to initialize its defaults. Do not silently discard fields or
  history. Returning to the newer launcher should use the preserved newer copy.

The compatibility probe in `src-tauri/examples/native_release_data_probe.rs`
executes current and historical readers against marked synthetic copies. A
refused old sync write may add only the existing empty `.sync-settings.lock`;
all pre-existing data bytes must remain unchanged.

Installer downgrade is intentionally refused. Supported rollback is a reviewed
manual reinstall of an older compatible package with its complete compatible
data backup, while preserving the newer tree separately, or resuming the
untouched legacy installation. Never lower discovery authority to roll back;
publish a separately accepted higher repair version instead. Failed preflight
keeps old installed bytes. Interrupted `native-new.exe`/`native-previous.exe`
evidence is a repair conflict, not permission to delete it. Preserve and hash
both candidates, close the exact application, and use a separately reviewed
manual repair. No universal power-loss or multi-file journal guarantee is made.
