# Aurora Launcher — NATIVE-8 continuation acceptance

**Classification: PASS — bounded Windows continuation; native release parity remains limited as documented.**

Date: 2026-10-31. Branch: `codex/native-ui-migration`.
Takeover HEAD: `117dab7ed7e5f317ccf3546611a257a718001cb4`.

This continues Claude's work; it does not restart NATIVE-8. The original complete
brief was not retained as a repository document. The owner's continuation brief,
commit history, working changes, scripts and evidence define this acceptance.
The NATIVE-6/7 reports remain historical, including their incomplete-parity and
release-authority classifications. This is local development acceptance, not
owner visual sign-off or permission to ship a native installer/update channel.

## Recovery and preservation

At takeover there were 39 modified tracked paths, no staged changes, and the
native UX modules/assets plus `native7-refinement/` and `native8/` evidence were
untracked. HEAD already implements shared concurrent/retried downloads and pack
corrections. The remaining native UX was implemented in the working tree.
There was no running native Launcher/compiler found in the initial process scan.
The expected worktree and branch were retained. No reset, clean, deletion of
pre-existing files, source-tree replacement, or push was performed.

Before editing, status, HEAD/history, all 3,066 tracked paths, worktrees, a binary
working patch, staged patch, hashes of modified/untracked files and critical-file
presence were recorded under `%TEMP%/aurora-native8-continuation-20261031/`.
Those private preservation records contain local paths and are not public-source
evidence. Existing Claude scripts/results remain in `%TEMP%/claude/<session>/scratchpad/`
(`bench*.ps1`, `scenario*.ps1`, `trace.ps1`, `setbg.py`, `bench/`, `scenario/`).
The earlier disposable installs remain under `%TEMP%/aurora-native8-20261031/`.
No accounts were copied, signed in/out, removed, or mutated during continuation.
The owner's media screenshot remains excluded; the media widget is disabled in
retained captures. Benchmark configuration edits restore the original bytes.

## Completed work retained

| Area | Implementation / existing evidence |
| --- | --- |
| Pack install investigation | HEAD fixes safe skipped overrides, exact-pack dependency recognition and lenient C2ME Mixin arrays; unsafe entries still fail. Real results in `install-acceptance-results.json`. |
| Shared transfers | Digest single-flight; 24 process-wide / 16 per-host permits; bounded transient retry; ordered concurrent acquisition; failed windows settle in-flight work. |
| Shared stores and Java | Verified asset hard links with copy fallback; ordinary libraries/client copied; shared runtime callers wait and reuse. Hash checks and transactional activation remain authoritative. |
| Native navigation/cards/icons | Icon rail, tooltips/accessibility names, instance grid, Mods first / Settings last, name/icon identity dialog, searchable Material Symbols paths and existing icon persistence service. |
| Home/widgets | Selected instance, Rust-owned readiness, configurable playtime, servers, worlds, instance details, optional event-driven Windows media. Existing unknown widget preferences round-trip. |
| Settings | Simple/Borealis background, three palettes including OLED, custom accent picker with live Rust palette preview and validation, Reduced Motion, Discord preferences. |
| Native chrome/identity | Frameless Windows drag/resize/maximize/minimize/close, native work-area behavior, confirmed Aurora mark and regenerated executable/window icons. Other platforms retain native chrome. |
| Skins/capes | Retained bounded worker rendering and cape correction; corrected horizontal drag, unchanged vertical bounds, no idle preview loop. See refinement report. |
| Activity | Independent instance installs can run concurrently; same-instance tasks queue; queued cancellation and failed-task retry are explicit. Running content/pack installs expose no cancellation API, and no false Cancel button is offered. |

No completed subsystem was reimplemented. Continuation source changes are confined to
Borealis timing/window state, opt-in numeric startup telemetry, repeatable
acceptance tooling, a fixed two-line card subtitle/minimum card height, and
accurate Reduced Motion copy. The pre-existing native implementation is included intact
apart from those justified corrections and Rust formatting.

## Borealis lifecycle and visual acceptance

The existing 960×600 pre-blurred contour PNG is composed below translucent tonal
surfaces. No video, live blur, procedural image regeneration, extra graphics
backend, or per-frame allocation system was introduced. One retained Slint timer
updates the layer every 500 ms; amplitude is 12 logical pixels on a 90-second
cycle. The maximum phase step moves about 0.42 px horizontally and 0.17 px
vertically. The phase freezes on suspension and delayed callbacks advance at
most one tick. Reconciliation does not reset/recreate a running timer.

The final Windows adapter refreshes identity from the actual winit window,
validates HWND existence/PID, and restyles only a new handle. Destruction clears
it. `IsWindowVisible`, `IsIconic`, foreground identity, DWM cloaking and reported
occlusion are separate checks. Focus/resize/occlusion/redraw reconciliation runs
after event propagation to avoid acting on pre-handler state. The callback also
checks authoritative state before advancing, bounding unreported suspension to
one tick. There is no background state-polling loop while paused.

Ordinary policy requires foreground + visible + not minimized/cloaked/occluded +
Borealis + motion enabled. Forced active bypasses only foreground, is compiled
only with `disposable-data-root`, and additionally requires an absolute diagnostic
data root. It is not a Settings control. Hidden/minimized/reduced-motion states
cannot be bypassed. Arbitrary overlapping-window occlusion is not reported by
winit on Windows; losing foreground still pauses normally. Other-platform
occlusion behavior and actual native window recreation were not exercised here.

**Pass on the optimized final executable.** All 33 intervals (11 states × three
runs) pass `validate-native8.ps1`. Each active/restored measurement has 18 ticks
in its nine-second interior window; mean observed tick spacing per interval is
500.06–500.59 ms. Every minimized, unfocused, Reduced Motion and forced-hidden
measurement has zero ticks. Resume events retain the exact phase/update count
from the previous pause; no catch-up burst or duplicate cadence is observed.

Lossless captures ten seconds apart change 56,824 of 94,400 contour-region
pixels (60.2%); mean absolute RGB-channel change is only 0.638 on the 0–255 scale,
and maximum summed RGB change is 9. This establishes subtle actual motion,
independently of timer messages. Two paused captures three seconds apart are
byte-equivalent in that same decoded pixel region (zero changed pixels). The
region excludes controls, labels and cursor; its physical coordinates are
x=150..1330, y=850..930 on the 1400×950 captures. Composition remains coherent.

Evidence: [validated results](native8/final/matrix/validated.json),
[active before](native8/final/matrix/borealis-home.png),
[active after](native8/final/matrix/borealis-active-after.png),
and the paired `borealis-paused-*.png` files. Forced-active intervals establish
lifecycle guards, not physical visibility of a window covered by other apps.

## Resource methodology and rejected observations

`native/tools/native8-benchmark.ps1` launches the exact optimized acceptance
executable with an explicit disposable root. It enumerates unowned visible
top-level windows belonging to the spawned PID, requires a real application-sized
window and rejects ambiguity. Every sample rechecks HWND ownership, creation
identity, visibility/minimized/foreground state. It rejects unexpected child
processes rather than silently omitting their memory. It samples working set and
private committed bytes every 250 ms, and CPU from process CPU-time delta over
the complete interval. Percent of one logical processor and percent of the whole
machine are both retained with the logical processor count. PNGs use PrintWindow;
numeric diagnostics prove timer activity independently of capture.

`windowCreatedVisibleMs` is process-start request → enumerated visible window,
not first painted frame, usable Home, or settled initialization. Native
`home-state-loaded` telemetry denotes registry/UI-state application only; it
precedes widget/readiness completion. Screenshots verify actual populated Home.
No physical presentation latency or application-settling claim is made. GPU
memory/utilization were not reliably measured by this final probe.

Claude's early `MainWindowHandle` scenarios are not accepted as window-state
proof. Later EnumWindows traces improve identity but do not independently prove
animation pixels moved. Borealis v1/v2/v3 results represent intermediate code.
The approximately 9 MiB / 20 ms figures are excluded: readiness and state are
not established, and the disposable config had a BOM problem during this work.
The config now parses as UTF-8 without a BOM; the harness writes UTF-8 explicitly.

Historical NATIVE-6 native 106.8 MiB working set / 85.7 MiB private / 874 ms
CPU quiescence and Tauri 407.2 / 213.0 MiB / 1615 ms remain historical empty-root
measurements. They are not directly comparable with a populated Home, different
code, first-window timing or the final sample windows. No dramatic footprint or
startup speedup is claimed from these mismatched endpoints.

Final binary SHA-256:
- Acceptance (`--release --features disposable-data-root`, 29,176,832 bytes):
  `5135beaeede28fd81689855aca90c0f56d97c0604b2ef90642ffd4afdc70d0b1`.
- Ordinary distribution-feature release (29,146,624 bytes):
  `a04babf70e110ded0b2587cbddf6c63c53d3a304cc0711eeea6cb6a4859750f8`.
  It builds successfully and contains none of the diagnostic root/force/trace/
  preview-probe flag strings. Actual boot/visual sampling uses the isolated
  acceptance build; the ordinary build is not booted against owner data.

Three approximately ten-second intervals per row; CPU is the observed range in
**percent of one logical core**. Memory is the mean of the three interval means.
This Windows host has **28 logical processors**. Divide CPU values by 28 for
whole-machine percentage. For example ordinary active Borealis is approximately
0.028–0.111% of machine CPU, not 0.78–3.11% of the entire machine.

| Scenario | CPU % one core, range | Working set MiB | Private committed MiB |
| --- | ---: | ---: | ---: |
| Simple idle, foreground | 0.00–0.00 | 40.3 | 13.4 |
| OLED idle, foreground | 0.00–0.16 | 41.0 | 14.1 |
| Borealis active, foreground | 0.78–3.11 | 43.5 | 16.7 |
| Borealis unfocused, visible/restored | 0.00–0.00 | 43.4 | 16.5 |
| Borealis minimized | 0.00–0.00 | 38.3 | 11.4 |
| Borealis restored and foreground | 0.31–2.80 | 43.4 | 16.5 |
| Borealis Reduced Motion, foreground | 0.00–0.31 | 43.0 | 16.0 |
| Diagnostic forced active, unfocused | 0.00–1.86 | 42.6 | 15.8 |
| Diagnostic forced minimized | 0.00–0.00 | 37.6 | 10.6 |
| Diagnostic forced restored | 0.16–1.71 | 42.6 | 15.8 |
| Diagnostic forced hidden | 0.00–0.00 | 42.6 | 15.8 |

A zero CPU-time delta is an observation at Windows counter granularity over a
bounded interval, not proof of literal zero instructions. One 15.625 ms quantum
is approximately 0.156% of a core over ten seconds. Small static-theme differences
and the active range are not grounds for an exhaustive tuning sweep. In
particular a zero forced-active CPU delta is not used to claim zero-cost visible
animation; ordinary foreground pixels and telemetry provide that proof.

The final probe also rejects two harness assumptions discovered during smoke
checks: a background `SetForegroundWindow` may be refused, and show-without-
activation after minimization does not reliably establish visible/unfocused
state. The final procedure uses native-helper activation of the exact requested
HWND and a neutral test-owned foreground fixture. It never activates ChatGPT,
private media or an unrelated user window. All configuration bytes are restored.

Enumerated visible-window time is 43.5–68.9 ms; **this is not application startup
settling**. Home-state application occurs 1,012–1,095 ms after native process-entry
telemetry, excluding loader time and preceding readiness/widget completion. The
probe waits six seconds after foreground activation before the first sample.
No first-painted-frame, physical presentation, settled-startup, GPU-allocation or
historical-memory improvement claim is made. Process identity, one-process idle
accounting, populated Home, and correct states are retained in the raw matrix.

Reproduction, helper semantics and resource-accounting limits are in
[the evidence README](native8/final/README.md).

## Install/download evidence

The retained results are sequential development observations, including failures;
failed entries are not promoted to passes. Transfer counters cover artifact
acquisition, not all HTTPS metadata/TLS traffic. They do not measure cache-copy
bytes, disk materialization, extraction, hash time, or individual network phases.
A zero artifact-byte counter is cache reuse, not faster network throughput.

| Observation | Wall time / artifact bytes | Interpretation |
| --- | --- | --- |
| Vulkan Optimized 1.21.11 initial acquisition | 42.76 s / 558,005,122 bytes; 4,677 transfers | FAILED final validation on C2ME before the metadata correction. Useful acquisition observation only. |
| Same pack, subsequent new instance | 13.42 s / 0 bytes | Successful warm-cache install; separate validation 1.42 s, ready/0 problems. Not a successful cold timing. |
| Second merge target creation + pack merge | 9.19 s + 4.99 s / 0 bytes | Shared game/artifact reuse; 25 mods + 35 overrides, subsequent review adds nothing. |
| Adrenaline, warm shared game cache | 19.62 s / 34,003,684 bytes; 24 transfers | Successful different pack; not fully cold and not a throughput comparison. |
| Two Vanilla 1.21.10 instances | 21.54 s combined / 98,304,258 bytes; 59 transfers + 59 deduplicated waits | Both succeed. Per-caller counter intervals overlap: never sum those bytes twice. |
| Later pair, already warm | 16.74 s combined / 0 bytes | Both succeed with shared game cache. Earlier Fabric attempts were deliberately rejected for ASM plan conflicts. |
| Shared Java | Earlier concurrent cold attempt had one busy error; corrected retry has two ready callers / 0 bytes / one deduplicated wait | Latest code waits; earlier failure is retained. Five subsequent instances return Java 21 ready in 1.43–2.34 s with no transfers. |
| Controlled Java fixture | cold 1,013 ms / 130 requests / max 16; warm 8 ms / 0 requests | Exactly 130 tiny synthetic files, fixed artificial 18 ms server delay per request, loopback only. Coordination/reuse regression, not internet throughput. |

A final loopback game fixture, after build/sampling work finished, records:

| Synthetic game scenario | Total ms | Requests / body bytes | Stage entries in elapsed ms |
| --- | ---: | --- | --- |
| Cold | 1,816 | 198 / 26,294 | asset-index 4; client 31; libraries 101; assets 523; materialize 1,374; natives 1,433; validate 1,435; commit 1,534 |
| Warm reinstall, same instance | 735 | 0 / 0 | initial old-tree validation 16; materialize 169; natives 226; validate 227; commit 325 |
| Second instance, shared cache | 500 | 0 / 0 | materialize 43; natives 102; validate 104; commit 204 |

Separate completed-tree validation takes 77/73/75 ms. Stage numbers are transition
timestamps, not independently isolated hash/network/copy durations. Hash checks
remain inside acquisition and validation; copied/linked byte totals are not
metered. Raw outputs are retained in `final/checks/game-controlled-final.txt`
and `runtime-controlled-final.txt`. Both fixtures intentionally inject the same
18 ms response delay; the earlier build-loaded observations remain private and
are not the final timings. No serial-versus-concurrent internet speedup is claimed.

The continuation reruns the controlled fixtures and existing deterministic tests
for distinct concurrent acquisitions, digest single-flight/owner-drop release,
transient retries, integrity failures, cancellation/rollback, same-instance
exclusion, shared asset reuse and interrupted-install recovery. It does not rerun
an expensive public-network cold install solely to obtain a more attractive number.
Cancellation releases a cancelled single-flight owner so remaining consumers can
acquire/reuse safely; it does not claim preservation of a partially downloaded
network response or UI cancellation of running pack installs.

## Verification and limits

- Native tests: 31 main tests plus 12 appearance, 4 install-driver DTO and 4 provider-driver DTO tests pass on the final source.
- Shared core: 1,020 pass, 31 explicitly ignored, zero failed. Includes download/cache/install/runtime/recovery, managed-path and authentication invariants. Default Tauri all-target type check passes (four pre-existing test-only warnings).
- Frontend: 301 tests pass; Svelte check has zero errors/warnings; static production build passes.
- Rust format: native and shared-core checks pass. Both optimized acceptance and ordinary release builds pass, as does the ordinary-feature native all-target type check.
- Real optimized Windows UI: Home and account-required Play state; instance grid and Mods-first workspace; persisted Swords identity; icon search; custom hue drag changes the live palette and Cancel restores it; Reduced Motion stops/resumes telemetry; Discord settings honestly disclose missing application registration; live Modrinth modpack browsing/details; signed-out account panel closes with Escape and returns focus; neutral player drag and Left/Right skin/cape tabs; widget customization and media add/remove; Downloads empty state; maximize/restore/close.
- Actual pointer restore works. UIA geometry was stale immediately after maximize, so that action was re-observed and verified by pointer rather than reported as a product defect. One launch with inherited short-lived diagnostic stderr exited before inspection; retained UI runs redirect stderr to a live file, and the benchmark owns/drains its pipe until orderly exit.
- Media was added/removed with screenshot capture disabled. Playback controls were not exposed in that observed viewport, so live transport buttons are not claimed tested. Existing source-label/layout tests pass; no track metadata is published and playback was not changed.
- A real card status overflow at the narrow responsive column width was found and corrected. Slint rejected the first attempted height/min-height combination; the corrected equal minimum/maximum subtitle budget is compiled and visually rechecked at both 1120×760 and 720×520 logical pixels. No feature redesign was performed.

UI flow captures in `docs/native-ui/native8/final/ui-*.png` were taken from
optimized acceptance binary `b7e6b3f2c8cd272a38d254c53292bbbbc01c69c54450b1eeb0e12441b4bf0713`
before the final card-fit and Reduced Motion wording correction; the final
Borealis renderer/window mechanism is identical. The corrected card and Settings
captures and the resource matrix use the final executable identified above.
PrintWindow PNGs are physical pixels; this host uses 125% desktop scaling.

Remaining limits: running content/pack cancellation and optional pack-file choice
are not implemented by this phase; native updater/installer and non-Windows
runtime acceptance remain separately gated. Live account switching, profile
upload/equip and credential removal are intentionally unexercised; synthetic
skin/cape and core service tests cover their retained contracts. Real system
media is private and is not retained as screenshot evidence. No exhaustive
accessibility certification, Narrator, high-contrast or 200% scaling claim is made.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
