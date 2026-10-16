# Quiet-system preflight — currently NOT READY

Actual read-only [30-second observation](evidence/quiet-preflight.json), raw
[ambient-process.csv](evidence/ambient-process.csv), 2026-10-16. Sample target is
the tooling's own idle Python process, not either owner's Java process. Global
system counters observe ambient load without attaching to owner applications.

Observed host: Windows 11 Home build 26300; i7-14700F, 28 logical processors;
RX 9070 XT driver 32.0.31041.1004; reported desktop 2560x1440 at 240 Hz; existing
Ultimate Performance scheme, unchanged. No battery returned; AC supply is therefore
**unknown**, not inferred true. GPU utilization/thermals, active physical-display
identity, VRR and actual Minecraft render/window settings remain unvalidated.
Two existing owner Java processes and production Launcher 1.4.2 remain running.
Development preparation was also active, so this is not a controlled idle baseline.

The observed mean machine CPU load was about 6.5%, minimum system available RAM
about 10.1 GiB; exact values/timestamps/sample overhead are in the JSON. These are
ambient telemetry only, not game measurements or a claim of lower launcher CPU.
Max observed telemetry-read duration was 0.77 ms. That is neither a game overhead
estimate nor an A/A calibration; observer cost and scheduler delay vary.

Provisional future screening criteria, to freeze before comparisons:

* Observe at least 60 s with 0.5–1 s samples after preparation completes. Mean CPU
  <5% of machine capacity and P95 <10% are practical noise screens for this host,
  not universal performance thresholds. Report absolute one-core/background rates
  too; a hybrid CPU's logical-thread percentage does not measure core equivalence.
* No concurrent owner game, installer, compiler or known heavy workload. Owner
  chooses a quiet window; the tool never closes applications. Unknown background
  work remains a declared condition, not an invitation for a heavy process scan.
* Available RAM must exceed selected heap plus at least 4096 MiB of native/graphics/
  background reserve throughout preflight; no recommendation silently changes the
  heap. Record total/available and paging activity if an approved source exists.
* AC/power mode unchanged; GPU idle <5% and no thermal throttling when reliable
  approved telemetry is available. Unknown GPU/thermal/AC facts block a strong
  quiet-system claim pending owner observation or approved instrumentation.
* Record active display/refresh/VRR, actual game client and framebuffer resolution,
  windowed/fullscreen mode, GPU driver, graphics/options hash and overlay state.
  A refresh value from CIM alone does not lock active display presentation.

Any failed/unknown criterion yields NOT READY with reasons. Thresholds may be
revised from A/A pilot noise before treatment comparisons, with the revision locked.
No power plan, priority, affinity, driver, security or system setting is changed by
these tools. A 30 s ambient observation is intentionally insufficient to declare
the planned 60 s quiet gate passed.
