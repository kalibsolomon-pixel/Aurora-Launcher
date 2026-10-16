# Frame capture decision — candidate selected, validation BLOCKED

Research date: 2026-10-16. Primary candidate: **PresentMon standalone console
2.3.1**, pinned deliberately for an explicit CSV contract, not advertised as the
latest version. No capture binary was downloaded, installed, executed or bundled.
PATH, three named installation directories and relevant process names were checked;
none identified a usable PresentMon/CapFrameX installation. This is bounded
discovery, not a claim that no copy exists anywhere on disk.

| Candidate | Assessment | Decision |
| --- | --- | --- |
| PresentMon console | PID-targeted ETW/QPC export; Windows OpenGL/Vulkan coverage is advertised, but those APIs have weaker instrumentation and latency-derived metrics can be less accurate. Aurora/Sodium compatibility and overhead remain untested | Primary **candidate**; actual capture BLOCKED |
| CapFrameX | Adds capture/analysis/monitoring around PresentMon; not an independent cure for OpenGL timing limitations. Project MIT license plus separately licensed bundled components need review | Windows fallback if console workflow fails and owner approves it |
| MangoHud | Linux OpenGL/Vulkan overlay and local logs; overlay/interception timing is not automatically displayed timing. Logging cadence must be pinned and verified as per-frame before tail analysis | Linux alternative, NOT TESTED |
| In-game recorder / profiler | Could export internal render timing with documented instrumentation overhead. No compatible recorder is implemented/approved here; JFR alone has no universal display-frame stream | Semantic fallback only; cannot satisfy display gate by relabeling samples |

Sources: [pinned PresentMon overview and limitations](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/README.md),
[pinned console contract](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/README-ConsoleApplication.md),
[CapFrameX](https://github.com/CXWorld/CapFrameX),
[CapFrameX license/components](https://github.com/CXWorld/CapFrameX/blob/master/LICENSE),
[MangoHud](https://github.com/flightlessmango/MangoHud),
[MangoHud license](https://github.com/flightlessmango/MangoHud/blob/master/LICENSE).
PresentMon/MangoHud use MIT-style licensing with notice obligations. No redistribution
is performed. Pin the approved binary hash and review its third-party notices before
any future redistribution; authorization here does not grant installation approval.

Proposed capture after approval: pinned default-format CSV, `--qpc_time`, explicit
`--process_id`, unique run-specific session name, explicit output, bounded timed
capture and normal termination. Do not use process-name targeting, dropped-frame
exclusion, no-display tracking, stop-existing-session or automatic elevation.
ETW access may require Performance Log Users membership or separately approved
elevation. Group membership/system settings were not changed.

The experimental importer requires the pinned default columns, one PID and one
explicit swap chain. It subtracts integer QPC origins before float conversion,
then estimates display time from CPU start plus DisplayLatency. Rows without a
display retain their raw count but are excluded from displayed-interval statistics.
CPU-start deltas alone are never called display intervals. The chosen estimate is
explicitly labeled `etw-display-estimate`; its OpenGL bias, compositor semantics,
VRR/tearing, multi-stream behavior and final-frame handling require real calibration.
The importer rejects unknown versions/formats rather than guessing new columns.

Future Vulkan rendering needs an independent workload/capture validation. An API
listed in tool documentation does not establish Aurora, Sodium or Vulkan compatibility.
No overhead estimate is claimed before an instrument-on/off A/A series.

Follow [CALIBRATION_PROTOCOL.md](CALIBRATION_PROTOCOL.md). A method becomes
Minecraft-validated only after actual legitimate Minecraft samples pass the protocol
and owner review. Parser tests, browser screenshots and standalone GC logs cannot
close that gate. Neither synthetic presentation nor actual Minecraft capture occurred.
