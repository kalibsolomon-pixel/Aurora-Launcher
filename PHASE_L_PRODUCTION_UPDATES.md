# Aurora Launcher — Phase L: Production Updates

Phase L implements Aurora's production update system: Aurora **Client**
updates (the versioned mod artifact the launcher already owns) and Aurora
**Launcher** self-updates (the official Tauri 2 updater). These are two
deliberately separate domains with distinct artifacts, trust roots,
activation lifecycles, and rollback semantics; they share release-channel
policy, presentation concepts, and the in-process update center — nothing
else.

Updates are **release-driven, never commit-driven**: only intentionally
published release metadata (the manually gated public Aurora Client release
manifest; the owner-published signed launcher update manifests) may become
update candidates. A pushed commit, a green CI build, or a locally created
tag is never an update. Nothing in this phase publishes anything; the
owner's publication gate remains manual.

## Release authority

### Aurora Client

The authoritative input is the **published release manifest** at the
owner-controlled location

```
https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/release-manifest/aurora-releases.json
```

(constant `updates::client::PRODUCTION_MANIFEST_URL`). It uses the same
schema-1 release-manifest model the launcher has always embedded
(`src-tauri/production/aurora-releases.json`), extended by one optional
field: per-release `notes` (bounded plain text, 8 KiB cap, no control
characters beyond newline/tab). Attaching this asset to a dedicated
`release-manifest` release is a manual owner action — until it exists,
Client update checks fail non-fatally as *Could not check for updates*.

Honest trust statement (unchanged from the embedded-manifest model): the
remote manifest is bootstrap discovery metadata authenticated by HTTPS plus
strict parsing — the same class as the official Mojang version manifest. It
is **not** independently signature-verified, and the SHA-256 values inside
it verify downloaded artifacts against the manifest; compromise of the
manifest origin could replace artifact URL and expected digest together.
No signing infrastructure exists for the Client manifest today; none is
invented. When the future website owns publication, it should publish
(mirror) this same document rather than becoming a second competing
release database.

### Aurora Launcher

The authoritative input is the owner-published **Tauri update manifest**
per channel, at

```
https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/launcher-updates/<channel>.json
```

(constant `updates::launcher::UPDATES_BASE_URL`, endpoint
`<base>/{stable,beta,nightly}.json`). The updater plugin fetches it,
compares versions (semver, greater-than only — no downgrades), downloads
the platform artifact, and verifies its **minisign signature** against a
public verification key compiled into the launcher
(`AURORA_UPDATER_PUBKEY`, see *Signing model*). Signature verification
cannot be disabled. Additionally, the plugin verifies the signed trusted
comment against the announced version when the CLI records it, preventing
a tampered endpoint from pairing a new version number with old bytes.

## Signing model and trust root

- The launcher self-update trust root is a **compile-time embedded public
  key** (`option_env!("AURORA_UPDATER_PUBKEY")` → `UPDATER_PUBKEY`).
- Production builds are compiled **without** any key until the owner
  generates the production keypair (documented owner action below). Such
  builds report launcher updates as honestly *not configured* —
  `updater_unconfigured` — and never download or run anything.
- Diagnostic/acceptance builds compile the diagnostic public key and
  loopback endpoints via `AURORA_UPDATER_PUBKEY`,
  `AURORA_LAUNCHER_UPDATES_URL`, and `AURORA_CLIENT_MANIFEST_URL`
  (compile-time only; runtime overrides and frontend-supplied URLs or keys
  do not exist).
- The **private signing key is never committed, embedded, or logged**.
  Build-time signing uses `TAURI_SIGNING_PRIVATE_KEY`, which exists only
  in the release environment.
- `tauri-plugin-updater` is pinned to **≥ 2.12** (CVE-2026-95624 fixed
  there).

### Production publication requirements (owner actions)

Before real production launcher self-updates can ship:

1. Generate the production keypair once:
   `npm run tauri signer generate -- -w <owner-secret-path>/aurora-launcher.key`
   (password-protected; store in a secret manager, never in the repo).
2. Configure the release workflow to set:
   - `TAURI_SIGNING_PRIVATE_KEY` (+ `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`)
     at build time,
   - `AURORA_UPDATER_PUBKEY` to the production public key content,
   - `bundle.createUpdaterArtifacts: true`.
3. Publish each release's `stable.json`/`beta.json`/`nightly.json` (v2
   artifacts: the NSIS `.exe` + `.sig`, with `version`, `notes`,
   `pub_date`, `platforms.windows-x86_64.{url,signature}`) as assets on
   the dedicated `launcher-updates` release, per channel, in the same
   manual gate.
4. For Client updates: attach `aurora-releases.json` (all channels, full
   history the launcher should see) to the dedicated `release-manifest`
   release of the Aurora-Client repository whenever a release is
   published.

Until 1–3 exist, launcher update checks correctly report unavailability —
that is the honest state, not a defect.

## Release channels

`stable` | `beta` | `nightly` (the model Aurora always had:
`distribution::ReleaseChannel`). Channel selection lives in Settings and
persists in the launcher configuration (schema 7, `updates.channel`,
default `stable`, explicit migration from schemas 1–6 which selects
stable — no historical policy is fabricated).

Eligibility is monotonic and policy-driven
(`updates::eligible_channels`):

- **stable** → stable releases only
- **beta** → stable + beta
- **nightly** → stable + beta + nightly

Changing the channel never mutates installed software; it invalidates
cached availability and redefines what future checks consider. "Newer" is
**semver precedence only** (`semver` crate); malformed version strings
never become candidates, and an installed Client newer than everything
eligible is reported truthfully as `installedNewer` — never silently
downgraded to a manifest version.

## Aurora Client update architecture

```
DISCOVER  fetch published manifest (bounded 1 MiB, HTTPS/loopback, strict parse)
→ PLAN    preview_update: read-only, compatibility, blockers, fingerprint
→ USER CONFIRMATION        (the Update action in the instance workspace)
→ ACQUIRE verified SHA-256 store (expected digest from the manifest)
→ STAGE   .aurora-update-stage-* beside destination, re-verified bytes
→ REVALIDATE  fingerprint re-derived under instance+registry locks
→ ACTIVATE hard-link staged artifacts; retire old artifacts by rename
→ VALIDATE whole-instance validate_instance must report Ready
→ COMMIT  aurora-installed.json + aurora-release.json, then the registry pin
→ cleanup retired artifacts
```

- **Compatibility is exact environment identity**: the candidate release's
  Minecraft version and Fabric Loader version must equal the instance's,
  and the release's Java major must equal the resolved game plan's Java
  requirement (asserted at apply through the same resolver transitions
  use). Anything unprovable fails closed (`update_incompatible`).
- **Running instances cannot mutate**: a Running/Starting game process is
  a named preview blocker and a hard apply refusal (`lock_stopped` is
  taken inside the transaction) — backend-enforced, not a disabled button.
- **The registry pin moves only after commit**: the old valid jar stays
  recoverable (renamed, not deleted) until the new release has activated
  and the whole instance re-validated; any failure rolls back activated
  links, retired artifacts, the installed-state document, the release
  sidecar, and the registry bytes to their exact prior content.
- **Stale plans are refused**: the fingerprint hashes the registry record,
  installed-state bytes, retained ownership, provider state, the mod
  inventory, the installed game manifest, the full candidate release, and
  the selected channel — re-derived twice (before acquisition and under
  the locks).
- **Ownership**: the update touches exactly the Aurora artifact it owns
  and its pinned Fabric API. User mods, configs, saves, resource/shader
  packs, provider-managed content, and pack-owned content are untouched
  (proven byte-for-byte in tests). A provider-owned Fabric API is carried
  across only when it exactly matches the release's pin.
- **Resolvability after update**: a version newer than the embedded
  manifest would strand validation and launch (which resolve pins from the
  bundled manifest). The transaction therefore persists the exact
  published release entry as `instances/<id>/aurora-release.json`
  (schema 1, launcher-owned, embedded entries always win, malformed fails
  closed). Resolution (`resolve_optional_aurora`),
  provider reconciliation (`verified_required_mods`), validation, and
  transitions consult embedded ∪ sidecar. Reconfiguring an instance whose
  platform identity is unchanged now *preserves* the installed pin — a
  settings re-apply can never silently downgrade a remotely updated
  Client.

## Launcher self-update architecture

`tauri-plugin-updater` (registered in `lib.rs`) driven entirely from Rust
(`updates::launcher`); no frontend capability is granted. Lifecycle:

```
Checking → Update available → Downloading (real bytes/total from the updater's
own callbacks; no fabricated percentages) → Ready to install → Installing
→ (Windows: the verified NSIS installer runs passively and the app exits;
the installer relaunches the new version)
```

- **Discovery** is read-only; installation only ever happens through the
  explicit user action.
- **Stale defense**: before downloading, the offer is re-checked and must
  still be the exact presented version (`update_stale` otherwise); install
  consumes exactly the signature-verified bytes the updater downloaded.
- **Failure leaves the current launcher usable**: an invalid signature,
  interrupted download, or install failure changes nothing
  (`update_install_failed`, "your current version is unchanged").
- **State across restart**: the managed data root is untouched by the
  installer; accounts, instances, appearance, widgets, Discord and update
  preferences survive verbatim — no second migration system exists.

## Update checking

- One bounded launcher check + one Client check (selected instance) after
  normal startup (`startup_update_check`, once per process, non-blocking,
  non-fatal).
- Explicit **Check for updates** in Settings → Updates (deduplicated
  in-flight; never doubles network work).
- No polling, timers, watchers, or background refresh anywhere; update
  services being unreachable never marks instances, accounts, or the
  launcher invalid — it surfaces as *Could not check for updates*.

## Concurrency

Backend-owned guards in the process-global update center: per-domain
check de-duplication (`begin_check`/`finish_check`), per-domain
transaction exclusivity (`begin_transaction`/`finish_transaction`,
`update_already_in_progress`), plus the instance content lock and the
registry lock inside the Client transaction, and the same-instance launch
exclusion (`lock_stopped`). Frontend button disabling is presentation
only.

## UI

- **Settings → Updates**: channel selector (stable/beta/nightly), current
  launcher version, selected instance's Client version, explicit Check,
  per-domain status, plain-text release notes (`white-space: pre-wrap`,
  never HTML), launcher Download/Install with truthful phase labels.
- **Home**: a restrained, dismissible notice when a launcher or selected
  instance's Client update is available; never blocks Play.
- **Instance workspace → Settings tab**: the Aurora Client update panel
  (check → candidate with notes/blockers → Update), reporting real
  transaction phases.
- Release notes render as text nodes only; remote note text is bounded
  and normalized and can never invoke markup or navigation.

## Configuration

Launcher configuration is now **schema 7** (`updates.channel`). Schemas
1–6 migrate explicitly to stable; malformed/future documents still fail
deliberately and are never overwritten. No secrets, no URLs, and no
timestamps enter the configuration: dismissed notices are session state,
and a stale cached availability must survive only until the next explicit
or startup check, which re-derives everything.

## Diagnostics and acceptance

Deterministic coverage lives in the ordinary test suites:
`updates::client` (19 transaction/policy/failure tests through the real
production path against loopback publishers), `updates::tests` (channel
policy, semver, guards), `updates::launcher` (endpoint policy, trust
gating), `config` (schema 7), `distribution` (notes validation, manifest
merge), plus frontend presentation tests (`updatePresentation.test.ts`).

Live-like acceptance uses disposable diagnostic builds
(`.zcode-diag/phase-l-production-updates/`, untracked): a loopback
publisher (`diagnostic-server.mjs`), synthetic diagnostic jars
(`make-fixtures.mjs`), a diagnostic signing keypair (`keys/`, never
committed, never production), and isolated launchers
(`build-diagnostic.ps1`: identifier `com.aurora.launcher.diagnostic`,
product name "Aurora Launcher Diagnostic", compile-time loopback
endpoints). The production installation, production launcher data, Client
2.1.5, and Launcher 1.3.1 are untouched. Evidence:
`.zcode-diag/phase-l-production-updates/`.

## Website handoff contract

The future Aurora website/distribution infrastructure must expose (or
mirror) exactly:

**Client** — one public document, schema 1:
`{ schemaVersion, releases: [ { auroraVersion, channel, minecraftVersion,
fabricLoaderVersion, java.majorVersion, notes?, artifact { url, sha256,
sizeBytes }, fabricApi? { version, artifact } } ] }` with HTTPS artifact
URLs and real SHA-256 digests, published only through the owner's manual
gate (recommended: the `release-manifest` release asset above so the
launcher's pinned URL works from day one).

**Launcher** — per channel, the Tauri v2 update manifest:
`{ version (semver), notes, pub_date (RFC 3339), platforms: {
"windows-x86_64": { url, signature } } }` plus the signed NSIS
installer (v2 updater artifact: the installer itself + `.sig`), published
per channel through the owner's manual gate.

The launcher is deliberately **not** hardcoded to a website that does not
exist; the GitHub-release publication model above remains authoritative
until the website can serve/mirror the same bytes and documents at the
same URLs or a later owner decision changes the constants deliberately.

## Manual publishing contract (unchanged gate)

```
develop → test → owner chooses release → release workflow invoked manually
→ artifacts built → integrity (SHA-256) / signature (minisign) generated
→ owner-controlled publication gate → public release metadata becomes visible
→ launchers may discover the update
```

No development commit alone can cross that boundary; nothing in this phase
publishes automatically.

## Limitations

- Production launcher self-update is **architecturally complete and
  deterministically tested**, but live production acceptance is blocked on
  the owner generating the production keypair and publishing real update
  manifests (documented above). Until then production builds honestly
  report *Launcher updates are not configured in this build*.
- The Client manifest is not signature-verified (HTTPS + strict parsing
  only) — the same honest posture as the embedded manifest; a signed
  manifest belongs to the website/release-infrastructure phase.
- Client update discovery offers the newest eligible release; there is no
  per-release deauthorization ("skip this version") mechanism yet.
- Launcher channel publication for beta/nightly depends on the owner
  maintaining `<channel>.json` assets; GitHub's `releases/latest` covers
  stable only, which is why the dedicated `launcher-updates` release is
  the documented location.
