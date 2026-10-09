# Aurora Performance P3 — Minecraft startup profiling

Date: 2026-10-09. Verdict: **COMPLETE** for this Windows warm-filesystem workload.
Ten primary real launches, one successful pilot, five JFR-disabled controls and
three retained failed setup pilots establish a reproducible startup investigation.
All 16 successful launches reached a deterministic usable menu and exited with
code 0. No production optimization, release, push, installed-Launcher replacement
or profiler upload was performed.

The primary profiled median is **20.481 seconds from test-harness Play initiation
to usable menu**, or **20.412 seconds from receipt of the frontend Play marker in
Rust**. The corresponding control medians are 19.640 and 19.570 seconds. The largest
observed work cluster is Minecraft texture, sprite, model and font preparation;
class loading/mixin work is another substantial contributor. Aurora's actual
`onInitializeClient` callback is 208 ms median, about 1% of total startup. Treating
all game startup as Aurora initialization would be incorrect.

No evidence here establishes a 20%, 33% or 50% production improvement. Those
require approximately 4.1, 6.8 or 10.2 seconds saved at this baseline. A reasonable
next-phase **hypothesis** is 0.5–2 seconds from resource-path work plus small,
independently measurable Launcher changes. Zero saving remains possible until
controlled implementations are tested; these are not additive proven savings.

## Identities and preserved workload

| Component | Verified identity |
| --- | --- |
| Launcher source | `7a7eeb62d145857df26c30da4c3bf61c4260479d`, branch `codex/client-3-production-authority` |
| Client source reference | `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d`, branch `codex/unified-theme-pilot` |
| Installed Client artifact | Aurora 3.0.0; 3,077,377 bytes; SHA-256 `43f918207a86f91b01b35045309e89d2b040951722e5ac71d01d886beec9811c` |
| Installed production Launcher | SHA-256 `15bfd5d71bbe73c9ad7f3010c052f3f1669d81abfa4790b8c48efb8b169fd868`; unchanged |
| Research executable | SHA-256 `6a70811a22b40c880ec9ee5b0f2a599ad3f81ab29433ef573aa1d713c251c7bd` |
| Agent | SHA-256 `e9879e690dd57afa9297b2fa7fc2b5ea52fcbfd527d6aa805ba5da4f28241c03` |
| Bootstrap marker bridge | SHA-256 `9dc52fd92675dbde405e48885d6910582952c43e229b5b978adb26f0927b8f67` |
| JFR configuration | SHA-256 `9199890517a72b9b9d41a3cb54cd827a901a4b63666e2774e6ed9545a4f4b644` |
| Managed Java | Microsoft OpenJDK 21.0.7+6 LTS, exact `java-runtime-delta` Windows x64 runtime |
| Java diagnostic executable | SHA-256 `0afb1170b83c156dfdb7f54bb34c9b058064175c329a5d80b782bcc9dbaa2164` |
| Java game executable | SHA-256 `1ba6a4fb32a6a3d5c925fe837ae6717e2f75743a71e013e668e6dadb909245ba` |
| Game/loader | Minecraft 1.21.11 / Fabric Loader 0.19.5 |
| Hardware | Intel Core i7-14700F, 20 cores / 28 logical processors; AMD Radeon RX 9070 XT |
| OS/driver | Windows 11 Home, build 26300; display driver 32.0.31041.1004 |

The executable is the pinned Launcher source **plus copy-only research patches**,
not an unmodified production binary. Four tracked files in the disposable copy
add the research feature, isolated data root, process hooks and numeric clock
bridge. A new helper supplies per-trial JVM properties. Their hashes and the Java
source identities are retained in the evidence. The original repositories' tracked
files remained unchanged. The Client jar was not rebuilt or instrumented on disk;
the Java agent inserts passive boundary calls into selected loaded classes.

The selected registered ready instance was copied byte-for-byte; no other instance
was selected. Display names and identifiers remain private. The workload retained
31 top-level mod jars, including Sodium 0.8.14, ModernFix 5.26.3-build.2, Lithium
0.21.4, Continuity, Voice Chat and Aurora. Embedded Fabric modules are additional
entrypoints, not 31 additional top-level jars. All versions/hashes are in
[inputs.json](docs/performance-p3/evidence/inputs.json).

There were three resource-pack files, one unselected directory and one shader-pack
file. None of the present resource-pack files/directories was selected. Options
referenced vanilla plus one **unavailable** file pack; retaining that stale setting
does not prove an active custom pack. Iris and its configuration were absent, so
the shader archive's presence is not evidence of shader use. Names remain private.
Graphics: fullscreen, GUI scale 3, VSync off, 260 FPS cap, mipmaps 4, render and
simulation distances 20, particles 0. Launcher memory 6,144 MiB; no additional user
JVM arguments. No modpack, graphics or shader variant was used for primary results.

## Preservation and JNA safety

The immutable preflight contains branches/HEADs/remotes/history, full tracked and
untracked inventories, document/manifest hashes, managed instances/runtime/cache,
diagnostics and the installed executable. It hashes **43,617 pre-existing files**:
641 Launcher tracked, 15,144 Launcher untracked, 137 prior diagnostic, 422 Client
tracked, four Client untracked, 24,193 protected managed, 3,072 documented operational
cache, three native-temp and one installed-executable files. Sixteen-thousand-plus
developer/repository files were not cleaned or normalized. Copies use file copies,
never links. Mutable research state and builds live in a newly created local root.

Final full comparison: **zero protected missing files, content changes or mtime
changes**, zero original tracked-membership losses, and all 17 JNA-related original
files intact. There were 38 content changes and 39 additional mtime-only changes in
the documented operational WebView cache. The first research boot used that cache
despite the configured window data directory; subsequent launches explicitly set
`WEBVIEW2_USER_DATA_FOLDER` to the research root. Counts remained stable afterward.
This is an operational-cache exception, not a claim that all 43,617 files were
byte-identical. No operational cache was restored or overwritten. The installed
production executable, owner instances, runtime, protected caches and both original
repositories matched the baseline. [Preservation receipt](docs/performance-p3/evidence/preservation.json).

[JNA 5.17.0's actual source](https://raw.githubusercontent.com/java-native-access/jna/5.17.0/src/com/sun/jna/Native.java)
selects `jna.tmpdir` before native dispatch loading and cleans marked `jna*.x`
objects in that selected directory. Three controlled probes used the exact managed
Java and JNA jar (SHA-256 `b3a9408e7c51e08ef0e3bfcc08f443f6ec0f6191ba8cd7c18d53d2b22e5bdbc0`).
Each proved extraction to its fresh directory, deletion of disposable marked
fixtures **inside** it, and unchanged marked sentinels outside it. Original JNA
objects were hashed before any game launch and again afterward.

Every real launch also calls JNA's actual `getTempDir` and reads the actual loaded
`jnidispatchPath` after initialization. Both canonical paths must match that
trial's JNA directory. All successful trials passed; native-library events provide
additional corroboration. `java.io.tmpdir`, child `TMP`/`TEMP`, and LWJGL's shared
library extraction directory are also fresh per trial; normal game natives remain
inside the copied game. Whole-owner-state comparison checks for unexpected writes.
These overrides exist only in the copied research process path; production launch
arguments and user settings were unchanged. [Probe receipt](docs/performance-p3/evidence/jna-isolation.json).

Normal authentication remained enabled, including OS-stored refresh-credential
rotation. The investigation neither copied secrets into evidence nor tried to
restore stale credentials. Installed production software was not run as a benchmark.

## Tools and recording configuration

JFR starts at JVM initialization with the managed Java 21 runtime. JDK 25.0.4
command-line tools read the recordings; agent classes target Java 21. Portable
JDK Mission Control 9.1.2 was downloaded from its official release and verified
against release-asset SHA-256 `d901afb52131ecea5851f3171d3be3930177b7b6a8e0edc28e17f04efcf41fa5`.
It opened baseline-06 locally after benchmarking. Method Profiling independently
showed `TextureUtil.solidify` and sprite/model processing among leading samples.
No JMC report was uploaded. Its workspace and native temporary storage were isolated.

[JFR configuration guidance](https://docs.oracle.com/en/java/javase/21/jfapi/configuration.html)
and [JVM startup recording options](https://docs.oracle.com/en/java/javase/21/docs/specs/man/java.html)
informed `startup.jfc`: all exact-runtime JDK events explicitly disabled except a
reviewed allowlist; execution/native-method sampling 20 ms, allocation sampling
50/s, blocking/class-load/file-I/O thresholds 10 ms, CPU/class statistics 1 s,
GC/compilation events and native-library chunk observations. Custom phase events
carry code-defined labels, edges and elapsed nanoseconds only. The transformer
disables directly declared third-party JFR event classes by default; a fake
secret-bearing custom event tests that boundary. The actual enabled event set is
checked before every successful recording is exported.

JVM additions, expressed without private paths:

```text
-Djna.tmpdir=<trial>/jna
-Djava.io.tmpdir=<trial>/tmp
-Dorg.lwjgl.system.SharedLibraryExtractPath=<trial>/lwjgl
-javaagent:<research>/agent.jar=<trial>
-XX:StartFlightRecording=name=AuroraP3,settings=<research>/startup.jfc,
    filename=<trial>/startup.jfr,disk=true,dumponexit=true,maxsize=128m
-XX:FlightRecorderOptions=repository=<trial>/jfr-repository
```

The last two additions are absent in controls. Ordinary validated game JVM
arguments, memory setting, managed executable and structured spawn remain intact.
Authentication arguments are deliberately neither serialized nor listed here.
No ambient Java injection, shell command assembly or final-validation bypass was
introduced. Research agent ASM 9.10.1 comes from the existing verified classpath
(SHA-256 `ed825d10ab1399c8c0cb669e688cf0c8c82629b4c8399b58352b68e92ca10fcb`).

Reference-method evaluation:

| Reference | Decision |
| --- | --- |
| [ModernFix launch profiling](https://github.com/embeddedt/ModernFix/wiki/Profiling-game-launches) | Reused startup profiling methodology. Installed fork retained. Its spark launch option can upload a profile, so it was not enabled. No assumption that upstream fixes match this 1.21.11 fork. |
| [TaskManager](https://github.com/Wueffi/TaskManager) | Per-entrypoint timing is useful. Reviewed current source targets Minecraft 26.2 / Java 25, so no unverified 1.21.11 installation. Narrow agent callbacks provide equivalent timing coverage without another mod. |
| [spark](https://github.com/lucko/spark) | In-game commands miss early JVM/bootstrap work; no remote-upload workflow or additional mod installed. JFR covers startup from process initialization. |
| [LazyDFU](https://github.com/astei/lazydfu) | Historical deferred-DFU idea is a hypothesis, not evidence of present benefit. No LazyDFU install or speculative ordering change. |

## Timeline definition and clock correlation

| Boundary | Evidence and precision |
| --- | --- |
| Harness initiates Play | Direct timestamp immediately before ordinary UI input; includes injection/dispatch overhead |
| Frontend Play request reaches Rust probe | Direct numeric event 108; primary native timing origin; median 68.945 ms after harness initiation |
| Preparation starts/ends | Direct native event 12; includes work through supervised process publication |
| Final locked validation | Direct native event 30; ordinary content/registry validation retained |
| Java spawn requested/returned | Direct event 31 surrounding structured `command.spawn`; separate immediately-after-return private confirmation |
| OS process created | Windows `GetProcessTimes` for the exact returned child PID, retaining its handle until exit |
| JVM early initialization | Creation-to-agent-origin interval measured; individual VM subphases not all instrumented |
| Agent origin / premain | `System.nanoTime` origin plus epoch bridge; premain callback follows bridge initialization |
| Fabric setup/discovery | Direct `Knot.init`, `FabricLoaderImpl.load/setup/freeze` and `invokeEntrypoints` boundaries; class loading inside them remains inclusive |
| Mod/Aurora callbacks | Direct zero-argument `onPreLaunch`, `onInitialize`, `onInitializeClient` method entry/normal return; nested inherited/proxy callbacks can overlap |
| Minecraft main / constructor | Direct intermediary methods mapped to official classes; main continues beyond menu and therefore has no completed startup total |
| Texture/model/resource preparation | JFR sampled stacks within measured constructor/post-constructor windows; no precise exclusive resource-reload wall timer |
| Usable menu | Direct two completed frames of Aurora title screen with loading overlay absent, followed by secondary visual check |

Native spans use one `Instant` clock. A bracketed Unix correlation point joins it
to OS creation time. JVM marker nanoseconds correlate with JFR phase timestamps;
the median timestamp-minus-elapsed offset joins the clocks. Maximum within-run
JFR offset spread was 321 microseconds. Native bridge brackets quantized to zero
at recorded microsecond resolution; that is not proof of zero clock error. JVM
wall/monotonic drift stayed within about 1.01 ms. Report millisecond-scale accuracy,
not nanosecond cross-process precision. Controls use the same JVM monotonic/epoch
bridge without JFR. No unrelated wall-clock/log timestamps were subtracted.

The menu criterion is stronger than spawn, splash disappearance or a screenshot.
The fixture rejects a loading overlay and a single frame, then accepts the second
completed unobstructed title-screen frame. It does not test world/server loading,
every button's action or future asynchronous mod work.

## Cohort and results

Warm means copied, existing game/runtime/assets and warm OS filesystem caches;
fresh Launcher and JVM each time, no cache purge or machine reboot. Each Launcher
was allowed at least six seconds before Play. The installed modpack remained intact.
Background desktop/network/OS activity, CPU frequency, graphics driver caches and
provider latency were not controlled. This is not a cold-storage benchmark or a
benchmark of repeated Play with an already cached Rust session in one Launcher.

Execution order: pilots 01–04, B1, C1, C2, B2, B3, C3, C4, B4, B5, C5, B6–B10.
B3 was interrupted **after** its usable-menu marker, remained idle, and later
exited normally. It is retained, including its relatively slow startup. A copied
fullscreen setting changed after the interruption and was restored before C3;
owner settings never changed. The post-menu pause weakens the temporal matching
of pair 3. All startup analysis is clipped at the deterministic menu boundary.

| Cohort | n | Harness Play → menu median | Min–max | Mean | Sample SD |
| --- | ---: | ---: | ---: | ---: | ---: |
| Successful pilot | 1 | 20.309 s | 20.309–20.309 s | 20.309 s | — |
| JFR baseline | 10 | **20.481 s** | 19.194–21.014 s | 20.313 s | 0.638 s |
| JFR disabled control | 5 | **19.640 s** | 19.372–20.586 s | 19.892 s | 0.500 s |

Earlier 20.5–30.1-second observational results used different timing conditions
and upper-bound observations. This table is not evidence of an optimization over
those observations; P3 changed measurement precision and collected repetitions.

| Baseline | Harness Play → menu | Native marker → creation | Agent origin → menu | Exit |
| --- | ---: | ---: | ---: | ---: |
| 01 | 19.194 s | 3.453 s | 15.466 s | 0 |
| 02 | 20.630 s | 3.726 s | 16.629 s | 0 |
| 03 | 21.014 s | 3.732 s | 16.992 s | 0 |
| 04 | 20.333 s | 3.663 s | 16.388 s | 0 |
| 05 | 20.787 s | 3.429 s | 17.083 s | 0 |
| 06 | 20.008 s | 3.565 s | 16.158 s | 0 |
| 07 | 20.870 s | 3.954 s | 16.634 s | 0 |
| 08 | 19.333 s | 3.615 s | 15.425 s | 0 |
| 09 | 20.188 s | 3.545 s | 16.364 s | 0 |
| 10 | 20.776 s | 3.565 s | 16.927 s | 0 |

Failed pilots, all exit 1: pilot-01 exposed Java's rejection of verbatim Windows
classpath prefixes; an A/B class-resolution probe confirmed ordinary absolute
paths work. Containment checks still canonicalize, then retain ordinary spelling.
Pilot-02 hit Fabric's duplicate-ASM guard; the agent stopped bundling ASM and uses
the existing exact classpath object. Pilot-03 exposed Knot's inability to see the
system-loader marker helper; a bootstrap-visible bridge solved visibility. No
Fabric security guard was disabled. Crash Assistant's helper was closed normally
without sending a report. These setup failures are not primary slow-run exclusions.

The five paired JFR-minus-control JVM intervals were **−1519, +768, +1804, −91,
+1168 ms**; median +768 ms, mean +426 ms, sample SD 1286 ms. Paired harness totals
had median +997 ms; whole-cohort harness medians differ by +842 ms. This suggests
potentially material overhead but does not identify a clean causal constant with
five noisy, partly interrupted pairs. Both groups retain the agent/markers, so
total agent overhead is unmeasured. No corrected production timing is claimed,
and no overhead estimate is subtracted from individual phases. More randomized
controls are required before judging improvements below roughly 0.5 seconds.

## Attribution

Disjoint timeline partitions (median per partition; do not sum medians as an exact
total):

| Interval | Median | Interpretation |
| --- | ---: | --- |
| Harness initiation → native Play marker | 0.069 s | Input/IPC marker receipt |
| Native Play marker → OS creation | 3.590 s | Launcher critical path |
| OS creation → JVM marker origin | 0.210 s | Early JVM/agent bootstrap, not all JVM initialization |
| Marker origin → Knot entry | 0.384 s | Agent/loader preparation |
| Knot entry → exit | 2.626 s | Fabric startup inclusive of discovery, freeze, transformations and prelaunch entrypoints |
| Knot exit → Minecraft main entry | 0.161 s | Main-class preparation |
| Main entry → Minecraft constructor entry | 5.181 s | Minecraft bootstrap/registries/DFU/class loading; no exclusive DFU duration |
| Minecraft constructor | 4.003 s | Client setup, mod entrypoints, resource jobs and graphics work overlap |
| Constructor return → usable menu | 4.235 s | Remaining resource/model/texture work, render-thread finalization and first menu frames |

Native preparation is 3.613 s median, slightly longer than creation because it
ends after spawn publication. Its direct child spans show where that time goes:

| Native operation | Median inclusive | Qualification |
| --- | ---: | --- |
| Initial complete-instance validation | 0.575 s | Includes game integrity and a mod scan |
| Mojang/composed game metadata | 0.114 s | Network/provider-dependent; official sources retained |
| Runtime metadata | 0.035 s | Official runtime authority retained |
| Two direct compatibility mod scans | 0.305 s | Two additional scans also occur inside initial/final instance validation |
| Runtime validation | 0.355 s | Contains 0.286 s bounded Java diagnostic and about 0.060 s recorded hashing |
| Rust session restoration | **1.602 s** | Fresh-Launcher provider chain; existing usable-session cache already implemented |
| Final locked validation | **0.580 s** | Required correctness/security boundary, unchanged |
| Structured spawn call | 0.006 s | Spawn itself is not the multi-second delay |

Across all nested native spans, game-integrity work totals 0.827 s median, with
0.814 s hashing; four mod inventory scans total 0.614 s, with 0.145 s recorded
hashing. Those overlap the validation rows above. Do not add them again. Every
primary trace still records exactly one P2 startup-update audit root. No P2 audit
was removed, bypassed, cached or weakened.

JVM findings:

* **Resource/texture/model/font CPU work dominates observed samples.** Across ten
  startups, 1,699 of 2,906 Java execution samples (58.5%) have leaves in the
  explicitly listed texture/sprite/model/font families. `TextureUtil.solidify`
  contributes 464 leaves, `SpriteContents.scanSpriteContents` 433. The latter is
  **Sodium-injected code in a Minecraft class**, proven by inspecting the exact
  jar's `SpriteContentsMixin` with `javap`, not vanilla work merely because its
  JFR declaring class says Minecraft. The reducer assigns that method to Sodium.
  `solidify` is present in the verified official mappings. Stacks show
  mipmap generation, native image processing, paletted sprites, Sodium's item
  model builder and font loading. Resource work is already concurrent during
  construction and continues afterward. This is a CPU-work diagnosis, not an
  8.238-second exclusive resource timer or a 58.5% wall-time claim.
* **Mixin/class loading is material but overlapping.** 10,606 transform calls per
  primary run total 2.183 s median inclusive thread time; thresholded class-load
  events total 3.085 s inclusive. Transformations occur in both loader and game
  phases, including nested loads. Bytecode/ASM samples support the diagnosis.
  Neither number can be added to the phase table or assigned wholesale to Aurora.
* **Fabric discovery alone is comparatively small.** `load` is 0.138 s median,
  enclosing `setup` 0.138 s; `freeze` is 0.680 s. Knot's 2.626 s includes other
  loader/class/mixin work. Three `invokeEntrypoints` spans total 2.070 s, including
  lazy class preparation around actual callbacks. That is not 2.070 s of mod
  callback bodies.
* **Aurora callback cost is bounded.** `AuroraClient.onInitializeClient` median
  208 ms, range 152–253 ms. Current source loads config/profile, migrates settings,
  registers features/HUD/keybinds and callbacks. Other substantial individual
  callbacks include ModernFix client 156 ms, Voice Chat client 150 ms and common
  66 ms, Sodium prelaunch 118 ms/client 64 ms, and RRLS Fabric client 85 ms.
  These are inclusive observed method costs, with dependencies and class loading;
  they do not establish that any mod should be removed. Inherited/proxy methods
  in the full evidence overlap and must not be summed blindly.
  This callback duration is not Aurora's total marginal cost: its mixins, hooks,
  assets and menu rendering can execute elsewhere. No Aurora-disabled variant was
  run, so there is no causal Aurora-on/off delta.
* **Aurora also appears in first-menu rendering.** Only 22 execution leaves across
  all startups were Aurora methods; 15 are `BlurPanelRenderer.Frame.readback`.
  Native samples include `glReadPixels`. The source converts a pixel buffer and
  reuploads the texture. This justifies a narrowly measured render investigation,
  not assigning the entire post-constructor interval to blur or assuming a
  driver-safe zero-copy replacement.
* **GC and blocking are secondary in this cohort.** Union of observed GC pauses
  is 189 ms median (150–203 ms); GC inclusive event totals are larger because
  concurrent collections overlap. Main/Render-thread blocking union is 134 ms
  median (105–162 ms), largely Voice Chat native initialization's `Thread.join`.
  Sampling shows native event-loop waits on other threads; those are not proof
  the critical thread was idle for the same duration.
* **Compilation is parallel background work.** JFR compilation durations sum to
  53.676 thread-seconds median across compiler workers. This is not a 53-second
  startup delay. No JVM flag-tuning gain is established. Allocation samples are
  statistical weighted bytes, not a precise heap census.
* **Slow synchronous Java file I/O is not the leading measured stall.** Reads
  above 10 ms total 27.5 ms median, about two events per trial. This excludes short
  reads, native/memory-mapped I/O and work outside JFR's Java file events. ZIP
  lookup/decompression and filesystem enumeration appear in samples; disk and
  hardware effects are not fully attributed. Median machine CPU load sampled at
  18.1%, JVM user 11.0%, JVM system 2.4%, all normalized across 28 logical CPUs;
  these do not rule out a busy individual core or background interference.

Execution-leaf ownership across all primary trials, using class prefixes plus the
verified Sodium scan and explicit injector-label exceptions:

| Owner classification | Samples |
| --- | ---: |
| Minecraft/Mojang | 1,377 |
| Sodium injected into Minecraft classes | 435 |
| Other third-party mod/library class prefixes (including ordinary Sodium classes) | 505 |
| JVM/JDK | 417 |
| Graphics/native support Java wrappers | 101 |
| Mixin engine | 40 |
| Aurora | 22 |
| Fabric | 8 |
| Research | 1 |

These 2,906 observations partition sampled Java leaf methods, not wall time or
total CPU seconds. Inclusion of an owner higher in a stack is recorded separately
and overlaps; unresolved injected-method provenance remains a limitation.

## Ranked next optimization proposals

These are proposals only. Each should be a separate reversible change with paired
real-launch tests. Estimated ranges below include **zero** as the defensible lower
bound; maxima are investigation budgets, not promised improvements.

| Order | Measured target / owner | Smallest proposal | Possible saving; confidence | Complexity and risk | Required validation |
| --- | --- | --- | --- | --- | --- |
| 1 | Texture/sprite/model CPU cluster; Minecraft/Sodium/resource integration | First add resource-job and critical-path timing in a disposable build, then test elimination/reuse of repeated pixel scans or mipmap work for identical reload inputs. Candidate methods: `TextureUtil.solidify`, `SpriteContents.scanSpriteContents`, `MipmapGenerator`, `ImprovedItemModelBuilder`. Prefer compatible upstream fixes. | 0–2 s exploratory budget; strong hotspot evidence, low saving confidence | High. Alpha edges, animations, connected textures, resource reload, memory, OpenGL/Sodium compatibility. No blanket disabling of visual work. | Paired original-pack launches, resource reload and visual comparisons, animated/paletted textures, Sodium/AMD plus other supported graphics paths; prove removed work lies on critical path. |
| 2 | Four mod scans, 614 ms inclusive; Launcher | Return the freshly validated normalized mod inventory through the existing locked validation callback and reuse it for the immediately following compatibility check under the same content lock. Start with one redundant scan; retain fresh authoritative validation and both game-integrity audits. Source: `launch/boundary.rs`, `instances/lifecycle.rs`, `application.rs` near 5765, `instance_mods.rs:212`. | 0–0.15/0.20 s for one scan; medium-high diagnosis confidence | Medium. Never reuse across requests/restarts or bypass final mutation detection. Frontend scan state cannot authorize Play. | Race/mutation tests, incompatible and damaged mods, unknown state, unchanged lock ordering, P2 audit count/hash coverage, paired Play preparation timings. |
| 3 | Session chain 1.602 s and independent preparation; Launcher | Evaluate starting ordinary session restoration concurrently with independent plan/runtime preparation after validating selected account/instance identity. Await every validation and session result before final locked spawn. Existing session caching already works; adding another cache is not the proposal. Source: `application.rs:5589`, `auth/flow.rs:281`. | 0–1.0 s fresh-Launcher critical-path budget; medium-low until measured | Medium/high. Cancellation, account changes, refresh rotation, single restoration gate, failure reporting and session lifetime must remain correct. | Fresh and cached sessions, expiration/revocation, account switch/sign-out races, failed runtime/instance validation, no token exposure; randomized paired controls. |
| 4 | 2.183 s inclusive mixin work plus class loading; Fabric/third-party/Aurora mixins | Identify transforms on the critical thread by class/mod and remove only proven redundant Aurora-owned work or adopt a demonstrated compatible upstream improvement. Begin with per-class aggregate timing, not another generalized cache. | 0–0.5 s preliminary budget; low optimization confidence | High. Transformation order and compatibility; no skipping required mixins or weakening classpath integrity. | Same full modpack, class/mixin correctness, startup and world tests, no duplicate ASM, exact input identities. |
| 5 | 208 ms Aurora initializer; Aurora Client | Add subphase markers around config/profile/registration; consider only nonessential eager allocations whose ordering can be preserved. Source: `AuroraClient.java:26`. | 0–0.10 s plausible target; absolute callback envelope about 0.21 s | Medium. Active profile, migrations, HUD/keybind registration and gameplay state must remain ready. | Client tests, default/migrated profiles, immediate settings/world entry, same menu criterion, per-subphase measurement. |
| 6 | First-menu blur/readback samples; Aurora UI | Time existing glass/readback subphases for the first two usable frames; investigate safe data reuse within a frame before proposing a different upload path. Source: `AuroraTitleScreen.java:89`, `BlurPanelRenderer.java:1444`. | 0–0.10 s tentative; no exclusive timer yet | Medium/high graphics risk. Preserve pixel-buffer hygiene, visual design, AMD/OpenGL/Sodium and supported renderer behavior. | First-frame visual QA and native diagnostics across graphics paths; no Vulkan-specific regression; paired menu-frame timings. |

The first concrete Launcher implementation candidate is order 2 because its
duplicate work and trust boundary are visible in source and direct timing. Order
1 deserves the largest research effort because it owns the largest sampled CPU
cluster. Do not add all row maxima: some overlap, some shift work, and none has a
measured implementation result. Do not remove mods to advertise an improvement.
The 189 ms GC-pause envelope does not support a large GC-tuning project here.

## Verification, privacy and limitations

Completed checks in the disposable source/build tree:

* Frontend type checking: zero errors/warnings; tests: 252 passed; production
  frontend build passed.
* Rust formatting and separate format check, all-target feature check, native
  release feature build (`--no-bundle`): passed. Rust tests: 846 passed, 29 ignored;
  six icon integration tests passed. The ignored tests are not claimed as passing.
* Real application boot and real game execution: 16 successful menu/normal-exit
  trials; three setup failures retained with reasons.
* Exact JNA isolation: three positive/negative controls and per-launch real-path
  assertions passed. Agent behavior/privacy fixture passed. Five analyzer tests
  cover invalid/missing/duplicate markers, error markers, overlapping intervals
  and verified injected-method ownership.
* Successful JFR recordings parse and contain no unexpected enabled event types;
  native traces report zero drops. The five core summary outputs were recomputed
  twice and compared byte-for-byte. Source/artifact hashes and final full-file
  preservation were verified. Original tracked/untracked files and critical
  manifests/lockfiles remain present; no original untracked file was staged.

Early build attempts exposed private-copy source-contract context and fake-child
test assumptions. The private Git index and test-only absent-research-environment
no-op fixed the harness; successful final reruns supersede failures. Logs and
their hashes retain the attempts. No Client build/test was run in P3 because
Client source/artifact was unchanged. Prior P0.2's build reproduction and Client
tests are prior evidence, not newly executed P3 checks.

Raw recordings retain private paths, thread names and runtime details. They remain
local; exact paths and SHA-256 are in the private root's `raw-index.json`, reached
through ignored `docs/performance-p3/private/research-root.txt`. Its current digest
and recording count are in the public verification receipt. No raw JFR, log,
authentication argument, token, process identifier, account identifier, server
address, owner name or private pack filename is committed. The reducer exports
code symbols, numeric summaries and public artifact hashes only. Declaring-class
ownership is approximate for remaining mixin-injected code; verified Sodium scan
and explicit injector labels are exceptions, not a complete provenance map. Privacy scanning
is a targeted export check, not a universal proof of secret absence.

Limitations: one machine/OS/runtime/modpack; warm file caches; fresh Launcher
session restoration; small, partly interrupted control cohort; common agent
overhead unmeasured; 20 ms sampling and 10 ms thresholds; no exact exclusive DFU,
resource-reload, GPU or per-mod mixin timer; no cold cohort or optimization variant;
no claim that menu readiness means every background task has completed. Official
Mojang mappings were SHA-1 verified, and ambiguous method mappings remain
intermediary. CPU samples and inclusive durations cannot prove causal savings.

## Commits, rollback and next phase

Instrumentation/reproduction tools and the report/evidence are independent local
commits. The report commit is discoverable with `git log --oneline --
PERFORMANCE_RESEARCH_P3.md`; instrumentation is the preceding P3 tools commit.
The instrumentation commit is `714a067` (`research: add isolated Minecraft startup
profiling tools`), with its full identity in `evidence/commits.json`. No production
files were changed, so rollback is reverting the report
commit, then the tools commit if desired. Preserve private evidence first; no
automatic deletion of the research root or original data is part of rollback.

Recommended P4: review the narrow locked-inventory reuse proposal and the resource
critical-path instrumentation proposal. Implement one independently revertible
change at a time, repeat randomized profiling-disabled controls, retain the full
modpack and all security/validation guarantees, and accept only gains larger than
the measured noise. P3 stops here for owner review.
