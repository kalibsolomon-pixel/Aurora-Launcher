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
