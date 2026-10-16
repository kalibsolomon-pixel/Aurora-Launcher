# Short calibration artifact v1

`tools/benchmark/short_calibration.py` inspects real bounded presentation exports.
It does not authenticate, capture, launch, change settings or produce a benchmark
winner. Measured schema 1 and its 300/600-second/three-source gates are unchanged.

The directory contains `short-calibration.json`, `frames.csv` and
`presentmon-stderr.txt`. Manifest keys are exact; duplicates/unknown keys fail.
Required fields:

| Field | Contract |
| --- | --- |
| schemaVersion / artifactKind | integer 1 / `short-calibration` |
| targetKind | `controlled-opengl` or `minecraft`; provenance reviewed separately |
| status | `complete`, `interrupted`, `failed` |
| startUtc / endUtc | zoned ordered timestamps; at most 90 seconds apart |
| pid / creationId | positive integer PID / decimal Windows creation FILETIME |
| executableSha256 / toolSha256 | non-placeholder lowercase SHA-256; exact staged PresentMon digest |
| format | `presentmon-2.3.1-default-present-qpc` only |
| qpcFrequencyHz / originQpc | positive integer frequency / integer origin, recorded from live Windows QPC |
| swapChain / presentRuntime | exact hexadecimal stream identity / `Other` |
| windowStartSeconds / windowSeconds | nonnegative offset / 1â€“30-second inner window |
| framesSha256 / diagnosticsSha256 | exact raw file hashes |

Default PresentMon 2.3.1 CSV actually emits `TimeInQPC`, `MsUntilDisplayed` and
`MsBetweenDisplayChange`. The historical P3-A adapter expects explicit-v2 columns;
it deliberately rejects this default export. This separate reader does not relabel
the default CSV or change the existing adapter. Pinned producer code establishes
the present QPC and display-latency units:
[CsvOutput.cpp](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/PresentMon/CsvOutput.cpp),
[OutputThread.cpp](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/PresentMon/OutputThread.cpp).

Display estimates use `(TimeInQPC-originQpc)/frequency + MsUntilDisplayed/1000`.
CPUStartQPC is not substituted for the present timestamp. NA display latency rows
are counted and retained; they are unavailable display estimates, not proof of
lost ETW events. Invalid/nonfinite/zero/negative display latency, mixed PID/stream/
runtime, noninteger/duplicate/regressing QPC, missing/duplicate columns, malformed
rows, hash drift and linked ancestors fail. At least ten complete intervals and
250 ms boundary coverage are required. Interruption or inadequate coverage remains
INCOMPLETE; producer error/loss diagnostics require review and fail this validator.

VALID WITH WARNINGS proves bounded offline consistency, not physical screen timing,
producer authenticity, independent loss accounting, Minecraft scene provenance or
observer overhead. Those require live operator review. Full measured-run quality
always remains INCOMPLETE for this format. No short artifact is labeled synthetic.
Redirected producer diagnostics support strict UTF-8 (optional BOM) and observed
Windows UTF-16LE with BOM; malformed encoding fails. The 180-row regression specimen is an actual controlled native OpenGL export;
malformed variants in tests are explicitly test-only transformations.

```powershell
python -B tools/benchmark/short_calibration.py <test-owned-directory>/short-calibration.json
```

Exit 0: VALID WITH WARNINGS. Exit 2: INVALID/INCOMPLETE. Failed artifacts are retained.
Raw game/account/world material stays private; committed summaries retain reviewed
hashes and limitations without account identifiers, private paths or child arguments.
