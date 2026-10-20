# Aurora Launcher 1.5.0 release acceptance

## Owner-accepted release scope

On 2026-10-20 the owner accepted the reviewed Forge configurations, including
Minecraft 1.20.1 and 1.20.4; the reviewed Modrinth integration and existing-instance
modpack merges; and the UI, including player/library sizing and layout.
These are owner acceptance decisions, not additional automated or authenticated
test executions. Historical evidence retains its original classifications.

## Source integration

The release integration starts from production main
`9890a9c8ba2b0fb64508df60d9ea8fd860a21f64`. It incorporates the production source,
tests, dependency locks and configuration from tested candidate
`ada46c63ff0c04c110536830105c2c9efd9b206f`, plus curated operating guides.
The original development/preparation branches and internal evidence are retained
locally. They are not ancestors of this integration. Internal recordings,
machine-specific capture scripts and diagnostic inventories are excluded from
new public history. No original branch was rewritten or force-pushed.

Application source equivalence, actual newly reachable Git objects and current
verification results are recorded in the release operator's acceptance evidence.
The final reviewed main SHA must match the protected CI source and the immutable
release asset inventory. A passing local build is not artifact-specific approval.

## Windows symbolic-link security acceptance

**PASS, 2026-10-20:** the unchanged audited driver and exact
`install::retention::tests::windows_file_symlink_refusal_helper` ran in the release
profile under ordinary privileges against an existing complete isolated fixture.
Both the real file symlink and dangling file symlink were observed. Two actual
retention attempts refused the linked backup, retired/reclaimed nothing,
preserved inventory/authority/sentinel hashes and deeply validated current game.
The helper and wrapper emitted the required UNELEVATED verification/PASS lines.
Exit 0; 1 passed, 0 failed. An independent 44-entry before/after snapshot matched.
No elevation, security-setting change, cleanup or replacement fixture was used.

The historical error-1314 result described fixture-creation privileges; it was
not a passing refusal test. The newly completed ordinary-user verification closes
that acceptance gate without changing a production security check.

## Production requirements

Release platform: **Windows x64 NSIS**. Linux packages and Linux secure account
persistence are not part of 1.5.0. Updater minisign authentication remains
mandatory; Windows Authenticode publisher signing remains unconfigured.

New compatible Aurora instances retain Client **3.0.0**, Minecraft **1.21.11**,
Fabric Loader **0.19.5**, Fabric API **0.141.6+1.21.11** and managed Java **21**.
Existing instance pins and conservative 2048 MiB defaults remain unchanged.
Advice modes are opt-in; no FPS/CPU improvement or new measured JVM preset is
claimed. Iris/Vulkan compatibility is not established by this release.

## Remaining artifact and publication gates

The protected workflow must build/sign the exact reviewed main source. The owner
must review the exact signed installer identity and accept installation,
authenticated general Minecraft smoke, normal exit and disposable 1.4.2 upgrade
before approving immutable publication. The separate authority environment gates
manifest-last discovery advancement. No environment approval is inferred from
this document or from general feature acceptance.

After publication, independently verify installer/signature/inventory hashes,
source/tag provenance, canonical update authority and real updater discovery.
Update the independent website's static Launcher metadata using the actual
published sizes/hashes/date and its own deployment controls, preserving Client
3.0.0 and Windows-only distribution. Until those operations complete, this
document is an acceptance record in progress, not a release announcement.
