# JVM configuration authority

Reviewed against starting HEAD0b1f5ac55825195e8783f519f5ad7c959c7c898b.
No production source or launch-pipeline change accompanies this experiment.

The instance editor submits original/proposed configuration through
`update_instance_configuration`. Native lifecycle validation, stale-value checks
and atomic registry persistence own Save. `performanceMode:null` means Existing
Configuration; mode advice never injects flags. Only this disposable instance's
`memoryMib` may change, through that existing command. Registry6/config7 and all
presets/defaults remain unchanged.

`application::play_instance` reads the saved configuration, validates it and its
custom argument text, then constructs `LaunchOptions::new` with `memory_mib()`.
After managed runtime/session validation, the locked final snapshot is rechecked.
`launch::resolve::resolve_launch_spec` prepends one launcher-owned heap argument
using `instances::settings::heap_argument`; `launch::process` passes the resulting
individual arguments to the exact validated executable and supervises its child.
No shell, system Java or frontend-supplied executable participates.

The intended source-derived JVM argument order is:

```text
-Xmx2048m                         # candidate only: -Xmx1536m
-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump
-Djava.library.path=<validated native root>
-Djna.tmpdir=<validated native root>
-Dorg.lwjgl.system.SharedLibraryExtractPath=<validated native root>
-Dio.netty.native.workdir=<validated native root>
-Dminecraft.launcher.brand=aurora-launcher
-Dminecraft.launcher.version=1.4.2
-cp
<exact validated ordered managed libraries; Minecraft client last>
-Dlog4j.configurationFile=<validated installed client-1.21.2.xml>
```

These are semantic managed-path labels, not a captured live command. No additional
JVM text, explicit Xms, collector selection or GC tuning argument is supplied.
Fabric contributes no JVM tuning arguments. Runtime ergonomic values may respond
to Xmx; that is an experimental mechanism, not a separately supplied setting.

The locked Microsoft OpenJDK21.0.7+6-LTS uses manifest SHA-1
`cb4394a27089d19f65d5baa6cf0482c27c3c7865`. Its observed javaw SHA-256 is
`1ba6a4fb32a6a3d5c925fe837ae6717e2f75743a71e013e668e6dadb909245ba`.
Normal native readiness/Play validates the managed runtime; the sampler checks
PID/creation identity and exact executable hash. A bounded `VM.flags` query
confirms effective MaxHeapSize and G1 without querying command-line/session data.
Overlay's exported `mem_max_mb` supplies an independent runtime maximum check
when a valid recording exists. No live LaunchSpec, command line, token or game
argument vector is exported. JFR uses the existing privacy-filtered profile.
