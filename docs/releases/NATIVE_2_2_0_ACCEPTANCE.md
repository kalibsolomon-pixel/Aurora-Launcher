# Aurora Launcher 2.2.0 acceptance

This release promotes the owner's polished GPUI rebuild and independent Client
release discovery. The production workflow compiles `native-gpui/` and uses the
existing signed native updater, installer identity, public key and authority.
The legacy schema-1 runtime marker is retained for shipped clients.

## Client discovery and updates

Live official GitHub checks on 2026-11-14 acquired and inspected the expected
SHA-256-verified JARs in a new disposable root. Both new-instance discovery and
existing-environment update discovery returned the same artifacts:

| Minecraft | Client | SHA-256 |
| --- | --- | --- |
| 26.1.2 | 3.4.0 | `7df204d3c59892d10fd16a3a382d2ace27383aaed0682fbb0688a3d707b37100` |
| 26.2 | 3.4.0 | `7dce642c6fdadc8788eed24db1e9a73bb82d6ecffe7ebaab71fee3727ec81b9e` |
| 26.3 | 3.4.0 | `854ee8a7ff9adc4810a9f8f6df68b7f1338ea380a68ff8b9b008c44e32df5fe6` |
| 1.21.11 (legacy) | 3.2.1 | `63fd2ca0ec12bb2bde708e38f81daed0b3b0f86c46e64d6edd84d30c0e90b992` |

The three modern checkpoints retain Loader 0.20.1 and Java 25. Legacy retains
Loader 0.19.5 and Java 21. Existing Fabric API pins remain unchanged. The checks
created no instances and changed no existing instance. Minecraft 26.1 and 26.1.1
are not aliases for the supported 26.1.2 artifact.

Regression tests cover a three-JAR GitHub tag, shuffled asset order, duplicate
and generic/suffixed ambiguity, missing/invalid digest, published state, foreign
URLs, JAR identity and dependencies, exact environment resolution, and retaining
the legacy release. Existing update transaction and rollback tests also pass.

## Local verification

- Core: 1,212 passed, 34 explicitly ignored external/live tests.
- GPUI: 386 passed, including the available Launcher offer contract.
- Release tooling: 16 passed, including GPUI build/package routing and signed
  envelope, immutable publication and authority read-back contracts.
- Core all-target check and core/GPUI/retained-host formatting passed.
- Optimized Windows x64 GPUI build and NSIS packaging passed. Executable metadata
  is Aurora Launcher, version 2.2.0, original filename `aurora-launcher.exe`.
- Package guards reject diagnostic/acceptance strings and retain the existing
  installation, data and credential identities. Release-only roots cannot enter
  debug showcase mode.

Local packaging is unsigned and is not the production publication artifact.
The protected workflow rebuilds the exact reviewed main SHA, signs its exact
installer and envelope, and revalidates those bytes before publication and
native authority advancement. Publication status must be established from that
workflow and independent public read-back. Authenticode is still unconfigured.

The earlier UI review captures remain under `native-gpui/design-reviews/`.
No new performance claim is made by this promotion. A production signed GPUI
update/restart must be distinguished from unit tests and unsigned local packaging;
these local checks do not claim to have replaced the owner's installed launcher.
