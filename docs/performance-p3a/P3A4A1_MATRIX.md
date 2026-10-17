# P3-A.4a.1 — native/Aurora diagnostic matrix and interval analysis

**PARTIAL. Two attempted controls; five B captures deferred. No comparison winner.**
Fresh 60-second idle preflight passed, but post-capture counter review estimated
background activity 2.259976% (C1) and 4.771219% (C2), above the 2% quiet screen.
Applying the owner's background-interference stop rule conservatively, no further
captures were taken and no favorable retry was attempted. C1's offline VALID WITH
WARNINGS result remains preserved but phase quality is INCOMPLETE/confounded;
C2 additionally reports 28,259 consumer-overflowed events and is INVALID. Thus native-versus-Aurora
timing/accuracy comparison is deferred, not inferred from implementation or WGL.
The table shows retained ETW-channel observations. INVALID rows are retained for
failure diagnosis only; their rates/tails do not represent complete throughput or
proven Minecraft stalls. VALID WITH WARNINGS is offline consistency, not independent
Minecraft completeness. No independent Minecraft frame source was available.

| ID | Condition | Ideal ms | ETW interval rate Hz | Median ms | P95 ms | P99 ms | Max ms | Raw display NA | Quality |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| C1 | A native 240 | 4.166667 | 231.335 | 4.1943 | 5.0951 | 5.6087 | 7.7641 | 1.811% | INCOMPLETE: background above quiet-system screen |
| C2 | A native Unlimited | — | 1828.086 | 0.2528 | 0.5981 | 0.8176 | 494.5272 | 92.984% | INVALID: consumer overflow; background confounded |
| A1 | B Aurora OFF/native Unlimited | — | — | — | — | — | — | — | DEFERRED: background interference |
| A2 | B Aurora 240 | 4.166667 | — | — | — | — | — | — | DEFERRED: background interference |
| A3 | B Aurora 500 | 2.000000 | — | — | — | — | — | — | DEFERRED: background interference |
| A4 | B Aurora 750 | 1.333333 | — | — | — | — | — | — | DEFERRED: background interference |
| A5 | B Aurora 1000 | 1.000000 | — | — | — | — | — | — | DEFERRED: background interference |

Full row/inner counts, count-per-fixed-window rate, high tails, interval variance,
signed target-error extrema, shorter/longer counts, fraction within 5% of target,
available display distributions, every CSV field and unavailable/zero counts,
presentation modes/streams, all retained loss counters, CPU/RSS/read overhead,
UTC/QPC/creation identity and raw hashes:
[minecraft.json](evidence/p3a4a1/minecraft.json),
[interval details](evidence/p3a4a1/interval-details.json).
The interval-derived rate uses only wholly inner adjacent intervals; raw and
fixed-window row counts are separate and can differ at boundaries/gaps.

## Conditions and provenance

Both attempted captures use exact verified PresentMon 2.3.1 default full tracking (input
tracking disabled), default present buffer 2048, 16 seconds, inner 4–14 seconds.
The separate 8192 WGL diagnostic does not silently change this matrix. Managed
Java 21.0.7+6-LTS, memory 2048 MiB, empty additional JVM text, same effective default
JVM vector. A launcher mode Balanced; B Existing Configuration/null mode; no
preset comparison or tuning. Native process/path/hash and exact child creation
were verified; all captures record foreground observations and normal producer exit.
Loss snapshots cover their query times, not an unobserved shutdown tail.

A/B copies were inventory-identical to frozen A restoration before launch.
Seed 123456789, flat plains, position(.5,66,.5), yaw0/pitch10, noon, clear weather,
creative/normal, no entities/block entities, generation/features and gamerules,
12/8 distances, 1920×1080 windowed, VSync OFF, Minimized inactivity, native Unlimited
except C1=240. Menu mouse movement was corrected through ordinary in-game camera
reset before capture; F3 disabled and messages allowed to fade. Initial world
warm-up ≥300 seconds. No five-repetition benchmarks or JVM/collector/heap sweep.

B retains Aurora 3.0.0/Fabric API 0.141.6+1.21.11/Sodium 0.8.14+mc1.21.11/
Lithium 0.21.4+mc1.21.11, MC1.21.11/Fabric 0.19.5. Exact jar hashes and world archive/
inventory/options provenance are in [scene provenance](evidence/p3a4a1/scene-provenance.json).
Main menu and world execution succeeded. B default Aurora HUD, renderer and
simulation optimizations differ from minimal A; they remain constant within B.
After exit, every existing canonical Minecraft option value still matches in B;
the only added option keys are 18 Aurora key bindings. Sodium's actual default
render settings and hashes of its options/mixin configuration and Lithium's
properties are retained in [execution closure](evidence/p3a4a1/execution-closure.json).
No external shader-pack loader in verified inventory; Aurora's built-in shaders
remain part of its workload. Resource pack/config differences are retained rather
than claiming B has A's empty-mod semantics. Post-exit common NBT inspection never
uses the A-only empty-mod attestation for B. This is not a canonical B lock.

Actual per-condition configuration is retained: smooth PARK, spin 500 µs, park 2000 µs,
lowCPU false, lowLatency true, adaptive false, disableVSync false, GUI 60. Native
Unlimited sentinel 260 avoids a lower ordinary-world native cap. Only global cap
changes among A2–A5. GUI cap and inactive throttles are not the requested global
cap. B caps were not captured; only 1000 was entered through the UI for non-capture
persistence verification. Configuration restart and final OFF restoration are recorded in
the execution closure evidence, separately from installed defaults.

## Interpretation and limitations

Observed ETW interval error is not independently measured limiter error. Scheduling,
render work, downstream timestamps, consumer loss and observer cost can contribute.
No sustained inability to render a target may be attributed to PARK from this
short matrix alone. Missing display estimates are not dropped-frame proof.
No every-gap-is-GC/stall claim, optical FPS inference or A-versus-B absolute
throughput comparison. GPU fields remain diagnostic ETW attribution.

Lower caps can make records more regular, but reaching a cap censors maximum
throughput and can hide future JVM differences. Neither clean-looking records nor
the largest cap supplies completeness, overhead or sensitivity evidence. Controlled
observer gate failed, so no Minecraft observer-on/off trials. No primary cap.
See [completeness correction](P3A4A1_COMPLETENESS.md),
[observer recovery](P3A4A1_OVERHEAD_RECOVERY.md) and
[metric/prerequisite decision](P3A4A1_METRIC_DECISION.md).
