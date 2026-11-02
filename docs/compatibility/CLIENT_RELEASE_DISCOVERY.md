# Independent Aurora Client discovery — Launcher 1.5.1

The Client repository's latest published stable GitHub release is the authority
for new Client discovery. A Launcher release is not required for each compatible
Client release. Drafts, prereleases, branch commits and CI artifacts are excluded.

Rust requests the pinned official GitHub Releases API. It requires the exact
versioned Aurora JAR, expected SHA-256, uploaded asset state and bounded size.
Acquisition and cache reuse verify bytes through the existing SHA-256 store.
Only verified JAR metadata supplies mod identity and Fabric dependency facts;
release prose is untrusted display text and is never parsed for compatibility.

The managed Minecraft/Fabric Loader/Java/Fabric API environment must satisfy the
verified mod dependencies. Historical embedded releases remain for existing exact
pins and environment policy; they no longer decide the latest Client version.
Unsupported new environments fail explicitly, without changing instance settings.

Checks and previews modify only reconstructable cache resources. New instances
and explicit Aurora enablement use the same source. Exact release metadata stays
with the instance so retry, offline validation and launch work after restart.
Existing instances move only through the explicit fingerprinted update/enable
transaction, retaining verification, backups, rollback and user-content protection.

Release discovery runs outside startup account/runtime/Play-readiness loading.
There is no polling, automatic update, new credential or signing infrastructure.
GitHub HTTPS metadata supplies the expected hash; it is not independently signed
Client metadata. Launcher updater signatures remain a separate trust domain.

Deterministic tests use local loopback metadata and known digest-addressed JARs.
The explicit `client_release_discovery` example performs live acceptance in a new
temporary root by default. An explicitly supplied managed root permits a read-only
selected-instance update preview; only cache objects are acquired.
