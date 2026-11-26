# Aurora Launcher 2.8.0

**Performance-mode launch optimizations.** Improves frame pacing and startup
consistency for Minecraft instances configured in Performance mode:

- Adds a launcher-owned low-latency GC preset for managed Mojang Java runtimes
  in Performance mode:
  - Uses Generational ZGC on Java 23+ (`-XX:+UseZGC`) and Java 21–22
    (`-XX:+UseZGC -XX:+ZGenerational`) when host RAM is at least 6 GiB and the
    configured heap is at least 4 GiB.
  - Falls back to low-latency G1 tuning (`-XX:+UseG1GC`, 4 MiB regions, 12 ms
    pause target, 15% reserve) on Java 17–20 or smaller heaps/hosts.
  - Emits equal `-Xms`/`-Xmx` and `-XX:+AlwaysPreTouch` only when host memory
    headroom is safe (at least 2 GiB headroom and heap at most 16 GiB).
  - Automatically skips preset injection below Java 17, on custom Java
    runtimes, or when plan/loader/user JVM arguments already configure a
    garbage collector or initial heap flag.
- Raises the Performance-mode advisory heap ceiling to 4096 MiB (while keeping
  host memory safety reserves) and honors the persisted instance performance
  mode when advice is queried without an explicit mode override.
- Opts Performance-mode Minecraft child processes out of Windows EcoQoS power
  throttling (`ProcessPowerThrottling`) on both online and offline launches,
  failing softly on older Windows builds.

Update trust, application identity and managed-data locations remain unchanged.
Windows publisher signing remains unconfigured; signed update artifacts provide
release authentication.
