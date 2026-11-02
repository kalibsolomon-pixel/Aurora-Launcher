# Launcher overhead â€” retain the event-driven implementation

2026-10-19, Windows 11, current embedded production frontend with **unoptimized
debug native build** (`tauri/custom-protocol`), one isolated Fabric test instance,
saved account/skin restored through the native flow, Discord disabled. Home is
visible at the top, no interaction/screenshots/builds/exports during each window.
Both captures follow >=60s settled home state, in separate normal Launcher
processes. The [protocol](LAUNCHER_INTEGRATION_PERFORMANCE_PROTOCOL.md) preceded
collection. There is no new production-source optimization or before/after gain.

| Observation, 60s each | Process family | CPU seconds | CPU % of one logical core | CPU % of 28-thread host | Summed RSS MiB range | Host busy % |
| --- | ---: | ---: | ---: | ---: | --- | ---: |
| First settled home | 8 | 0.046875 | 0.0781 | 0.00279 | 502.199â€“504.148 | 2.358 |
| Restarted settled home | 8 | 0.015625 | 0.0260 | 0.00093 | 484.445â€“485.273 | 2.192 |

Each family contains the native process and its seven observed WebView/child
processes, with 121 samples per exact retained read-only handle. Endpoint family
membership is unchanged, no sample failures; transient between-endpoint children
could still be missed. CPU counters are quantized; near-zero is not free.
Maximum whole-family counter-read wall cost was 2.032/2.690 ms. The observer is
not calibrated away. Summed working sets can double-count shared pages and are
not unique physical memory, Java heap, private bytes or a memory-leak test.
Available RAM minima were 18728/18694 MiB. Ambient host activity is uncontrolled.

No material repeated idle CPU bottleneck is established. Do not remove required
validation, file hashing, session restoration or process supervision to improve
an idle number. This sample does not test a large instance/mod library, downloads,
Discord-enabled reconnect behavior, minimized windows, low-memory hardware or
long-duration growth. The seven-process WebView cost is real context, but two
working-set observations do not establish an attributable safe memory reduction.

## Source attribution and earlier work

The existing P1 startup-readiness deduplication is retained, not counted as a new
initiative gain. Its correctness tests keep mutation/invalidation checks fresh.
The current native boundary still validates actual content/runtime/session at
Play and uses structured arguments; UI readiness is advisory.

`PlayerPreview.svelte` redraws once for changed skin/angle/visibility and has no
idle animation loop. `InstalledArtwork.svelte` and `ResolutionCache` share bounded
in-flight/completed work; retry timers belong to transient/error results, not
constant successful-item scanning. Host memory facts have bounded cached reads.
Discord's disabled worker waits on messages (no deadline means a pending future);
enabled health/reconnect deadlines preserve required behavior. Native game
supervision awaits the exact spawned child, rather than polling all Java by name.
The 50-request research IPC interval is build-mode/explicit-option gated and is
not a production idle loop. Tests and actual idle measurements support retaining
these implementations. No speculative cache, polling or frontend rewrite.

## Startup and preparation limits

Two real app boots and normal restart were observed. Spawn-to-first-home upper
bounds were 41.646s and 20.957s; neither is time-to-Ready or an optimization gain.
Spawn/first-home timestamps
are retained, but they include observation delay and are upper bounds, not exact
startup latency or a release-build comparison. The first fixture lacked Fabric
API because development Gradle supplied that dependency outside `mods/`; native
readiness correctly blocked it. After adding the same cached API jar to the new
test root and explicitly rechecking, native Java/Play recovered. No validation
was weakened. On restart Home initially showed Needs attention; later native
configuration/readiness refresh restored Ready. Unattended first-Ready latency
and the initial transient's exact cause are not established. The successful
refresh is a test workaround, not evidence of instant readiness or an attributed
performance defect. Normal Play must still perform its independent final checks.

The UI saved 1536 MiB/Balanced, reopened after restart with that exact value,
rejected a conflicting custom Xmx without modifying persistence or losing draft
text, then saved 2048/empty custom text and returned Ready. Failed copy preparation
at a Windows long path was preserved; a fresh shorter Temp destination completed
without an OS setting change. No original test tree was reused for mutations.

[Machine-readable measurements](../../performance-initiative/launcher-resources.json)
include binary hash, windows, identities, counts and limitations. Raw family CSVs,
events, screenshots, separate stdout/stderr, preparation records and installed
test data stay in the private integration Temp root referenced by
`.git/performance-initiative/integration-root.json`. Account metadata, worlds,
native launch logs and browser data are never public report artifacts.
