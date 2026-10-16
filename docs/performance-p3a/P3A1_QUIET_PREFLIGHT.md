# P3-A.1 quiet-system observation — BLOCKED

Read-only ambient inventory on 2026-10-16 in the owner's session; **not** a 60-second
test-session preflight. No application was closed or profiled, no power/security/
driver setting changed. The old [QUIET_SYSTEM.md](QUIET_SYSTEM.md) remains historical.

| Condition | Actual observation / limit |
| --- | --- |
| CPU/OS | i7-14700F, 28 logical processors; Windows 11 Home build 26300 |
| CPU activity | Instant CIM LoadPercentage 2%; no distribution or stability claim |
| Memory | Instant free 22,185,508 KiB; total visible 33,304,812 KiB |
| Graphics adapter | AMD Radeon RX 9070 XT, driver 32.0.31041.1004 |
| Reported desktop | 2560x1440 / 240 Hz; active display/VRR/game framebuffer not proven |
| Existing workload | Production aurora-launcher.exe present; no Java or PresentMon process found in the bounded inventory |
| AC / thermal / GPU utilization | UNKNOWN; no battery returned, not evidence of AC |
| Capture overhead | NOT TESTED; help-only execution cannot measure tracing overhead |

The absence of Java in this observation supersedes neither historical observations
nor future preflight requirements. A low instant CPU value is insufficient. Under
the future test user, wait until preparation completes and apply the established
>=60 s CPU/memory observation, unchanged power, actual display/VRR/GPU/thermal/AC
checks and heap-plus-reserve criterion. Ask the owner for a quiet window; never
terminate applications or change settings to produce one. Keep unknowns explicit.
No short game session was attempted; quiet-system gate remains BLOCKED.
