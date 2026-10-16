# JVM launch audit — Performance P1

Audited 2026-10-16 at `8983f938c4792fabbd314a854d612d2eb51f7f46`, branch
`codex/client-3-production-authority`. This JVM investigation is distinct from
the earlier startup-readiness P1 already in this directory. Historical tools,
README and evidence are retained. No launch behavior changes accompany this audit.
AGENTS.md and ARCHITECTURE.md were read; current implementation takes precedence
over historical sections. UI-1–5 remain owner-approved; the owner's confirmation
closes UI-4 native verification. This phase does not reopen them.

## Settings to spawn

Source links are relative to this document; line numbers refer to the audit HEAD.

| Stage | Actual authority and behavior | Source |
| --- | --- | --- |
| Global Minecraft & Java | Shows saved selected-instance MiB; links to instance Settings/Overview. No global JVM, memory, or Java override | [MinecraftSettings.svelte](../../src/lib/launcher/MinecraftSettings.svelte), [UI-5 audit](../ui-5/SETTINGS_AUDIT.md) |
| Instance editor | Per-instance draft, whole-MiB input, additional JVM text, optional window; explicit Save. Changing Java is not a picker | [InstanceSettingsPanel.svelte](../../src/lib/instances/InstanceSettingsPanel.svelte), lines 203–247 |
| Frontend save | `detailDrafts` preserve edits across navigation. `runSaveConfiguration` submits whole proposed configuration and original expected configuration, then publishes native result and refreshes readiness | [store.svelte.ts](../../src/lib/launcher/store.svelte.ts), lines 565–625 |
| Typed boundary | `InstanceConfiguration` uses `memoryMib`, `additionalJvmArguments`, tagged loader, Minecraft version, `auroraEnabled`, `window`. `updateInstanceConfiguration` sends original and proposed values | [backend.ts](../../src/lib/backend.ts), lines 568, 689; [application.rs](../../src-tauri/src/application.rs), line 2334 |
| Save validation | Validates configuration and implemented platform; requires Ready registry record; refuses stale original and unreviewed Aurora association changes; atomically saves registry | [instances/lifecycle.rs](../../src-tauri/src/instances/lifecycle.rs), line 914 |
| Play preparation | Rust preparation reservation; capture snapshot; deep content check; exact game/runtime plan resolution; validate saved configuration and parse JVM text | [application.rs](../../src-tauri/src/application.rs), lines 5786–5895 |
| Runtime/session | Deep managed runtime validation with actual bounded `-version`; use its exact launch executable. Usable Rust session or native restoration required | same file, lines 5896–5960; [runtime/install.rs](../../src-tauri/src/runtime/install.rs), `validate_runtime` |
| Final authority | `prepared.with_validated` rechecks snapshot and content under final locks; local mod compatibility repeated; resolves LaunchSpec and spawns inside the boundary | same file, lines 5960 onward; [launch/boundary.rs](../../src-tauri/src/launch/boundary.rs) |
| Assembly | `LaunchPlan::from_game_plan` consumes normalized effective arguments; role/containment-checked installed paths and Rust-owned session replace placeholders | [launch/resolve.rs](../../src-tauri/src/launch/resolve.rs), lines 282, 325, 512 |
| Spawn | Structured executable and individual strings, instance-root cwd, null stdin, piped output; exact returned child supervised | [launch/process.rs](../../src-tauri/src/launch/process.rs), lines 172, 240 |

Launcher configuration is **schema 7**, with selection, appearance, widgets and
Discord preferences; it has no heap or JVM preference. Instance registry is
**schema 5**, with desired configuration on each record. Defaults: 2048 MiB,
empty additional arguments, no custom window. Memory is a `u32`, inclusive
512–32768 MiB. It is a product bound, not a RAM safety calculation. Memory/JVM/
window saves affect the next launch without reinstalling game files. Minecraft
and loader identity changes can require explicit installation. Existing known
migrations, malformed-state refusal, revision checks and atomic writes stay intact.

## Java identity and validation

`instances::lifecycle::resolve_instance_launch_plans` (line 1347) resolves the
installed concrete Minecraft/platform pin. The composed plan supplies Java
component and major; Aurora's repeated major is a compatibility assertion.
Fabric's declared major floor is recorded by composition; it cannot cause a
silent switch to an unrelated runtime component. Vanilla/Forge/NeoForge retain
their implemented composition semantics; P1's gameplay matrix focuses on Fabric.

[runtime/metadata.rs](../../src-tauri/src/runtime/metadata.rs), lines 126–225,
uses the pinned Mojang runtime index, exact platform/component and exactly one
entry. Missing/empty/ambiguous selections fail. Index version must match the
required major. Its file manifest is acquired through the SHA-1 verified cache,
then normalized into JavaRuntimePlan. No system Java, PATH, JAVA_HOME, vendor
picker, custom executable, or architecture fallback participates.

[runtime/plan.rs](../../src-tauri/src/runtime/plan.rs), lines 60, 93, 255, 332,
maps OS/architecture explicitly and derives
`runtimes/<component>/<platform>-<manifest-sha1>/`. Windows uses `bin/javaw.exe`
for Minecraft and `bin/java.exe` for diagnostics; Linux uses `bin/java` for both.
Linux x64/x86 map to `linux`/`linux-i386`; Linux ARM64 is unsupported by the
current mapping. Windows x64/ARM64/x86 have separate keys. No Java is copied into
an instance or installed into the system.

[runtime/install.rs](../../src-tauri/src/runtime/install.rs), `ensure_runtime`,
`validate_runtime`, `run_java_diagnostic`, and `run_process`, installs only
verified SHA-1 store objects into derived staging, writes schema-1
`runtime-installed.json` last, and promotes the whole tree. Existing valid exact
runtimes are reused. Exact-target kernel/process coordination protects installation;
unknown/malformed/future state is not replaceable ownership. Validation matches
the full plan/state, files/sizes/SHA-1, directories, links and executable semantics.
Play additionally runs an absolute executable with `-version`, ten-second timeout,
bounded supervised output and major check. Version validation is **not** a
collector/flag support test or a full vendor/build attestation.

For Minecraft 1.21.11 the checked-in Mojang fixture specifies
`java-runtime-delta` / Java 21. Runtime feeds can change within that component;
major 21 does not identify the exact vendor/update/build. Record manifest SHA-1,
runtime state version, vendor, complete VM build string and executable SHA-1 for
each future run. This session observed Microsoft `21.0.7.0` file-version metadata
on two already-running Java executables and `JAVA_VERSION="21.0.7"` in their
release files. No complete `+build`, runtime manifest identity or benchmark
runtime validation was established; do not substitute a historical Java build.

## Final effective argument order

The production call constructs `LaunchOptions::new(configuration.memory_mib(),
parsed_arguments, window)`. `LaunchOptions::default()` omits memory only for
other native callers/tests; it is not the normal Play default.

1. One launcher-generated `-Xmx<memoryMib>m`.
2. Applicable Mojang JVM groups, original order, substituted individually.
3. Loader JVM contributions, if present (Forge/NeoForge). Fabric contributes none.
4. Mojang logging argument, with the validated installed XML path.
5. Parsed additional JVM arguments, in user order, revalidated at assembly.
6. Final main class.
7. Applicable Mojang then loader game arguments, with real native session/path
   substitutions; optional quick target augments this same spec.

Evidence: [game/plan.rs](../../src-tauri/src/game/plan.rs), lines 336, 354;
[launch/resolve.rs](../../src-tauri/src/launch/resolve.rs), lines 169, 512–558.
There is no shell command string. Full process arguments are sensitive and must
never be recorded for this audit or benchmark reporting.

For the **checked-in 1.21.11 fixture**, Windows x64/Fabric, default memory and
empty custom text, the JVM portion is the following ordered argument vector.
Angle-bracket paths are redacted labels; this is source-derived assembly,
not a captured live command or a claim about the owner's saved settings:

```text
-Xmx2048m
-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump
-Djava.library.path=<validated natives root>
-Djna.tmpdir=<validated natives root>
-Dorg.lwjgl.system.SharedLibraryExtractPath=<validated natives root>
-Dio.netty.native.workdir=<validated natives root>
-Dminecraft.launcher.brand=aurora-launcher
-Dminecraft.launcher.version=1.4.2
-cp
<validated ordered libraries; Minecraft client last>
-Dlog4j.configurationFile=<validated installed client-1.21.2.xml>
```

Source: [1.21.11 Mojang fixture](../../src-tauri/src/forge/fixtures/1.21.11-mojang.json),
plus the resolver tests. Linux x64 omits the Windows HeapDumpPath entry, uses `:`
as classpath separator, Unix paths, and `bin/java`. x86 metadata additionally
selects `-Xss1M`. Other Minecraft versions contribute their own official arguments;
do not hardcode this fixture as a universal runtime command.

Aurora adds **no `-Xms`, collector, pause target, worker-count, pre-touch, young-
generation, region-size or compiler tuning flag by default**. Its initial heap
and collector follow the selected runtime's ergonomics unless additional arguments
change them. Actual VM flags/collector still need measurement.

## Custom parsing, precedence and limits

[instances/settings.rs](../../src-tauri/src/instances/settings.rs), lines 483–605,
splits spaces outside double quotes, preserves grouped text, and applies even/odd
backslash-before-quote rules. Single quotes do not group. Shell expansions and
shell escaping do not apply. Empty `""` becomes a real empty argument. Control
characters are rejected before parsing, **including tabs/newlines**, despite the
parser's tab-separator branch and introductory whitespace wording. Text is limited
to 8192 bytes, at most 64 tokens, each at most 4096 bytes. Unterminated quotes fail.
The assembly boundary re-quotes/reparses for validation, then emits the original
parsed strings; it does not use a shell parser.

Heap prefixes rejected: `-Xmx`, `-Xms`, `-Xmn`, `-XX:MaxHeapSize`,
`-XX:InitialHeapSize`, `--max-heap-size`, `--initial-heap-size`.
Exact tokens rejected: `-cp`, `-classpath`, `--class-path`, `-jar`.
Ordinary direct heap override attempts fail at Save and launch; use `memoryMib`
for the maximum. There is no supported per-instance initial-heap setting.

**This is a lexical conflict filter, not a complete JVM policy.** It does not
deduplicate arbitrary `-D` properties or `-XX` flags, validate collector combinations,
test vendor support, or reject every equivalent spelling/indirection. For example,
`--class-path=...`, `@...`, `-XX:Flags=...`, `-Djava.library.path=...` and
`-Dlog4j.configurationFile=...` are not matched by this filter. Custom non-heap
properties emitted last can supersede generated properties; conflicting collector
switches can make Java refuse startup rather than resolve to a useful winner.
Indirection can undermine the intended heap/classpath ownership. Do not advertise
the current filter as proving all custom flags safe or all duplicates impossible.
No such bypass was executed during P1. A comprehensive policy is an outstanding
design decision, not a behavior change in this audit.

Safe *experimental scope* today: explicit per-instance maximum heap within existing
bounds; one supported collector/pause-target change at a time; optional benchmark
logging to a disposable relative path. Syntax acceptance alone proves neither
compatibility nor a performance benefit. `-Xms` candidates need a separately reviewed
isolated laboratory path; do not weaken the production parser to run them.

## Environment, supervision and diagnostics

Minecraft inherits the normal host environment for graphics/audio. The spawn
explicitly removes `CLASSPATH`, `JAVA_TOOL_OPTIONS`, `_JAVA_OPTIONS`,
`JDK_JAVA_OPTIONS` and inherited activity-bridge bootstrap fields; an eligible
bridge injects fresh fields through the child Command only. PATH/JAVA_HOME can be
inherited but never choose the absolute managed executable. Diagnostic Java has
an empty environment except Windows SYSTEMROOT/WINDIR/TEMP/TMP, or Unix LANG/LC_ALL=C.

The isolated instance root is both cwd and game directory. Classpath/assets/natives/
logging come from role-checked managed game state. User mods load through the loader,
not a launcher classpath scan. Unknown placeholders fail. The native spec is
non-serializable and sensitive values redact Debug. Account access tokens and bridge
capabilities stay Rust-only. Custom values are not a place to put credentials;
ordinary configuration is non-secret storage.

Supervision reserves preparation/Starting/Running process-locally per instance,
awaits the exact returned child, and reports sanitized exit/failure without automatic
repair/relaunch. Different instances can run together; no cross-process gameplay
exclusion is claimed. Streams are drained with bounded initialization/tail retention
(512 KiB each), then exact known secrets are scrubbed before a reserved instance-local
launch log is written. This is not a continuous frame/GC/CPU profiler, nor a general
privacy sanitizer for arbitrary mod output.

Existing feature-gated `performance-p0-2` probes measure launcher preparation,
metadata, integrity, Java diagnostic and spawn-call spans. `Running` means spawn
succeeded, not main menu. Installation diagnostics measure installation stages.
The authenticated activity bridge can identify menu/world transitions, but not frame
presentation, heap pressure or GC pauses. Earlier player/browser benchmarks measure
launcher UI rendering; they are unsuitable evidence for Minecraft gameplay tuning.

## Hardware awareness

Existing launcher runtime inputs are normalized OS, architecture, game Java
component/major and per-instance configured memory. Native status exposes platform;
there is no existing RAM/free-memory, physical/logical CPU topology, GPU, shared-VRAM,
battery/thermal or desktop/laptop recommendation service. Source searches found no
system-memory/topology detector driving memory or GC choices. Some existing bounded
I/O worker constants and launcher rendering probes are unrelated to JVM heuristics.

Read-only workstation inventory used built-in Windows CIM/powercfg, outside the
product. See [BASELINE_RESULTS.md](BASELINE_RESULTS.md) for observed values. No
hardware dependency or production hardware DTO was added.

Future RAM advice should combine workload live-set evidence and transient free RAM
with explicit reserves for OS, launcher/background apps, native/off-heap Java,
graphics/shared-memory pressure, and workload bursts. A product ceiling of 32768 MiB
does not make that allocation sensible on this 32 GiB machine. Choose the smallest
tested sufficient heap; leave deliberate per-instance overrides. Report uncertainty
and pressure, never use a universal percentage or silently clamp existing settings.

Logical thread count is not topology, especially on this hybrid CPU. Physical cores,
P/E distribution, SMT, affinity/container limits and power state can explain results;
they do not by themselves justify forcing GC workers. Keep JVM heuristics unless
matched measurements support an override. Reliable GPU/topology/power detection is
a proposed addition requiring scope/dependency review; missing information should
remain unknown, with manual benchmark inventory accepted.
