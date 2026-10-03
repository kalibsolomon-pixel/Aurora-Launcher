# Aurora Launcher — Phase L: Production Updates

Phase L has one public production release stream. Aurora Client and Aurora Launcher remain separate native update engines with different artifacts, trust roots, activation, and rollback. A single Settings action checks both and explicitly updates the applicable domains. There is no public release-channel preference, selector, channel eligibility ladder, or per-channel launcher endpoint.

This document describes the simplified correction. The original acceptance at `9b19f3af2acf1adfd92282d44727e5cb711ab966` remains history: 189 frontend tests, 813 Rust tests, 29 gated ignores, and the original Phase L diagnostic evidence. That evidence is preserved; the corrected acceptance is recorded in `PHASE_L_SIMPLIFIED_UPDATES_CORRECTION.md` and `.zcode-diag/phase-l-simplified-updates/`.

## Release authority

### Aurora Client

The single owner-controlled published release truth is:

```text
https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/release-manifest/aurora-releases.json
```

`updates::client::PRODUCTION_MANIFEST_URL` fetches this schema-1 document with a bounded request and strict validation. Selection chooses the greatest semver precedence newer than the installed version among intentionally published compatible entries. Minecraft, Fabric Loader, and the release Java assertion must match; apply additionally verifies the resolved game plan's Java authority. Equal/build-metadata-only versions are current; installed newer never downgrades. Malformed versions reject the document. Unpublished development fixtures, commits, branch names, and local builds never join remote discovery.

The contract includes `auroraVersion`, exact Minecraft/Loader versions, `java.majorVersion`, expected SHA-256 plus artifact URL/optional size, optional Fabric API metadata, and optional bounded plain-text notes. New entries may omit `channel`. Historical entries and persisted exact release pins retain their legacy classification for identity compatibility; it has no update eligibility or user preference role. Modrinth per-item Stable/Beta/Alpha policies remain a separate existing content feature.

The manifest is HTTPS-authenticated bootstrap discovery metadata, not an independently signed artifact. Artifact hashes verify against its pre-known expectations; compromise of its origin could replace both URL and digest. No Client manifest signing infrastructure is invented. A future website mirrors this same release truth, rather than creating a competing database.

### Aurora Launcher

The sole production manifest is:

```text
https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/launcher-updates/launcher-update.json
```

`updates::launcher::PRODUCTION_MANIFEST_URL` serves the official Tauri v2 static-manifest shape: `version`, optional `notes`/`pub_date`, and `platforms.windows-x86_64.{url,signature}`. Windows uses a signed NSIS installer. There are no stable/beta/nightly manifest alternatives. Compile-time `AURORA_LAUNCHER_MANIFEST_URL` is the diagnostic injection; it replaces the old base-URL override. No frontend/runtime URL, key, or hash override exists. Production transport is HTTPS; explicit loopback HTTP is solely the documented diagnostic path.

The official `tauri-plugin-updater` stays at the existing >=2.12 security floor (lockfile 2.13.1). Rust embeds the minisign verification public key through `AURORA_UPDATER_PUBKEY`. A build lacking it reports `updater_unconfigured` and never downloads or installs. The private key never enters source, configuration, DTOs, logs, or frontend code. Official signature and signed-version checks remain mandatory. Metadata checks have a 30-second timeout, downloads 300 seconds. Download rechecks the presented version; install rechecks version, URL, and signature, rejecting same-version drift before consuming the exact retained verified bytes. Windows runs the official passive installer and exits; its installer relaunches the updated application. The running version is never fabricated before that boot.

### Manual publication

Only owner publication of reviewed Client metadata and signed Launcher manifest/artifacts makes an update visible. Commits, tags, successful tests, and builds do not publish them. The existing release workflow is `workflow_dispatch` only, with exact audited source and reviewer-gated publication; this correction changes no workflow and publishes nothing. Updater signing and the single manifest publication still require explicit owner configuration.

## Configuration compatibility

Launcher configuration remains schema 7. Corrected documents contain no `updates` field. Schemas 1–6 use the existing explicit migrations for unrelated settings without adding an update preference. Original schema-7 `{updates:{channel:stable|beta|nightly}}` documents validate that legacy field strictly and discard it in memory. All three lead to exactly the same production behavior. Loading is read-only; the next ordinary atomic save omits it. Selection, appearance, widgets, Discord/privacy settings survive. Malformed legacy values, malformed documents, and unknown schemas fail deliberately and are never overwritten. No registry/game/Client pin migration or instance filesystem move is involved.

## One update action

Settings -> Updates shows concise installed Launcher/selected-instance Client versions and one primary action:

| State | Primary action | Behavior |
| --- | --- | --- |
| Idle | Check For Updates | One explicit aggregate check |
| Checking | Checking… | Disabled; both native domains checked once |
| Both current / Client not applicable | None Available | Disabled for 5,000 ms |
| Applicable offer | Update | One explicit operation for offered domains |
| Updating | Updating… / Downloading… / Installing… | Disabled; real phase and bytes only |
| Check failed | Check Failed — Retry | Fresh bounded check |
| Update failed | Update Failed — Retry | Retry the remaining domain; changed offers require a fresh check and new Update action |

The five-second reset changes local presentation only, creates zero network requests, and is cleared on view destruction. It is not polling. The pure controller is tested with fake timers and feeds the thin Svelte adapter used by the real UI.

If one service is unavailable and the other has a valid offer, the primary action remains Update for that known offer, with concise unavailable-service context. The unavailable domain never becomes current by assumption. Without any valid offer, a failed manual check produces Check Failed — Retry.

Client-only performs its fingerprinted native preview/apply. Launcher-only performs official download then installation with that same click. Both run **Client first, then Launcher**, never concurrently, so Windows exit cannot strand a Client operation. Client failure prevents Launcher mutation. Client success plus Launcher failure retains the committed Client version and reports the partial outcome honestly; retry touches only Launcher. Duplicate clicks share one in-flight operation, native aggregate checks reject overlap, and existing native transaction/instance/launch locks remain authoritative.

Client preview binds exact displayed instance/version and re-derives native compatibility, ownership and fingerprints. Acquisition requires expected SHA-256 through the verified store. Staging, recoverable old artifacts, whole-instance validation, registry commit last, and byte-exact rollback remain intact. User content, credentials, managed runtimes, and game data are not updated by the Client artifact transaction. Running games and stale previews refuse mutation.

## Startup and presentation

One bounded discovery check runs per process after local state initialization. Current and offline startup remain silent; actual available releases enable one restrained Home notice opening this same Settings area. No modal, dismissal bookkeeping, install-at-startup, polling, watcher, or background update loop exists. Startup failure never enters Play readiness. A manual check during startup waits and performs one fresh manual aggregate check.

The instance workspace shows concise selected-instance Client status plus View Updates, with no second manager or apply controls. Release notes are secondary What's New disclosure content, bounded and rendered as text nodes. Progress is indeterminate for phases without measured totals; Launcher bytes come from real updater callbacks. Errors expose concise Retry state rather than endpoint bodies or technical stacks. The Phase F glass shell, typography and responsive composition remain in use.

## Production owner requirements and website handoff

1. Generate and securely retain the real production updater keypair; embed its public key in production builds and supply the private signing key/password only in the release environment.
2. Enable official updater artifacts, sign the reviewed NSIS installer, and manually publish its immutable bytes and `.sig`.
3. Manually publish **one** `launcher-update.json` pointing to those exact platform bytes/signature. No public channels.
4. Publish reviewed Client entries with their pre-known expected SHA-256 into **one** `aurora-releases.json`. Keep exact compatibility requirements and useful history.
5. A future website presents/mirrors these two authorities. It does not invent another Client release truth, per-channel Launcher feeds, signing keys, or automatic commit publication.

Until signing/publication are configured, the production Launcher self-updater is honestly unconfigured. Existing production Launcher 1.3.1 and Client 2.1.5 are not replaced by this correction. Diagnostic artifacts and loopback metadata are isolated acceptance assets, never public releases. Integration, push, publication, and website work remain owner decisions outside this correction.
