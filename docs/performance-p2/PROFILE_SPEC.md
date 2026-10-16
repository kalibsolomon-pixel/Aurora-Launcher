# Performance modes — P2 specification

P1 gameplay baselines remain **NOT MEASURED**. These modes select configuration
advice, not an FPS/latency/startup guarantee. All launch authority remains Rust-owned.

| Mode | Advisory target | JVM tuning |
| --- | --- | --- |
| Efficient | At most 1536 MiB, reduced by the safe host budget | None; no CPU throttling |
| Balanced | At most 2048 MiB, reduced by the safe host budget | JVM default ergonomics |
| Performance | Same as Balanced until demanding-workload evidence exists | No speculative collector/pause/compiler switches |
| Custom | Manual supported maximum/arguments; 2048 MiB advice is optional | Portable supported properties/assertions only |
| Existing Configuration | Compatibility state for missing/null mode | Exact established argument policy retained |

The four modes are explicit persisted choices; Existing Configuration is the
compatibility state, not an automatically activated preset. Fresh instances also
start in Existing Configuration with the existing 2048 MiB default, until opted in.
Changing mode preserves memoryMib and additionalJvmArguments byte-for-byte. Explicit
Use recommendation modifies the existing draft maximum; only Save persists it.
Manual maximum remains available in every mode. No duplicate heap/argument store.

## Authority, migration and precedence

Instance registry **schema 6** adds optional performanceMode. Schema 5/4 migrate
without activating a mode or changing any existing configuration field; existing
2/3 migrations still preserve their documented settings and remain mode-free.
Legacy documents already containing performanceMode fail deliberately. Unknown
fields, modes, versions and malformed documents fail, never overwrite/reset. No
config-7/game/runtime-state migration. Older launcher versions that cannot read
schema 6 require an explicit reviewed downgrade procedure; do not point an older
production launcher at the development root. Production data is not migrated by
these acceptance runs.

Existing atomic registry writes, revision conflicts, full expected-original Save,
instance IDs and installation-affecting comparisons remain authoritative. A mode
or memory change is launch-only. Save validates the whole proposed configuration;
launch repeats it and uses validated_jvm_arguments. No separate command builder.

Effective order is unchanged: generated Xmx, applicable Mojang JVM groups, loader
contributions (none for Fabric), Mojang logging, custom arguments in original order,
main class, game arguments. Profiles emit **no JVM flags**. Only explicit memory
edits change the one generated Xmx. Xms remains absent/unsupported. The exact game
plan continues selecting managed Java/component/architecture; profiles do not
replace it, force Java 21 onto other Minecraft versions, or select another vendor.

Opted-in modes accept `-Dname[=value]` with conservative ASCII property keys and
standard assertion switches (-ea/-da/-esa/-dsa and their long spellings). Managed
Java/JDK/sun properties and Minecraft native/logging/brand/version properties are
reserved. Unsupported VM switches, agents, argument/Flags files, classpath/entrypoint
and heap overrides reject Save with an explanation. No arbitrary user option is
executed to probe support. Java 21 is the primary target; these standard supported
options require no vendor-specific VM tuning assumption.
Property support covers JVM syntax, not every mod-specific property's interpretation;
advanced properties remain explicit user configuration, not a sandbox for mod behavior.

Identical repeated properties/assertions are retained. Differing system properties
remain last-wins, in original order; native advice includes generic notices without
echoing values. Multiple enabled collector selections are classified as mutually
exclusive. A single collector or pause target is still **unsupported/BLOCKED**
pending exact-build validation, so no unverified last-wins GC setting is applied.
The legacy parser stays unchanged; this stronger supported vocabulary is opt-in.
Existing configurations can retain their old custom switches without silent edits.
Selecting a mode with unsupported text fails safely and keeps that text for review.

## Memory heuristic and hardware

Windows uses GlobalMemoryStatusEx through the existing windows-sys package (one
additional SystemInformation feature, no new package); Linux reads at most 8192
bytes of /proc/meminfo for MemTotal/MemAvailable. Other/unavailable reports stay
unknown. Architecture comes from the host; exact managed-runtime mapping already
prohibits architecture substitution. No GPU/model/process/topology scan, mod-count
inference, power change, polling, startup loop, affinity/priority or telemetry.

Request only when Performance mounts, mode changes, saved argument text changes,
or Use recommendation explicitly refreshes. Facts are cached process-locally for
30 seconds. Reject unknown, inconsistent (available > total), zero, stale (>30 s)
or unknown-architecture reports. Using saturating arithmetic:

```text
budget = min(total MiB - 4096, available MiB - 2048, architecture ceiling)
ceiling = 1024 MiB on x86; 32768 on x86_64/aarch64; 0 if unknown
recommendation = min(mode target, budget), only if budget >= 512 MiB
```

Total reserve covers OS/background plus native Java/shared-GPU uncertainty;
available reserve covers additional native/shared-memory demand beyond currently
resident OS/apps. This is a conservative heuristic, not a benchmark-validated
live-set estimate. Unknown GPU and workload demand and point-in-time availability
are disclosed. 8/16/32/64 GiB and larger reports use the same explicit reserves,
not a universal percentage. Low pressure headroom can reduce or suppress advice;
the 512 minimum is an existing product bound, not proof a demanding pack fits.
Never clamp/rewrite a saved manual heap. No recommendation guarantees future RAM
availability or game compatibility. User workload knowledge can require Custom.

The read-only get_instance_performance_advice command accepts a registered instance
ID and mode. It resolves paths natively and returns facts, observation age, advice,
unknowns and generic duplicate-property notices. It never resolves/downloads Java
or returns a LaunchSpec/session/game argument vector. A refresh that changes advice
requires review/re-click; late responses cannot overwrite a newer manual heap or
an instance/mode switch, including A→B→A generations.

## Review, reversal and benchmark readiness

Global Minecraft & Java links into the one existing instance Settings Performance
area. Saved mode/heap, unsaved mode/draft, recommendation and raw custom arguments
are separately labeled. Selecting a mode replaces no effective arguments, so no
destructive Custom transition exists. Use recommendation names its concrete value;
Undo recommendation/Discard reverse it before Save. Switching back to Existing
changes policy only; manually restore a prior heap explicitly after Save. No hidden
argument backup/history store or auto rollback is implied.

Failed Save retains the saved configuration and editable draft/error. Editor
initialization depends on the instance ID, so a fresh authoritative registry object
cannot erase a save error. Existing UI-5 navigation/preferences/tokens remain shared.

For future benchmarks, record saved mode, memoryMib (Xmx), ergonomic initial heap
(no configured Xms), raw parsed supported JVM-only arguments, ordered normalized
Mojang/loader/logging contributions with private paths replaced by semantic labels,
validated runtime full version/build/manifest/executable identity, and advice facts/
age. Reuse P1's private run manifest/provenance protocol. Do not export live command
arrays, tokens, account IDs, bridge fields or absolute personal paths. Runtime status
remains the existing native Overview readout. No permanent logging/export telemetry.
