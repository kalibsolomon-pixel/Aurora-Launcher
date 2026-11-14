# Independent Aurora Client discovery

The Client repository's published stable GitHub releases are the authority for
new Client discovery. Discovery checks the newest 30 publications and selects
the newest compatible Client for each managed Minecraft environment. GitHub's
single Latest designation does not hide maintained or archived Minecraft
checkpoints. A Launcher release is not required for each compatible Client
release within these environments. Drafts, prereleases, branch commits and CI
artifacts are excluded.

Rust requests the pinned official GitHub Releases API. It requires the exact
versioned Aurora JAR, expected SHA-256, uploaded asset state and bounded size.
Both historical `aurora-<version>.jar` and
`aurora-<version>-mc<minecraft>.jar` names are supported. A publication can contain one runtime JAR per distinct
Minecraft suffix, as in Client 3.4.0. Duplicate environments or a generic JAR
mixed with suffixed JARs are rejected. Discovery and installed update checks
share this asset validation. A Minecraft filename suffix must match the selected
environment, and never substitutes for verified dependency checks.
Acquisition and cache reuse verify bytes through the existing SHA-256 store.
Only verified JAR metadata supplies mod identity and Fabric dependency facts;
release prose is untrusted display text and is never parsed for compatibility.

The managed Minecraft/Fabric Loader/Java/Fabric API environment must satisfy the
verified mod dependencies. Historical embedded releases remain for existing exact
pins and environment policy; they no longer decide the latest Client version.
Unsupported new environments fail explicitly, without changing instance settings.

The verified 2026-11-14 release matrix is:

| Minecraft | Published Client | Fabric Loader | Fabric API | Java |
| --- | --- | --- | --- | --- |
| 26.1.2 | 3.4.0 | 0.20.1 | 0.155.3+26.1.2 | 25 |
| 26.2 | 3.4.0 | 0.20.1 | 0.161.0+26.2 | 25 |
| 26.3 | 3.4.0 | 0.20.1 | 0.166.0+26.3 | 25 |
| 1.21.11 (legacy) | 3.2.1 | 0.19.5 | 0.141.6+1.21.11 | 21 |

The 26.1 checkpoint targets **26.1.2**; 26.1 and 26.1.1 are not declared
compatible. All three public Client JARs were checked against GitHub's expected
SHA-256 and inspected for mod identity and dependencies. Fabric API bytes were
checked against Fabric Maven's published SHA-256, and the official Fabric Meta
API confirmed each exact Loader combination. The existing 1.21.11 environment
and historical Client pins remain available. Updates retain the installed
Minecraft, Loader, Java and Fabric API environment and never cross these rows.

Checks and previews modify only reconstructable cache resources. New instances
and explicit Aurora enablement use the same source. Exact release metadata stays
with the instance so retry, offline validation and launch work after restart.
Exact identity includes both Client version and Minecraft environment; a
version-only lookup refuses ambiguous multi-environment releases.
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
