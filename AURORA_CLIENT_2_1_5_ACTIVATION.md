# Aurora Client 2.1.5 production activation — 2026-09-28

This document records the **production activation event**: Aurora Client 2.1.5
became the reviewed production client artifact with bridge v2 active for its
exact identity. It is distinct from the earlier implementation acceptances —
E1/E2/E3 (`PHASE_E1_ACCEPTANCE.md`, `PHASE_E2_ACCEPTANCE.md`,
`PHASE_E3_ACCEPTANCE.md`) accepted the bridge/history/cosmetics **code** while
the production pin still pointed at Aurora Client 2.1.3; those historical
records are unchanged and remain accurate for their time. This activation
changed only production artifact trust metadata, tests, and documentation.

## Activated production artifact

- Aurora Client **2.1.5** (stable), tag `v2.1.5`, runtime source commit
  `a5497f680a38883a40afc2011fe418c59821029b` (Aurora-Client repository).
- Public release:
  `https://github.com/kalibsolomon-pixel/Aurora-Client/releases/tag/v2.1.5`
  (published via the hardened draft-first procedure; immutable).
- Asset `aurora-2.1.5.jar`, **2,468,545 bytes**, SHA-256
  `fdc344e28c95a93b84af4b4fb1e61f9d3cc5774339f753f45f603c901df324d6`.
- Runtime lineage: 2.1.5 is the reviewed bridge-v2 candidate that 2.1.4 was
  meant to be — the 2.1.4 publication was lost to GitHub's immutable-release
  tag reservation during a metadata remediation and 2.1.4 was never publicly
  released; 2.1.5 differs from that fully reviewed candidate by exactly one
  JAR entry (`fabric.mod.json`, the version string). 443 of 444 entries are
  byte-identical, so the prior real-Minecraft v2 smoke test remains applicable
  to these runtime bytes.

## Launcher changes (this activation)

1. `src-tauri/production/aurora-releases.json` — 2.1.5 stable entry added
   (Minecraft 1.21.11, Fabric Loader 0.19.5, Java 21, the exact public URL,
   SHA-256 and size above, same pinned Fabric API 0.141.6+1.21.11 coordinates
   as 2.1.3). The immutable 2.1.3 and 2.1.2 entries remain for existing pins.
2. `src-tauri/src/launch/activity_bridge.rs` — `BRIDGE_ARTIFACT_SHA256` now
   the independently verified 2.1.5 digest; the digest-gated session
   preparation in `application.rs` uses `prepare_for_protocol(2)`. Eligibility
   remains bootstrap-active + Minecraft 1.21.11 + **exact reviewed digest**:
   a wrong-hash "2.1.5", the former 2.1.3 digest, or any unknown artifact
   receives no bridge at all. 2.1.3 is thereby not silently reclassified as
   v2; clients that still speak v1 (Aurora Client 2.1.3 with older launcher
   builds) are unchanged — the v1 protocol path remains in the codebase.
3. Pin-referencing tests updated: `distribution.rs` (3 releases, 2.1.5 exact
   identity), `instance_mods.rs` / `instances/lifecycle.rs` (production
   resolution now selects 2.1.5; old pins retained).

## Verification evidence

- **Public download verification**: the published release was independently
  downloaded (not from the build tree) and re-hashed — exactly 2,468,545
  bytes, SHA-256 `fdc344e2…1df324d6`; GitHub's asset digest agrees. JAR audit:
  444 entries, no duplicates, no nested JARs, no test harness, no secrets, no
  Discord SDK, no receiver; embedded version 2.1.5, Minecraft `~1.21.11`,
  Java `>=21`, Fabric Loader `>=0.16.0`.
- **Acquisition against the live public release**: the launcher's own
  acquisition chain (production manifest → strict HTTPS `ArtifactSource` →
  streaming SHA-256 + declared-length verification → staging → cache copy
  re-verification) was exercised against the actual public artifact; a
  one-byte-substituted local JAR produces a different digest and gains no
  trust.
- **Bridge v2 smoke with the public artifact**: the launcher's explicit
  interop test (`compiled_java_v2_worker_interoperates_with_rust_receiver`)
  was run with the **publicly downloaded** `aurora-2.1.5.jar` on the
  classpath: the real shipped Java worker negotiated v2 with the real Rust
  supervisor through the real supervised-spawn boundary, transported
  singleplayer save identity (`stable-save`) and multiplayer server target
  (canonicalized to `example.invalid:25565`), and exited cleanly. A wire-level
  probe additionally captured the full frame sequence: hello(schemaVersion 2)
  → accepted → MAIN_MENU → SINGLEPLAYER(+worldSaveId) → MAIN_MENU →
  MULTIPLAYER(+serverTarget).
- **In-game evidence applicability**: the prior real-Minecraft v2 smoke test
  (menu/singleplayer/multiplayer identity, clearing — run against the
  byte-identical runtime) remains the authoritative in-game evidence; it was
  not repeated because 2.1.5's runtime is proven identical to that artifact
  except for the embedded version string.
- **Regression suite**: Rust 562 passed / 0 failed / 21 ignored; icon tests 6
  passed; frontend tests 130 passed; svelte-check 0 errors / 0 warnings;
  frontend production build passed; `cargo fmt --check` and
  `cargo check --all-targets` passed.

## Scope boundaries

- Launcher public release remains **1.1.0** — unchanged, untagged, not
  replaced. This activation lives on development `main` and will be packaged
  by a future launcher release decision.
- No launcher version bump, no launcher tag or GitHub release.
- Discord/privacy behavior is unchanged: identity stays within the launcher's
  local history/privacy architecture; the client still sends world/server
  identity only over the authenticated loopback bridge.
