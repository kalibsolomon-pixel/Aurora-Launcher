# P3-A.4a OpenGL presentation-path investigation — PARTIAL

[Field/source semantics](PRESENTMON_FIELD_SEMANTICS.md),
[frozen diagnostic protocol](P3A4A_DIAGNOSTIC_PROTOCOL.md),
[actual Minecraft matrix](evidence/p3a4a/minecraft.json),
[independent gap review](evidence/p3a4a/gap-review.json).

One authenticated isolated Minecraft child, exact Microsoft OpenJDK 21.0.7+6-LTS,
Fabric 0.19.5 / Minecraft 1.21.11, canonical restored A scene, Balanced/2048 MiB,
empty custom JVM text, 1920×1080 windowed, render/simulation 12/8. All conditions
use VSync off and Minimized inactivity limiting. No player motion. The corrected
stationary warm-up before the first trace is 300.120 seconds; debug overlay off.
Each trace requests 16 seconds, with a predeclared inner window 4–14 seconds.
No VSync-on/fullscreen/global driver/display experiment was necessary or performed.

## Capped/uncapped observations

Rates and interval maxima below use the inner 10 seconds; row counts and NA
fractions use the entire raw trace. These denominators are deliberately distinct.

| Actual installed Max Framerate option | Raw rows / unavailable display estimates | NA fraction | Observed ETW submission rows/sec | Available display estimates/sec | Max available estimate interval |
| --- | --- | --- | --- | --- | --- |
| Unlimited, maxFps=260 | 29605 / 27630 | 93.3288% | 1827.332 | 122.354 | 512.4966 ms |
| 120 FPS, maxFps=120 | 1889 / 0 | 0% | 118.198 | 118.194 | 14.0259 ms |
| 240 FPS, maxFps=240 | 3695 / 47 | 1.2720% | 231.541 | 228.398 | 10.4442 ms |

Every row has Other / 0x0 / Composed: Copy with GPU GDI, SyncInterval=-1,
PresentFlags=0 and AllowsTearing=0. All three producer exits are 0; exact retained
PID/creation identity matches, all 36 foreground samples per condition target the
game, and observed ETW session counters are zero through each inner window.
QPC frequency is 10000000 Hz both before and after each capture. Raw hashes,
all fields/distributions, process samples and original short-reader results are
retained. Full measured-run quality remains INCOMPLETE.

The near-zero missingness at capped rates and large uncapped missingness establish
condition dependence on this host. They do not establish optical delivery or prove
that the cap improves game performance. Available estimates' uncapped median is
4.1669 ms while their mean is 8.1730 ms: sparse tails change the average markedly.
Do not collapse this into a physical FPS number.

## Submission holes are an additional blocker

The uncapped submission stream itself has a **508.6165 ms maximum interval**,
with long row gaps overlapping the ten roughly half-second display-estimate gaps.
The few events inside each display gap cluster around its boundaries; they do
not establish continuous submission throughout the gap. A boundary-complete CSV
with valid monotonic timestamps can still omit long interior spans.

In the separate 1800-Hz WGL trial, independent markers establish continued swap
calls across PresentMon row gaps, including 3356 swaps during a 1873.352 ms hole.
That experiment disproves treating such ETW row gaps as swap-call stalls in the
controlled condition. Minecraft lacks an independent continuous render/call marker,
so the exact game gap cause cannot be assigned. F3 spot observations (3737 before
warm-up, 3848 after diagnostics with fresh input) differ from ETW row rate; they
are contextual instantaneous observations, not an independent synchronized series.

## Hypothesis review

| Hypothesis | Evidence and conservative disposition |
| --- | --- |
| Many presents do not become distinct displayed frames | Latest-copy/DWM association is supported by pinned source and observed composed mode; not every NA has an individually known drop reason |
| Desktop composition changes availability | All captures use the same composed path; cap changes availability within it. No isolated alternate-compositor comparison, so causal contribution is not quantified |
| WGL limits inference | Other lacks tracked runtime entry/exit instrumentation. API-time zero is not measured zero call duration. High-rate row/marker mismatch fails one-row-per-call assumptions |
| Refresh/VRR/driver behavior matters | Desktop reports 240 Hz and common available intervals approximate 4.167 ms. VRR, overrides, scheduling and thermals remain unknown; no global settings changed |
| Adapter interpretation is wrong | TimeInQPC deltas agree with MsBetweenPresents to export precision; original display reconstruction is unchanged. The rejected assumption is measurement completeness/physical meaning, not a discovered arithmetic bug |
| Replacement/queuing under uncapped submission | Most-recent-copy association supplies a mechanism for fewer display estimates. Queue depth, actual scanout/drop counts and every missing record's cause remain unmeasured |
| Observer affects high-rate behavior | Independent marker rate changes are small in these blocks, but a CPU screen fails, repetitions are insufficient and the observer's event stream is incomplete. No negligible-overhead claim |

ETW session loss counters cover session buffers/events through their query times.
They do not prove driver event coverage, internal PresentMon present tracking or
shutdown completeness. The exact source of missing high-rate records remains a
P3-A.4b investigation. CPU/GPU/display physical behavior cannot be reconstructed
reliably across an unobserved span from these columns alone.

## Preservation and decision

Only maxFps changed in the disposable options bytes. Unlimited was restored after
each cap; the complete saved options hash matched the canonical lock each time
and after normal game exit. The final camera/framebuffer still match, post-exit
scene validation finds zero non-player entities/block entities, and the frozen
archive/full inventory/restoration remain intact. Game and launcher exited 0.
[World proof](evidence/p3a4a/world.json), [preservation](evidence/p3a4a/preservation.json).

Uncapped submission and display tails are DIAGNOSTIC ONLY. There is no accepted
primary metric for uncapped JVM comparisons. Capped submission rate is useful
secondary context with lower-rate prior validation, not a new canonical treatment.
Physical display timing is unavailable; GPU attribution remains diagnostic.
See [quality decisions and P3-A.4b prerequisites](P3A4A_QUALITY_GATES.md).
P3-B remains NOT READY / NOT AUTHORIZED.
