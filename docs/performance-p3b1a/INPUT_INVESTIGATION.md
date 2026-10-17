# B2 input timeline and limits of attribution

The failure is real: normal-save metadata records yaw
`137.8499755859375`, pitch `12.600001335144043`, unchanged position
`[128.5,96.0,128.5]`. The start image shows `135.0 / 15.0`; the first final image
shows approximately `137.8 / 12.6`. No surviving evidence identifies the input
source or the first moment of movement. Neither Aurora nor the automation helper
is established as the cause.

Review includes the P3-B.1 acceptance/plan, excluded-run JSON, private retained
screenshots, world state, resource series, diagnostic commands, game log and
the original agent action transcript. The transcript's tool invocation/completion
times bracket compound calls, not the exact instant of every nested operation.
Exact F10 and resource times below come from their own records. Timestamps are
UTC on 2026-10-17; log-local times are EDT (UTC−4).

| UTC time / interval | Observed action | Input significance |
| --- | --- | --- |
| 09:00:22.544 | Native Play returned running, PID 20360, creation 134367012225187561 | Exact supervised process, 2048 MiB/G1 confirmed separately. |
| 09:00:39.356–09:00:40.072 | Identity checks and helper window enumeration | No recorded pointer input; foreground acquisition remains possible on later targeted input. |
| 09:00:56.158–09:00:57.431 | Select returned game window, snapshot | Observation, no explicit click in call. |
| 09:01:02.192–09:01:03.901 | Coordinate click Singleplayer | Absolute window-relative target; menu, before accepted start camera. |
| 09:01:07.571–09:01:09.557 | Coordinate double-click world row | Last recorded pointer action before the capture; world loading can capture/recenter cursor afterward. |
| 09:01:23.286–09:01:23.382 | World snapshot | No explicit pointer operation. |
| 09:01:27.180–09:01:27.543 | `t`, snapshot | Chat opens and releases normal gameplay mouse-look context. |
| 09:01:31.478–09:01:31.782 | Type fixed `/tp @s 128.5 96 128.5 135 15` | Command typed but not yet submitted. |
| 09:01:31.782–09:03:55.704 | Gap in recorded UI actions; execution resumes | No evidence this is owner input or the earlier preparation interruption. Do not conflate the two. |
| 09:03:55.704–09:03:56.113 | Resumed snapshot | Observation only in recorded call. |
| 09:03:58.414–09:03:58.652 | Return submits command | Game log confirms teleport at 05:03:58 EDT. |
| 09:04:01.420–09:04:02.587 | F3, start snapshot, then heap checks | Live start camera 135.0/15.0. Persistent pre-start drift is excluded by this image, but not later input. |
| From 09:04:06.103 | F3 hides debug view, accessibility-only snapshot, sampler starts | No image after hiding. The helper's target activation is not independently instrumented. |
| 09:04:06.896366–09:09:06.901569 | 300.000131 s warm-up, 601/601 foreground | No recorded game-target helper input during this span; physical/OS mouse deltas are not recorded. |
| 09:09:14.948002–09:09:15.292420 | JFR.check / GC.heap_info | Managed-JDK attach commands, no game UI input in these scripts. |
| Before 09:09:16.237 | Named JFR preparation | Exact diagnostic-command records retained. No camera checkpoint after warm-up. |
| 09:09:16.237–09:09:16.270 | F10, then accessibility-only snapshot | Keyboard recording trigger; targeted-input activation could still matter. No image before or after F10. |
| 09:09:16.699735–09:09:46.701344 | 30.000147 s resource window, 61/61 foreground | Existing Overlay/JFR/sampler; no recorded pointer or game-key operations within the measured interval. |
| After resource endpoint, before final inspection | Named JFR stop/export and reader review | Commands only; foreground evidence does not cover every millisecond outside the sampled window. |
| 09:09:52.828–09:09:54.142 | JFR review, F3, final screenshot | First observed changed camera. This call itself includes a key and snapshot, so final inspection cannot be excluded as onset. |
| 09:10:06.564–09:10:07.154 | Escape opens pause menu | Occurs after the drift was observed. |
| 09:10:11.774–09:10:12.144 | Coordinate click Save and Quit | Menu pointer movement follows observed failure; cannot be its sole cause. |
| 09:10:19.629–09:10:20.011 | Snapshot confirms world saved | Metadata subsequently confirms changed orientation. |
| 09:10:24.471–09:10:26.089 | Coordinate click Quit Game, native exit check | Normal supervised exit 0 recorded 09:10:26.032. |

## Hypothesis review

| Hypothesis | Supporting evidence | Exclusion / uncertainty |
| --- | --- | --- |
| Mouse movement while captured | Changed yaw and pitch with unchanged position are consistent with mouse-look. No input trace records actual deltas. | No recorded helper pointer action after world loading and before final image. Physical input is neither proved nor excluded. |
| World-entry recenter/focus transition | Prior world entry used a coordinate double-click. Helper inputs automatically activate their target. | Start camera was valid after teleport, so entry alone cannot explain a persistent later offset. Later activation behavior remains unmeasured. |
| Helper key or observation side effect | F3-hide, F10 and final F3 target the game; screenshots and accessibility reads occur at boundaries. | Supported API describes keys/coordinates, not low-level raw mouse semantics. No per-call camera checkpoints in B2. Cannot equate API intent with a recorded relative-delta trace. |
| F3 final verification | First changed image is captured after final F3; this is inside the unresolved interval. | Could also expose an earlier change. Reading F3 is not proof that F3 caused it. |
| Drift during warm-up | Five-minute gap has no orientation observations. | All 601 foreground samples agree with game PID, but foreground is not orientation. No helper UI calls in that gap. |
| Drift between warm-up and recording | About 9.8 s from warm-up endpoint to sampler start; diagnostic attach and F10 occur there. | No intermediate image. JFR/heap scripts contain structured attach commands and reads, not pointer injection. |
| Drift during recording | No camera observation brackets the measurement independently of warm-up/final inspection. | Sampler/Overlay/JFR do not certify input neutrality; 61/61 foreground does not rule out captured mouse motion. |
| Shutdown changed camera | Saved orientation differs. | Final image already differs before Escape/Save/Quit, so shutdown is excluded as the sole origin. It can only preserve or further affect an existing discrepancy. |
| Aurora Client input bug | Aurora is in the stack. | No differential reproduction, input-pipeline evidence or attribution. Presence is not causation; no Client patch justified. |
| Heap/G1 caused orientation change | B2 was a 2048 MiB baseline. | No causal evidence; baseline and candidate valid runs retained 135/15. Never infer a JVM effect from an excluded camera run. |

Earliest unresolved interval is after the start screenshot and before the final
F3 screenshot, including F3-hide before warm-up, warm-up, transition, capture and
final inspection. Persistent drift before the valid start image and shutdown-only
onset are excluded. No stronger historical localization is defensible.

## Smallest supported procedural correction

The evidence identifies a **checkpoint gap**, not a proven defective input API.
The [bounded procedure](CONTINUITY_PROTOCOL.md) removes menu pointer operations,
omits the pre-run teleport by loading the exact frozen orientation, separates UI
work from the stationary windows and adds six boundary observations. It uses the
same supported helper keyboard/screenshot APIs, existing observers and normal save.
No focus switching is planned during warm-up/capture. Manual input remains enabled.
No camera lock, input hook, system setting or Client source change is made.

This is risk reduction plus interval localization. Passing follow-ups cannot
retroactively certify B2 or prove which removed operation caused it. If a boundary
fails, exclude the whole affected session and inspect that earliest interval. At
most one justified procedural correction and one bounded confirmation are allowed.

## Follow-up setup observations

In the fresh P3-B.1a processes, the first observed menu was Options in S1 and
Select World in S2, before this agent's first explicit game key/click in each
process. S1 returned to title with Escape; S2 continued from the observed world
list. These observations do not establish what navigated the menus. They occurred
before world entry and both initial F3 checkpoints were correct; do not silently
describe the complete setup as a fully observed input-free startup. No explicit
helper pointer action was issued in either validation process.

If camera drift recurs, the specific next action is an owner-attended isolated
session with hands off during the stationary periods, retaining these same
checkpoints. Inspect the earliest failing phase and, if it brackets a targeted
helper call, ask the helper maintainer to verify whether target selection or
activation synthesizes pointer events and how absolute coordinates map under
cursor capture/DPI scaling. That contract is currently unknown. Do not add a
global input hook or change system raw-input/acceleration settings to investigate.
