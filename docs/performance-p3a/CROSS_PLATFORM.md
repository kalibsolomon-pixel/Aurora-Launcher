# Cross-platform limits

Shared JSON/CSV uses explicit clocks, units, semantic kinds, digest algorithms,
nullable unknowns and portable relative paths. Python offline parsing is designed
for Windows/Linux, but this session executed **Windows only**. Do not call that
Linux validation. The process-capture adapter deliberately refuses non-Windows.

Windows measurements use QPC/monotonic time, process kernel+user cumulative CPU,
working set and OS lifetime peak. Working set is not Java heap or private committed
memory. Global system CPU uses busy/total cumulative counters. ETW privileges and
OpenGL instrumentation are separate frame-capture concerns.
The Windows sampler refuses unknown or >64 logical-processor counts: GetSystemTimes
can report only the calling processor group on larger systems. No whole-machine
accounting claim is made for that case.
[Microsoft system-counter contract](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes).

Linux future adapter needs /proc PID plus start-time identity, monotonic clock and
clock-tick conversion, VmRSS/VmHWM semantics, MemAvailable and explicit CPU accounting.
RSS is not interchangeable with every Windows private/working-set quantity. Shared
metrics must label source/definition rather than silently merge accounting methods.
No Linux adapter/fixture is advertised as executed here.

MangoHud is a possible approved OpenGL/Vulkan interception source, not an automatic
display-equivalent replacement. Pin logging cadence and verify per-frame samples;
Wayland/X11/compositor/VRR and swap/display semantics require their own calibration.
Avoid automatic uploads or environment injection into owner games. GPU/driver and
display timing paths differ; Windows calibration does not validate Linux.

Resolve official Mojang Java for Linux's exact supported platform/component;
record its own vendor/build/manifest/executable hashes and tools. No Windows binary
or architecture fallback. The launcher's Linux OS-backed credential store is not
implemented, an additional authenticated-launch blocker. Do not introduce plaintext
tokens or bypass normal authentication to obtain Linux performance samples.
