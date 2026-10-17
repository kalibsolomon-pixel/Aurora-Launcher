# P3-A.4a.1 — installed Aurora 3.0.0 limiter audit

Read-only source inspection: Aurora Client branch `codex/unified-theme-pilot`,
HEAD `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d`; its operating guide and
architecture were read first. No build or source changes. Installed artifact
SHA-256 `43f918207a86f91b01b35045309e89d2b040951722e5ac71d01d886beec9811c`
is authoritative. Disassembly of its FramePacer, MinecraftClientRenderMixin,
RenderSystemMixin, AuroraConfig and FeatureRegistry confirms the behavior below.
Local source/build class bytes did not establish exact build provenance; source
comments alone are not implementation evidence. Class hashes are in the evidence
inventory; disassembly remains private.

| Question | Installed implementation |
|---|---|
| Supported control | Integer slider 0–2000, editable numeric value; 0 OFF, positive UI minimum 1, maximum 2000. A comment's 10 minimum is not enforced. Runtime only clamps negative values to 0; out-of-range configuration is not a supported test route. |
| Requested values | 240/500/750/1000 are exact supported values. No nearest-value substitution. |
| Global limiter hook | `Minecraft.runTick` HEAD, before input application and rendering. It schedules frame boundaries, rather than changing the simulation tick-rate constant or directly measuring presentation. Tick scheduling can still be affected by frame waits. |
| PARK | Read GLFW time; park once for remaining time minus spin tail, then `Thread.onSpinWait` until target. No `Thread.yield` strategy. |
| Alternatives | VANILLA, PARK, HYBRID, SPIN. HYBRID parks to target minus park threshold, then spins; its spin-threshold argument is unused. |
| Compensation | Target advances by exactly 1/FPS from the previous target, rather than observed completion. Reset on first call, cap change, or lateness greater than half a target interval. This prevents simple anchor drift; it is not a measured wake-up overshoot estimator. |
| Adaptation | No CPU-load feedback or measured frame-cost prediction in global-cap pacing. Separate `adaptiveRenderSleeping` chooses a pacing source, not an adaptive overshoot estimator. |
| Low-CPU option | Changes park threshold; PARK consumes spin tail, so that threshold adjustment does not change PARK's wait calculation. Documented for separate review, not fixed here. |
| Clock | `GLFW.glfwGetTime`, seconds on GLFW's monotonic timer. A GLFW base-time reset could change its origin; no such call was found in the limiter. |
| Frame-pacer OFF/VANILLA | Does not disable a positive global cap: fixed HYBRID(2000,500) fallback remains. Only global cap 0 disables that cap. |
| Native interaction | Global cap supplements native constraints. RenderSystem injection replaces the native wait when called and enabled; native Unlimited sentinel 260 normally bypasses that post-swap call in an ordinary world. A lower native cap, AFK/minimized throttle, or VSync can still constrain cadence. |
| Adaptive interaction | With low-latency + smooth + adaptive sleeping + non-VANILLA, frame-head pacing uses the native option, including sentinel 260, then min(global cap). Must be false for isolated >260 global-cap trials. |
| VSync | Separate swap-interval controls remain. DisableVSync is another configurable interaction; options and actual config must be recorded. A cap is not proof of VSync state. |
| Focus/inactivity | Global cap itself has no focus-specific branch; native AFK/iconified behavior can impose lower limits. Test foreground, nonminimized, native inactivity option Minimized. |
| GUI | Aurora screens have separate `guiFpsLimit` behavior; diagnostics occur with every screen closed. Do not confuse GUI cap with global cap. |
| Persistence | AuroraConfig saves asynchronously, blocks at shutdown; profile manager can persist/reapply scoped configuration. Code supports persistence; actual restart verification is a separate observation in the matrix report. |

Factory configuration found in installed bytecode: cap0, smooth true, PARK,
spin 500 µs, park 2000 µs, lowCPU false, lowLatency true, adaptive sleeping false,
disableVSync false, GUI 60. These are defaults, not proof of live configuration.
The non-capture configuration check records actual saved settings and UI1000
after a normal restart, then restores OFF. B timing captures were deferred for
background interference; installed support and configuration persistence do not
establish empirical pacing accuracy at any requested cap.

[GLFW timer documentation](https://www.glfw.org/docs/latest/group__input.html#ga03d4a1039b8662c71eeb40beea8cb622)
documents the monotonic time source. [Java 21 LockSupport](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/concurrent/locks/LockSupport.html#parkNanos(long))
permits early/spurious returns; exact scheduling precision and CPU cost must be
measured rather than inferred from strategy names or source performance comments.

No limiter, rendering hook, Sodium integration or mod artifact was modified.
Potential comment/implementation mismatches are observations for separate owner
review; none authorize optimization in this phase.
