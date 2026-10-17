# PresentMon 2.3.1 field semantics and provenance

This phase uses the pinned standalone console binary, not the PresentMon service,
GUI overlay or a newer release. The actual default header is authoritative. The
console README mixes metric families; producer source determines exact names and
units. Private source copies are hashed in [source provenance](evidence/p3a4a/sources.json).

## Actual export fields

| Actual columns | Meaning and limit |
| --- | --- |
| Application, ProcessID | Producer name/PID; creation identity comes from a separately retained process handle |
| SwapChainAddress, PresentRuntime | Stream identity; Other/0x0 does not identify a DXGI swapchain or instrument WGL entry |
| SyncInterval, PresentFlags | Recorded runtime attributes; -1 is unknown, zero flags do not establish driver policy |
| AllowsTearing, PresentMode | Inferred path attributes; zero tearing is not proof VSync was enabled or VRR disabled |
| TimeInQPC | PresentStartTime in integer QPC ticks, not milliseconds |
| MsBetweenPresents | Difference of consecutive PresentStartTime values in milliseconds |
| MsBetweenSimulationStart, MsPCLatency | Exported NA in this default path; unavailable, not zero |
| MsInPresentAPI | TimeInPresent converted to milliseconds; Other's observed zero lacks measured API call boundaries |
| MsRenderPresentLatency | ReadyTime minus PresentStartTime; ETW completion attribution, not a GPU timestamp query |
| MsUntilDisplayed | ScreenTime minus PresentStartTime, rounded to 0.0001 ms; zero internal sentinel exports NA |
| MsBetweenDisplayChange | Difference from previous tracked display timestamp; sentinel exports NA |
| CPUStartQPC | Derived CPU frame start, based on the previous present/end; distinct from TimeInQPC |
| MsBetweenAppStart, MsCPUBusy, MsCPUWait | Derived frame accounting, not thread execution profiling or CPU utilization |
| MsGPULatency, MsGPUTime, MsGPUBusy, MsGPUWait | ETW-attributed scheduling/work durations; no independent completion calibration here |
| MsAnimationError, AnimationTime | Derived animation accounting; signed error is legitimate; NA remains unavailable |

Header selection, NA emission and formatting:
[CsvOutput.cpp, v2.3.1](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/PresentMon/CsvOutput.cpp).
Derived start, GPU and display calculations:
[OutputThread.cpp, v2.3.1](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/PresentMon/OutputThread.cpp).
No Dropped, DisplayedTime, DisplayLatency, FrameType, DisplayTimeAbs or hardware
scanout column exists in the actual retained default captures. Do not invent them
or infer their values from a similarly named metric family. No exclude-dropped
option was used. NA does not expose the internal reason/final-state enum.

## OpenGL and composition

For a runtime without tracked DXGI/D3D9 entry events, the consumer can create the
present from its first relevant kernel event. Thus TimeInQPC is a downstream ETW
submission proxy, not the exact start of SwapBuffers or the game render loop.
On the observed composed GPU-copy path, the consumer retains the latest copy per
window for a DWM handoff. Dependent presents from one window may be superseded.
ScreenTime can be propagated from DWM's completion; it is not an optical sample.
See [FindOrCreatePresent, HandleDxgkPresentHistoryInfo, HandleDWMEvent and CompletePresent](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/PresentData/PresentMonTraceConsumer.cpp)
and the [path description](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/PresentData/PresentMonTraceConsumer.hpp).

Many uncapped copies can therefore coexist with fewer available estimates. This
is an expected mechanism, not a proof that every NA row was physically dropped.
Internal tracking loss is also distinct from ETW session buffer/event loss. Zero
session counters do not certify one CSV row per WGL call.

Upstream warns that Other runtimes have reduced instrumentation and less accurate
CPU pacing/related latency; hardware-accelerated GPU scheduling can further affect
GPU timing. [Pinned README](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/README.md).
This host's driver overrides, VRR, hardware scheduling and thermals are not
comprehensively established. No system/driver setting was changed.

## Claims and uncertainty

Submission intervals use integer differences before seconds conversion. QPC
resolution is known; ETW-to-API offset/jitter and physical scanout uncertainty are
unknown. UTC identifies recordings, not fine-grained causal alignment. No GC
clock anchors or drift bound were measured. Separate endpoint coverage, producer
exit, identity, raw hashes, ETW counters and independent marker reconciliation are
required; no single check substitutes for the others.

Available display-estimate intervals describe the subset that the consumer
associated with display. Missingness is condition-dependent. Their P99/max cannot
be called Minecraft stutters or physical screen frame times. GPU fields are
diagnostic until independently validated; numeric availability is insufficient.

No currently approved instrument proves rendered-frame completion or optical
smoothness. A future reviewed GPU timestamp/in-game marker or optical experiment
would need separate instrumentation approval; none was installed or injected here.
