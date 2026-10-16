# Performance P3-A — benchmark infrastructure acceptance

**Overall: PARTIAL. P3-B is NOT READY and NOT AUTHORIZED.** Offline tooling,
exact Java identity, isolated game/mod materials and native verification are
implemented and tested. Actual presentation calibration, authenticated Minecraft
capture, immutable worlds/execution validation and a quiet resource window remain
blocked. Gameplay performance is **NOT MEASURED**; no preset/tuning improvement
or Minecraft launch is claimed. Stop after P3-A.

## Repository and local commits

Branch `codex/client-3-production-authority`; starting HEAD
`514fa088ee33ea99cccc63432adac1a40e5d99b4`, matching the request. Initial tracked
tree/index clean; four protected untracked files and seven worktrees present.
Full tracked/untracked hashes/history/worktrees recorded before changes in
`.git/performance-p3a-baseline.json`; document input identities in
`.git/performance-p3a-input-documents.json`.

| Local commit | Scope |
| --- | --- |
| `802e830` | Raw pinned capture import, quality/clock/cohort gates, bounded Windows sampler, G1 parser, schemas and synthetic tests |
| `1a2d60c` | Actual isolated runtime/workload/native/ambient evidence and safe native preparation helpers; processor-group guard |
| `b292a93` | Exact telemetry-handle exit detection and actual exit-code-259 regression; final tooling count 36 |
| Commit containing this report | Audit, decisions, protocols, workload locks, readiness, user authorization and complete inventory |

Ending implementation HEAD: `b292a93`. Ending review HEAD is the local acceptance
commit containing this report. Its exact SHA is recorded after commit in
`.git/performance-p3a-final.json` and the final response; a commit cannot contain
its own content-dependent hash. No push, merge, tag, workflow, publication,
release preparation, version bump or installer execution.

Full changed-file inventory: [CHANGED_FILES.txt](docs/performance-p3a/CHANGED_FILES.txt).
**58 added paths.** All changes since the starting HEAD are additions; no original tracked file changed
or disappeared. Intermediate P3-A commits refine only files created in this phase.

## Audit, capture decision and calibration

[Measurement audit](docs/performance-p3a/MEASUREMENT_AUDIT.md) traces actual native
runtime/configuration/launch/supervision/auth boundaries and separates earlier
launcher-startup studies from JVM gameplay measurement. P1 had an offline analyzer,
not raw capture or a game baseline. P2 modes remain instance-scoped advice: Performance
equals Balanced, existing configurations retain their semantics, registry 6/config 7
are unchanged. No production source/dependency/UI/validator/Java policy changed.

[Frame decision](docs/performance-p3a/FRAME_CAPTURE_DECISION.md) selects PresentMon
2.3.1 standalone console as a pinned **candidate**, with CapFrameX and Linux MangoHud/
internal timing alternatives evaluated. Official upstream documentation and licenses
were researched. OpenGL latency/display estimates have known limitations; advertised
API support is not Aurora/Sodium validation. No third-party capture tool was installed
or executed. Bounded PATH/named-directory/process discovery found none usable.

[Calibration protocol](docs/performance-p3a/CALIBRATION_PROTOCOL.md) defines stable/
variable presentation stimuli, identity, raw completeness/loss, QPC units, start/stop,
clock offsets/drift and instrument-on/off A/A. **Zero actual presentation samples**,
zero controlled presentation calibration and zero Minecraft captures. Synthetic
import tests and a real standalone GC probe do not close these gates. Status is
machine-readable in [calibration-status.json](docs/performance-p3a/evidence/calibration-status.json).

## Java, materials and isolation

[Exact Windows Java lock](docs/performance-p3a/JAVA_RUNTIME_LOCK.md): Microsoft
OpenJDK `21.0.7+6-LTS` / Microsoft-11369940, windows-x64, java-runtime-delta,
official manifest SHA-1 `cb4394a27089d19f65d5baa6cf0482c27c3c7865`.
Actual javaw/java executable SHA-256 identities and fixed bounded diagnostics are
in [java-runtime.json](docs/performance-p3a/evidence/java-runtime.json). Native deep
validation checked 402 files / 99,041,788 bytes. Default G1 executed; selector names
and diagnostic modules/tools were observed, not proof of nondefault collector support.
No collector/Xms tuning was activated in Minecraft or product configuration.

Normal native create_instance prepared **three real isolated Fabric installations**,
not injected Ready fixtures. A is minimal Fabric, Aurora/API absent. B/C have Aurora
3.0.0, Fabric API 0.141.6+1.21.11, Sodium 0.8.14 and Lithium 0.21.4. Provider installation
used exact version IDs and fingerprint-reviewed previews. Complete content validation
returned Ready; artifacts were independently rehashed against release SHA-256 and
publisher SHA-1/SHA-512. Additional observed SHA-256 values are labeled honestly.
[Mod evidence](docs/performance-p3a/evidence/mod-environment.json).

[Workload specification](docs/performance-p3a/WORKLOAD_SPEC.md) and
[machine locks](docs/performance-p3a/workloads.json) freeze versions, proposed scene/
camera/graphics/power/warm-up/capture conditions. Stationary scenes deliberately
reduce manual route variability without automation mods. Actual world/route/options
hashes remain null and block measured manifests. No owner world or test world was
created/copied; B/C coexistence and demanding-scene intensity remain untested.
Installed content Ready is not authenticated/world/execution readiness.

Fresh marked direct-temp managed root, separate registry/config/WebView, instance
game directories and test logs. Existing verified native installers/resolvers only;
no Python Minecraft/Java installation mechanism. Four exact isolated native sessions
exited 0. Balanced/2048 MiB/empty custom text persisted through actual restart; UI
Check Java showed the validated runtime. Six actual native screenshots, CSS viewport
1120x790 and DPR 1.25, are indexed in
[NATIVE_VERIFICATION.md](docs/performance-p3a/NATIVE_VERIFICATION.md).
They are not browser fixture substitutes or new native-minimum sizing acceptance.

## Authentication and instrumentation boundaries

Zero isolated accounts; no sign-in, token/credential read/copy, owner account database
reuse, entitlement bypass, private server or Play command. The
[authorization procedure](docs/performance-p3a/AUTHORIZATION.md) identifies an important
boundary: Credential Manager targets use account UUID, not managed-data root. A
same-account login under the owner's Windows user could rotate production's credential.
Use a separately authorized Windows test user/session or a reviewed dedicated account;
do not treat a separate data root as secret-store isolation. No account namespace
change or plaintext fallback was implemented.

P2 modes reject GC logging flags. Future instrumented experiments must deliberately
use disposable Existing Configuration laboratory instances with equal reviewed logging
and explicit heaps, recording advice origin separately. No validator exception or
hidden flag injection. Generated-Xmx/Mojang/loader/logging/custom order is unchanged;
the existing native resolver test proves mode equality/order. No LaunchSpec, full
child argument array, account/game arguments or tokens were exported. This is
source/fixture proof, not a captured real Minecraft invocation.

## Metrics, quality and tests

[Artifact contract](docs/performance-p3a/ARTIFACT_FORMAT.md),
[schema](docs/performance-p3a/run.schema.json),
[tool guide](tools/benchmark/README.md). Standard-library-only Python tools:

* Experimental pinned PresentMon/QPC display-estimate import and explicit normalized
  internal/synthetic sources; no CPU-start/display conflation; dropped rows retained.
* Mean/median/P95/P99/type-7 percentiles, P99.9 only at sufficient depth, variance,
  presentation rate, >50 ms stutter frames/bursts and retained long gaps.
* Shrink-safe HotSpot G1 pauses/heap and rounded process CPU during logged phases;
  not isolated GC-worker or total GC CPU. Real bounded standalone Java log ingestion
  exposed/corrected P1's before-used/end-capacity assumption without changing P1 files.
* Explicit-PID/creation/retained-handle Windows CPU/working-set/OS-peak/system-memory
  sampling; counter-derived weighted rates, bounded output and partial-file retention.
  Unknown or >64-processor Windows groups deliberately unsupported. Heap/paging/GPU/
  thermal unknowns remain null. No owner Java process was sampled or attached.
* Two-anchor timestamp alignment/drift/uncertainty and noncausal GC/stutter proximity;
  hashes/containment/metadata/identity/units/order/completeness/warm-up/depth/cohort gates.
  VALID / VALID WITH WARNINGS / INVALID / INCOMPLETE records retain every attempt.
  Offline VALID never proves producer authenticity or real-game capture compatibility.

[Test matrix](docs/performance-p3a/TEST_MATRIX.md): **36 new tooling tests**, **13 P1
analyzer tests**, **992 Rust tests** (31 existing ignored), **40 focused frontend
tests**, no failures; type check zero errors/warnings; production frontend assets
and isolated native development build pass. Existing UI-1–UI-5 evidence is byte-
preserved, not rerun because no product source/UI changed. New native evidence is real.
Linux/native game capture and game profiling remain NOT TESTED.

Actual 30 s read-only self-process/global-system telemetry records about 6.5% mean
machine CPU and ~10.1 GiB minimum available memory, with 0.77 ms maximum observed
sample-read duration. These are ambient/plumbing observations, not game performance
or calibrated game-observer overhead. [Quiet preflight](docs/performance-p3a/QUIET_SYSTEM.md)
says **NOT READY**: existing owner Java activity, uncontrolled development load,
unknown GPU idle/thermals/AC and less than the planned 60 s quiet observation.

## Preservation, evidence and next gate

All **2,375 original tracked files and four protected untracked files** hash-identical
to baseline. Seven worktree paths/branches retained; other worktree HEADs unchanged,
only this branch advances. Critical root docs/manifests/lockfiles/native configuration,
old acceptance/UI evidence and protected developer bytecode remain intact. Scoped
P3-A raw-byte Git attributes prevent CRLF checkout from changing artifact digests.
[25-artifact inventory](docs/performance-p3a/EVIDENCE_INVENTORY.json); staged and local
raw bytes/hashes verified. Private preservation checker: .git/performance-p3a-check-preservation.py.

No production Launcher 1.4.2 installation/configuration/credentials/instances/worlds/
Java/release artifact was mutated by these actions. Owner Java/launcher processes
were left running. No process-name termination, broad cleanup, .minecraft use,
power/affinity/priority/security setting change, telemetry/upload or release work.
Ordinary writes by already-running owner software are not claimed to be frozen.

[Readiness matrix](docs/performance-p3a/READINESS_MATRIX.md) keeps blocked gates separate.
Exact next owner actions: approve a pinned frame tool and any required privileges;
choose safe test-user/account isolation; authorize short legitimate Minecraft/world/
capture validation; supply a quiet resource window; review the updated gates before
separately authorizing repeated P3-B. [P3-B protocol](docs/performance-p3a/P3B_PROTOCOL.md)
plans effective-vector equivalence, A/A noise, >=5 repetitions, rotated order,
300/600 s intervals, preserved failures and evidence/owner promotion gates.
No FPS, tails, GC, CPU or startup improvement is claimed. **P3-B was not started.**
