# P3-A.3 frame-pacing diagnosis — PASS for the unintended limiter

The 30 Hz condition is reproduced and controlled by the disposable game's
**Reduce FPS when** setting. No JVM, heap, collector, global input, OS power or
production setting changed. This is a capture-condition correction, not an
optimization result. [Actual diagnostics](evidence/p3a3/pacing.json).

| Same exact child, same scene and heap | Game inactivity | F3 observation | Present-call rate, inner 10 s | ETW display-estimate rate |
| --- | --- | --- | --- | --- |
| A: focused AFK setting, recent F3 input | Recent; exact callback timestamp unavailable | 3147 FPS | 1782.304 Hz | 153.055 Hz |
| B: focused AFK setting, stationary | At least 85.770 s since last game input | 30 FPS | 29.912 Hz | 29.913 Hz |
| C: focused Minimized setting, stationary | At least 69.409 s since last game input | 2672 FPS | 1781.049 Hz | 153.859 Hz |

Each used one unique bounded PresentMon session, exact PID/creation identity,
pre/post-roll and an unchanged 10-second inner window. All sampled foreground
windows belonged to the target game (36 observations/condition). Source CSVs,
options, process telemetry and diagnostics remain private with public hashes.
All three short inspections are VALID WITH WARNINGS, full-run quality INCOMPLETE.
These diagnostic captures used F3; the final scene uses the overlay off.

The installed remapped 1.21.11 client was inspected with the locked runtime's
javap, without executing game code or exporting child arguments. Fabric's exact
[Yarn mapping](https://maven.fabricmc.net/docs/yarn-1.21.11%2Bbuild.1/net/minecraft/client/option/InactivityFpsLimiter.html)
identifies intermediary class_9919. Installed bytecode confirms:

* AFK first stage: more than 60000 ms since game input, min(maximum FPS, 30).
* AFK second stage: more than 600000 ms, 10 FPS.
* Minimized window: 10 FPS; this test remained visible and focused.
* Without an AFK/minimized reason: configured maximum; a no-world GUI has a
  separate 60 FPS branch. Input resets the game's last-input timestamp.

The official [Minecraft limiter explanation](https://www.minecraft.net/en-us/article/save-energy-with-the-frames-per-second-limiter-feature)
also documents the one-minute AFK reduction. Reproduction plus the isolated
setting change establish the cause that was only plausible in P3-A.2. A background
condition was unnecessary. Win32 foreground sampling establishes OS foreground
ownership; it is not a direct query of Minecraft's private input timestamp.

Persisted maxFps=260 is the installed UI's **Unlimited** sentinel. VSync=false,
fullscreen=false, pauseOnLostFocus=false, overrideWidth/Height=1920/1080. The
installed UI and F3 confirm effective windowed rendering at 1920x1080. Individual
1.21.11 graphics are represented as Custom: smooth lighting/leaves on, improved
transparency off, clouds off, particles All, GUI 2, normal FOV, mipmaps 4,
texture filtering None, render/simulation 12/8. Exact options bytes are frozen in
[the canonical A lock](workload-a-lock.json). This replaces the old proposal's
ambiguous Fancy label with the actual reviewed individual settings.

Desktop inventory remains RX 9070 XT / driver 32.0.31041.1004,
2560x1440 / 240 Hz; active Ultimate Performance scheme unchanged. No driver-profile
editor was opened or global setting changed. Driver-level caps, VRR and thermals
are not comprehensively observable here. They cannot explain the controlled
AFK-only 30 FPS branch; they remain limitations for performance comparisons.

The high-rate captures retain many unavailable display estimates and long estimate
gaps. Present calls, F3 rendering FPS and display estimates are different quantities.
No missing display estimate is called a lost ETW event, and no 153 Hz estimate is
represented as physical display proof. High-rate timing/observer interference remains
an explicit calibration blocker; see [PresentMon calibration](PRESENTMON_CALIBRATION.md).
