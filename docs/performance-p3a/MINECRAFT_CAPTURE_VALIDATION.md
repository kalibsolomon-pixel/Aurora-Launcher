# Minecraft P3-A.3 — restored scene validated, measurement confidence PARTIAL

[Actual final capture](evidence/p3a3/final-capture.json),
[pacing diagnosis](FRAME_PACING_DIAGNOSIS.md), [canonical world](WORLD_SNAPSHOT_LOCK.md).
One final short capture used a fresh copy of the independently restored canonical
A archive. Aurora performed normal authenticated entitlement, deep content/runtime
validation and exact-child launch. Microsoft OpenJDK 21.0.7+6-LTS, Minecraft 1.21.11,
Fabric 0.19.5, no optional mods, Balanced/2048 MiB and empty JVM text are unchanged.
No child arguments, reusable secrets or account identity were exported.

| Final actual observation | Result |
| --- | --- |
| Warm-up / measured inner window | 342.978 s stationary / 20 s, 26 s requested capture |
| Target | Exact native child PID 11780 / creation FILETIME 134366777835965485 |
| Runtime/framebuffer/camera | Locked Java hash; F3 verifies Java 21.0.7, 1920x1080, (0.5,66,0.5), yaw/pitch 0/10 before and after |
| Focus / inactivity | All 56 foreground samples target the game; Minimized limiting; debug overlay off during capture |
| PresentMon | Exit 0; 46170 rows, one Other stream 0x0 |
| Independent presentation-call rate | 1832.378 Hz; proves the stationary scene avoided the 30 FPS branch, not physical FPS |
| Unavailable display estimates | 43026 rows retained, not automatically lost events |
| Display-estimate intervals | 2486 inner intervals; mean 8.044 ms / 124.320 Hz, max 504.896 ms; gaps retained |
| Short reader | VALID WITH WARNINGS; full measured-run quality INCOMPLETE |
| ETW loss | Zero counters through inner window; shutdown tail unknown |
| Process telemetry | 61 samples / 30 s, full 20 s weighted window, max interval 0.500419 s |
| Process CPU / sampled RSS | 262.506% of one logical core / 1122340864 bytes; neither GC CPU nor Java heap |
| Machine busy CPU | 14.677% during capture versus fresh preflight 1.232%; includes game/OS/observer/unrelated work |
| Normal exits | Supervised Minecraft exit 0; retained-handle isolated launcher exit 0 |
| Post-exit scene/options | Exact canonical conditions including 0 entities and 0 block entities; options hash unchanged |

The fresh 60-second main-menu preflight observed AC power and >=21132648448 bytes
available RAM. GPU idle/VRR/thermals were unavailable. The machine-minus-game CPU
estimate includes instrumentation and OS work; it is not attributed solely to
unrelated apps. No app, priority, power or driver settings were changed.

F3 observations were 3477 before warm-up and 3582 after capture with new input;
they are instantaneous rendering observations. The final no-input presentation calls
and earlier controlled AFK/Minimized comparison establish limiter control. Display
estimates are a separate quantity. Their high unavailable fraction and recurring
long gaps are not validated physical stutters and cannot support a JVM performance
claim. The 120-Hz overhead experiment does not bound this uncapped workload.

The final native restart revalidated canonical paths, binary/signature and live
WebView isolation under the owner's approved startup-refresh sequencing exception.
The earlier launcher disappeared during the interruption; its exact exit is unknown.
All failed/interrupted attempts and old P3-A.2 bytes remain preserved. Capture QPC
origins are session-specific; no cross-session or absent-GC causal alignment is claimed.
P3-B remains NOT READY / NOT AUTHORIZED. Historical P3-A.2 evidence follows.

# Minecraft P3-A.2 capture — real data, overall PARTIAL

[Actual sanitized capture](evidence/p3a2/minecraft-capture.json),
[native/auth proof](evidence/p3a2/native-auth.json),
[quiet preflight](evidence/p3a2/quiet-preflight.json).
One real Workload A Minecraft session and one bounded gameplay capture occurred.
No B/C game execution, repeated A/B comparisons or P3-B work occurred.

All three prepared installations revalidated Ready through native commands, including
402 files / 99,041,788 bytes of exact Microsoft OpenJDK 21.0.7+6-LTS runtime per
validation. Game execution used the locked javaw SHA-256 and official runtime manifest
SHA-1 from [JAVA_RUNTIME_LOCK.md](JAVA_RUNTIME_LOCK.md). Main menu and F3 verified
Minecraft 1.21.11/Fabric, Java 21.0.7, 1920x1080 framebuffer, expected RX 9070 XT and
camera 0.5/66/0.5, yaw/pitch 0/10. A had no optional mods or Fabric API.

The owner completed legitimate sign-in interactively in the isolated launcher and
confirmed this in chat. Supported session restoration and entitlement/readiness
returned ready without blockers. No production account database copying, fabricated
records, token export, credential backup or direct production vault inspection.
The owner signed in while read-only revalidation was underway: prelaunch test-tree
checks had passed, but final live WebView profile confirmation followed sign-in.
This observation-order limitation is retained; it is not retroactively claimed as a
fully observed pre-authentication gate. Final native/WebView/canonical paths all
point to the intended isolated tree. Shared credential continuity risk was accepted;
production sign-in was not probed, so its reauthentication effect remains unknown.

Normal launch with Aurora's explicit 1920x1080 window setting failed before spawning:
`launch_placeholder_unresolved` for resolution_width. The failed attempt is retained.
Restoring the supported null window setting and using Minecraft's own saved
1920x1080 overrideWidth/overrideHeight allowed the normal authenticated pipeline.
No native resolver fix, fake session, shell launch or validator bypass occurred.
An earlier driver result lost the structured exception crossing the browser boundary;
the checked native call preserved the concrete code. No failed game child existed.

The isolated game PID was **25332**, creation FILETIME **134366600133671485**.
The executable path/hash matched the locked managed runtime; no child argument array
was read or exported. Capture ran **2026-10-16 21:45:51.390345–21:46:17.513650 UTC**.
It followed 334.372 seconds of stationary warm-up, used a unique ETW session, exact PID,
QPC timestamps, display tracking and no input tracking. PresentMon timed 26 seconds
with a predeclared 20-second inner window starting four seconds after the origin.
Capture and telemetry retained independent read-only process identity/handles.

| Actual data / gate | Result |
| --- | --- |
| PresentMon process exit | 0, bounded normal start/stop |
| Raw presentation rows | 776, one Other stream 0x0, no unavailable display rows |
| Inner intervals / coverage | 598 / PASS, 250 ms boundary screen |
| Offline short reader | VALID WITH WARNINGS; original UTF-16 decoding failure retained, same bytes reinspected |
| Presentation pacing | Approximately 29.924 Hz despite saved Unlimited/VSync OFF; a condition discrepancy, not an optimization result |
| CPU/memory telemetry | 61 samples over 30 s; complete counter-derived 20 s window, max interval 0.501306 s |
| Sampled / OS-peak RSS | 1,738,133,504 / 1,787,535,360 bytes |
| Mean process CPU | 8.75045% of one logical core; not GC CPU |
| Background interference | Capture mean machine CPU 6.04158%, higher than preflight 0.68541%; retain warning |
| Game GC / correlation | Unavailable; Balanced profile kept, no hidden logging flags or laboratory instance |
| Clean supervised Minecraft exit | exited / 0 / no diagnostic, 21:47:59.145 UTC |
| Isolated launcher normal exit | 0, retained child handle, 21:48:50.829297 UTC |
| Full measured-run quality | INCOMPLETE; short calibration never satisfies 300/600 s and three-source gates |

F3 immediately after capture showed about 30 FPS with an unlimited target; later
input/menu interaction showed thousands of FPS. Saved inactivity limiting was AFK.
This is consistent with an inactivity/focus limiter, but the cause was not isolated.
Do not treat these samples as an unlimited stationary workload baseline. No retry
was made to choose more favorable metrics. The native 1.21.11 UI represents the
individual requested graphics as Custom, with smooth lighting/leaves enabled,
clouds off, particles All, GUI scale 2, render/simulation 12/8 and normal FOV. The
older specification's generic Fancy label is not a newly validated preset mapping.

After a clean save/exit, the complete snapshot/restoration validated, but NBT inspection
found 19 non-player entities rather than the specified zero. See
[WORLD_SNAPSHOT_LOCK.md](WORLD_SNAPSHOT_LOCK.md). Captured-world reproducibility and
exact workload conformity are distinct. Capture pacing, entity drift, full cadence,
loss/overhead confidence and pre-auth observation ordering remain open. No performance
improvement or causal GC/stall conclusion is claimed. P3-B remains unauthorized.
