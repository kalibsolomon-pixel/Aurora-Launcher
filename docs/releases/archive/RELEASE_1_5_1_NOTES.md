# Aurora Launcher 1.5.1 — release notes

## Fixed

- Check for Updates now discovers the latest published stable Aurora Client
  directly from the official Aurora Client GitHub repository. It no longer
  depends on the separate legacy release-manifest asset or a new Launcher build
  for every Client release.
- New compatible Aurora instances and explicit Aurora enablement use the same
  GitHub release discovery. Existing instances keep their exact installed pins
  until you explicitly apply an update.
- Remote Client discovery runs separately from startup account and Play-readiness
  loading, so a slow release service does not delay those checks.

## Verification and compatibility

GitHub must publish an expected SHA-256 for the Client JAR. The Launcher verifies
the downloaded JAR in its trusted cache before reading its Fabric identity and
dependency requirements. Minecraft, Fabric Loader, Fabric API and Java
compatibility remain enforced. Updates retain the existing preview, fingerprint,
transaction, backup and rollback protections.

Aurora Client 3.0.0 instances in the supported Minecraft 1.21.11 / Fabric Loader
0.19.5 / Java 21 environment can discover the independently published Aurora
Client 3.1.0. Future compatible stable Client releases use the same discovery
path; the Client version is not pinned by this Launcher release.

## Known limitations

- Public Launcher distribution remains Windows x64 NSIS. No Linux release is
  included.
- Production Launcher updates use Aurora's updater signature. Windows
  Authenticode publisher signing remains unconfigured.
- GitHub availability and API limits can make a check temporarily unavailable;
  discovery never substitutes an unverified JAR or silently downgrades an
  installed Client.
- A Client release requiring a different Minecraft/loader/runtime environment
  is rejected rather than changing an existing instance automatically.
