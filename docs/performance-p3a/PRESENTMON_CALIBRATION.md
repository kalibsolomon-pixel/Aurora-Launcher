# PresentMon P3-A.4a — high-rate fidelity PARTIAL

[Field semantics](PRESENTMON_FIELD_SEMANTICS.md), [OpenGL matrix](OPENGL_PRESENTATION_INVESTIGATION.md),
[gap evidence](evidence/p3a4a/gap-review.json), [quality decisions](P3A4A_QUALITY_GATES.md).
Three same-child stationary Minecraft traces show 93.329% unavailable display
estimates uncapped, 0% at cap 120, and 1.272% at cap 240, all on Other/Composed:
Copy with GPU GDI. Latest-copy/DWM association explains why many submissions need
not yield distinct available display estimates. Exact physical drop/delivery counts
remain unknown. The approximately 512 ms estimate gap is not a validated stutter.

The uncapped submission stream also has roughly half-second holes. At representative
WGL rates, 3356 independent swaps continued across one 1873.352 ms PresentMon row
hole. Zero observed session loss counters do not certify driver/consumer event
completeness. Both uncapped channels' tails are DIAGNOSTIC ONLY; no primary JVM
comparison metric is accepted. Independent GPU completion/scanout is unavailable.

All new producers exit 0, exact identities/focus match, QPC frequencies before/after
are 10000000 Hz, and raw hashes/long gaps are preserved. The separately named short
fidelity review reports offline consistency, null/zero, signed diagnostics and
external counter/creation evidence; it never claims physical display or full-run
quality. The original P3-A.3 actual short-reader result is exactly unchanged on
reinspection. No old raw bytes or failed inspection was replaced.

The representative-rate interference trial does not establish a defensible bound;
Minecraft observer-off/on trials were gated off. Full measured schema, thresholds,
300/600 seconds and source requirements are unchanged. P3-B NOT READY / NOT AUTHORIZED.
Historical calibration reports follow unchanged.

# PresentMon P3-A.3 — fixed cadences PASS, full calibration PARTIAL

The exact 421312-byte 2.3.1 binary and Valid Intel signature were rechecked at
both isolated starts. SHA-256 remains
`364e5d98d4d134bd54dd25c22ed2ca2f4883f8bc3ed6502bee0c151e3436d30c`.
No elevation, service, driver, privilege change or replacement tool was used.
[Actual cadence evidence](evidence/p3a3/cadences.json),
[overhead report](INSTRUMENTATION_OVERHEAD.md), [pacing diagnosis](FRAME_PACING_DIAGNOSIS.md).

| Requested cadence | Independent observed rate | ETW display-estimate rate | Inner present rows / markers | Mean / max paired interval disagreement |
| --- | --- | --- | --- | --- |
| 30 Hz | 30.00055 Hz | 30.00020 Hz | 600 / 600 | 0.06012 / 0.6763 ms |
| 60 Hz | 60.00031 Hz | 60.00427 Hz | 1200 / 1201 | 0.06866 / 0.9819 ms |
| 120 Hz | 119.99909 Hz | 119.45082 Hz | 2400 / 2400 | 0.05317 / 1.0391 ms |

Each stimulus ran 26 s with a 20 s inner window (3–23 s), >=2 s pre/post-roll,
independent pre/post-swap integer QPC markers, exact PID/creation identity and a
unique bounded trace. The stimulus gate followed capture startup. All short
inspections are VALID WITH WARNINGS. Raw rows are 779/1559/3119; unavailable
display rows are 0/0/11. The unmatched 60-Hz marker is at the window boundary;
there are no interior unmatched or duplicate-nearest markers. QPC frequency is
10000000 Hz. Present timestamps follow pre-swap markers by mean 0.225–0.282 ms,
max 1.209 ms; most are outside the CPU-call bracket. This is an observed
asynchronous boundary, not identical API/physical-display timestamps.

One phase plan now drives stimulus metadata and execution. Independent markers
bracket swaps, and fixed deadlines avoid adding drawing work to each period.
Stale P3-A.2 phase/stall metadata and original captures remain untouched. New
regressions reject stale fields, phase bounds, wrong QPC units, identity mismatch,
incomplete stimuli and interior losses. The old variable/stall specimen remains
historical evidence; no new variable/stall calibration or game-stall inference is claimed.

Read-only ControlTrace QUERY succeeded for each owned trace without privileged
changes. EventsLost, LogBuffersLost and RealTimeBuffersLost were zero before the
stimulus and after the inner window: **ZERO OBSERVED THROUGH WINDOW**, not
unconditional zero loss or shutdown-tail proof. Failed/absent queries stay UNKNOWN.
Microsoft documents the [query operation](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/nf-evntrace-controltracew)
and [counter meanings](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/ns-evntrace-event_trace_properties).
Marker reconciliation adds bounded completeness confidence. NA display fields,
boundary truncation, stream mismatch and invalid CSV remain separate classifications.
The generic short reader does not consume external counter/overhead reviews;
its conservative warnings remain alongside this additional evidence.

Alternating instrument-off/on trials passed their observed 120-Hz screens, but
two blocks/state cannot establish negligible interference or bound high-throughput
Minecraft overhead. Uncapped Minecraft's unavailable display estimates and long
estimate gaps remain unresolved for frame-tail comparisons. No full-run schema,
threshold, clock/GC requirement or measured 300/600 s gate was weakened. Overall
calibration is PARTIAL; full measured-run quality INCOMPLETE. P3-B remains
NOT READY / NOT AUTHORIZED. Historical P3-A.2 results follow.

# PresentMon P3-A.2 operational calibration — PARTIAL

Actual execution under the owner's existing unelevated Windows principal is now
validated. No privilege/group/policy change, service, driver, installer or new tool
version was used. Same-user scope: [AUTHORIZATION.md](AUTHORIZATION.md).
[Sanitized evidence](evidence/p3a2/controlled-calibration.json).

The staged 421,312-byte PresentMon 2.3.1 executable was rehashed before execution:
`364e5d98d4d134bd54dd25c22ed2ca2f4883f8bc3ed6502bee0c151e3436d30c`.
Authenticode remained Valid / Intel Corporation. Official acquisition identity is
unchanged from [P3-A.1](PRESENTMON_INSTALLATION.md). A three-second unique-session
permission check against the isolated launcher host started/stopped and exited 0.
It generated no CSV because that host did not present frames; this is permission
plumbing only, not successful presentation capture. No existing trace was stopped.

| Stage | P3-A.2 result |
| --- | --- |
| Identity and unelevated tracing/start-stop | PASS, actual execution |
| Controlled native OpenGL presentation | Actual 1,428 rows; bounded operational capture PASS WITH WARNINGS |
| Full fixed/variable/stall protocol | PARTIAL, mixed-cadence overlap only; not 20 seconds per cadence |
| Units and exact target | PASS, PID 3856 / recorded creation FILETIME, Other stream 0x0; QPC frequency 10,000,000 Hz |
| Export/parser integrity | PASS WITH WARNINGS; real default CSV and raw hashes retained |
| Independent event-loss accounting | NOT ESTABLISHED, no explicit producer loss counters |
| Observer overhead / A/A | NOT MEASURED; no repeated runs authorized |

The exclusively created native WGL stimulus ran 36 seconds, emitted 2,320 independent
pre-SwapBuffers QPC markers and exited 0. It requested 30/60/120 Hz, an alternating
phase and 100/250 ms stalls. PresentMon's bounded 20-second collection overlapped
17.109695 seconds of presentation data because the stimulus finished before its
timer. The inspected inner window was declared from retained boundaries, with
pre/post-roll; it is **13.109695 seconds**, not a claimed complete 20-second capture.
No rows were erased. There were 1,427 application markers inside the first/last
present span versus 1,428 CSV rows, consistent with boundary quantization but not
independent loss proof. Nearest marker discrepancy: mean 0.212443 ms, max 1.7794 ms.
The helper's initial plannedStallsSeconds metadata was stale; the private review
explicitly records actual code timings 29/31 seconds and preserves original metadata.

The actual default producer CSV uses `TimeInQPC`, `MsUntilDisplayed` and
`MsBetweenDisplayChange`. The old adapter expected v2 columns and did not fit this
export. The separate [short artifact reader](SHORT_CALIBRATION_FORMAT.md) uses
present-QPC plus latency in milliseconds; it never substitutes CPU-start timestamps.
It counted all 424 unavailable display rows and kept their uncertainty. These rows
are not automatically missing CSV events or physical-display drops. Strict failures
cover identity, stream, units/order, missing/invalid columns/rows, bounds and hashes.
The actual 180-row prefix regression specimen is byte-identical to the raw prefix.

Direct Windows stdout/stderr redirection produced UTF-16LE with BOM during the
Minecraft capture; the first inspection failed decoding. That failed result remains
private. Strict BOM-aware decoding and a regression now support the observed
encoding; the **same raw capture** passes reinspection. No second game capture was
made and no measured-run gate was weakened. Final tests: 52 benchmark / 13 P1 pass.

The tool emits a generic unelevated process-query warning. Exact PID and live creation
identity were verified independently; no process-name targeting or elevation fallback
was used. No error/loss warning was observed, but absence of warnings does not prove
loss-free capture. OpenGL Other/composed timing remains an ETW display estimate,
not optical display measurement. Observer overhead, thermals and VRR remain unknown.

Actual Minecraft capture is now available, separately reported in
[MINECRAFT_CAPTURE_VALIDATION.md](MINECRAFT_CAPTURE_VALIDATION.md). Near-30-Hz pacing
and world-condition drift keep overall P3-A.2 PARTIAL. Full measured-run classification
remains INCOMPLETE, and P3-B is NOT READY / NOT AUTHORIZED.
