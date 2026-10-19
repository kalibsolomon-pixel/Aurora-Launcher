# Aurora Launcher — Native Resolution and Readiness Reliability

2026-10-19. Both authorized tasks are complete. The resolution defect and a
reproduced read-only validation race are corrected. No Client source, JVM policy,
release manifest, dependency or persisted schema changes were needed.

## Resolution defect

The saved optional `WindowConfiguration` already validates and persists width
and height, and `LaunchOptions` correctly enables `has_custom_resolution`.
Mojang's conditional argument group therefore participates, but the shared
`PlaceholderValues`/substitution match lacked both `resolution_width` and
`resolution_height`. Resolution stopped before spawning. The previous custom-window
test used a plan without that group and could not detect the defect.

The existing authoritative resolver now supplies decimal dimensions through its
single substitution mechanism. It reuses configuration's **100–7680 inclusive**
bounds at the launch boundary, including native callers that construct options
directly. Missing dimensions requested by active metadata fail closed. `window:
null` still disables the conditional group and adds no resolution arguments.
Unknown placeholders, structured process spawning, token redaction, native path
validation and final locked readiness checks remain intact.

Regression coverage normalizes the existing SHA-1-verified official Minecraft
1.21.11 document through the real rule/planning path. It covers both placeholders,
explicit dimensions, bounds, absent dimensions, inconsistent feature/options,
unrelated game arguments, heap ordering, sensitive arguments and JSON round trips.
Partial, negative, oversized and out-of-range persisted dimensions are rejected.
The original fails the new metadata regression with unresolved
`${resolution_width}`; the corrected focused suite passes **18 tests** (two
existing live-metadata tests remain ignored).

Native acceptance used a newly copied disposable installed instance: Windows 11,
Minecraft 1.21.11, Fabric Loader 0.19.5, managed Microsoft Java 21.0.7+6 and the
existing local Aurora 3.0.0 jar. Saved fullscreen and override dimensions were
disabled **only in the copy** so they could not mask the command-line test.

| Configuration | Observed numeric arguments | Independently observed framebuffer | Menu / normal exit / Ready |
| --- | --- | --- | --- |
| Explicit 1280×720 | `--width 1280 --height 720` | **1280×720** | Passed / 0 / Passed |
| Explicit 1024×576, corrected validation build | `--width 1024 --height 576` | **1024×576** | Passed / 0 / Passed |
| Unspecified window, subsequent relaunch | No width or height arguments | **854×480** | Passed / 0 / Passed |

Argument observations retain only numeric dimensions, unresolved-resolution and
2048-MiB heap checks for the exact child identity; the full sensitive command line
is never emitted or saved. Actual dimensions come separately from Minecraft's
runtime window/framebuffer API at the renderable main menu, using the existing
opt-in research observer jar. No dimensions are inferred from arguments alone.
All three games exit normally without opening a world.

## Needs Attention investigation

Two fresh healthy native reference processes, normal startup/update discovery,
and repeated minimize/restore focus refreshes form the bounded reproduction.
Both reference boots record **one genuine validation-lock contention**. The native
UI also shows the failed-content-validation blocker before later recovery.
The reference already contains the resolution fix; this comparison isolates
validation locking, not a startup performance change.

Root cause: runtime/readiness can validate the same installed tree concurrently.
`read_game_target` acquired an **exclusive** `.game-install.lock` even though its
two production callers only read and verify bytes. Another legitimate reader
therefore returned `AlreadyInProgress`; readiness mapped that error to Damaged
and displayed the content-validation failure. No corruption was necessary.
The lifecycle regression fails the original with this exact error on a valid
installed fixture.

Only these byte-verification readers now use an explicit shared kernel lock.
They still perform complete validation. Install, recovery, final launch and
lifecycle ownership remain exclusive; notably `TargetLock::existing` stays
exclusive because its lifecycle caller can replay recovery. Containment,
reparse/link refusal, noncreation for absent read targets and descriptor lifetime
rules are unchanged. No cached Ready decision, retry timer, integrity bypass or
frontend freshness relaxation was added. Winning-reload startup synchronization
and stale account/instance/request guards are preserved.

Tests prove reader coexistence, writer exclusion until the last reader releases,
reader exclusion while a writer owns the target, independent-process shared
ownership/release after exact-child death, valid lifecycle checks and rejection
of changed game bytes. The corrected instrumented native captures have **zero
contention events**. A small numeric `ReadinessContention` event remains in the
existing opt-in research vocabulary; ordinary builds compile it away.

A separate, explicitly labeled negative control changes one byte of the copied
asset index. The corrected native app shows **Needs Attention and disables
launch**. Restoring the retained exact bytes and clicking Check again returns
Ready. This control verifies real failures remain visible; it is not presented
as reproduction of the original intermittent observation.

The ordinary native build, without research frontend mode or Rust feature,
reaches Ready automatically and remains Ready after two additional minimize/
restore cycles. It emits no research trace. Native selection was exercised with
one restored saved instance; asynchronous selection/account cases remain covered
by the full frontend tests.

The historical Initiative II `n3` trace did not retain its underlying error, so
its exact later trigger cannot be proven retrospectively. A concrete production
mechanism with the same false-damage behavior is now reproduced and fixed; this
does not imply every future Needs Attention state is erroneous. Legitimate
exclusive operations, damaged content, runtime/account failures and metadata
failures can still block launch.

## Validation

| Check | Result |
| --- | --- |
| Rust tests, including real Windows kernel/process checks | **1004 passed**, 31 existing ignored |
| Frontend tests, including winning reload, stale results and failed focus checks | **284 passed** |
| Frontend type/style check | **0 errors, 0 warnings** |
| Rust format and all-target check | Passed |
| Research frontend/native build and ordinary production-asset frontend/native build | Passed |
| Native startup, saved selection, two custom sizes, default relaunch, shutdown and Ready recovery | Passed |
| Native corrupted-content control followed by exact restoration/revalidation | Passed |

Native binaries are unoptimized Windows desktop builds with embedded assets and
Tauri's custom protocol. Ordinary-build acceptance is separate from opt-in
diagnostic captures. Existing native warnings, including STATIC_VCRUNTIME
deprecation, remain unrelated. G1/2048 MiB, managed Java selection, Client Frame
Pacer and Reflex policies are unchanged. Linux source compatibility is retained,
but no Linux runtime acceptance or signed release acceptance is claimed. No new
performance/latency work was undertaken.

## Evidence and reproduction

Sanitized results, binary identities and **24 raw-artifact hashes**:
[results.json](docs/native-resolution-readiness/results.json).
Private raw root: OS Temp `aurora-p0-2-rr`; build/test logs, baseline and original
fixture inventory: `.git/native-readiness-reliability`. Raw accounts, arguments,
browser state and logs are excluded from commits.

For repeat validation, use a **new marked disposable root**, not the retained
captures. Reuse `tools/benchmark/initiative2_startup.py` and the existing exact-child
observer; `native_resolution_observation.py` exports only reviewed numeric fields.
An ordinary build requires explicit `--without-research-probes`, which asserts
that no research trace was emitted. The menu-only observer must use a new output
folder each time. Inspect and hash the copied source first; never run against
the original managed instance. Full regression commands are:

```text
npm run check
node --test --test-concurrency=1 "src/lib/**/*.test.ts"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets --features performance-p0-2
cargo test --manifest-path src-tauri/Cargo.toml --features performance-p0-2
npm run build
cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bin aurora-launcher
```

## Repository state and release readiness

Starting branch/HEAD: `codex/performance-initiative-ii`,
`a083740eba2b48acaa484c590a5ae26a5ea513c9`, tracked-clean. Work is on
**`codex/native-resolution-readiness`**. Local implementation commits:

- **`e30c3b5`** — resolve validated native custom window dimensions; resolver,
  configuration tests and narrowly scoped native validation helpers.
- **`25ae6b2`** — concurrent read-only installation validation; shared-lock/lifecycle
  tests and opt-in contention diagnosis.

The documentation/results commit completes delivery; its final HEAD is recorded
in the delivery response and private `delivery-receipt.json`. Final audits inspect
all commit deletion lists and the original **2634 files / 731,752,097 bytes**.
No original file is missing and no pre-existing untracked developer file/cache is
changed. The source fixture's **10,288 files / 1,565,494,231 bytes** remain
hash-identical. All **170 saved-world files** in the disposable copy also match
their pretest hashes. Remotes are unchanged; no push, merge, publication or
official manifest mutation occurred. All validation apps exit normally.

The changes are suitable for owner code review and eventual integration, with
Windows native evidence and the stated platform limits. No temporary null-window
workaround is required for explicit resolutions. Original profiles/worlds and
previous benchmark reports remain intact; Aurora Client's tracked tree remains
unchanged at `fdc1ba5`.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**.
