# P2 architecture decision

Use one optional `performanceMode` enum inside the existing InstanceConfiguration:
efficient, balanced, performance, custom. Missing/null means Existing Configuration,
an explicit compatibility state, not an automatically selected mode. Registry moves
from 5 to 6; known legacy schemas retain all settings and acquire no active mode.
Old-schema documents containing the new field are malformed rather than speculative
migrations. Unknown fields/versions remain deliberate errors. Config 7, game/runtime
state and release authority do not change.

A schema field is necessary because mode selection must survive restart even when
Balanced and Performance intentionally produce the same advice. Deriving mode from
heap/flags would mislabel explicit custom/legacy choices. No separate file, heap
copy, command builder or saved alternate argument vector is introduced.

Mode changes alone change no JVM argument. Each mode selects a conservative native
memory recommendation. Explicit Use recommendation changes the existing draft's
memoryMib; atomic Save remains the only persistence operation. Manual maximum and
raw arguments remain accessible, and switching mode never clears/reorders text.
Discard and session-local Undo recommendation retain a reversible route. Since no
mode transition replaces arguments, no destructive Custom-to-preset confirmation is
needed. Invalid new-mode arguments reject Save with a concrete explanation.

Legacy configurations retain the old parser/launch behavior. Opted-in modes accept
a deliberately limited standard JVM vocabulary (user system properties and assertion
switches); launcher-owned paths/properties, heap/entrypoint/indirection and unsupported
VM switches fail. Duplicate properties retain last-wins order with advice; exact
duplicates remain intact; mutually exclusive collectors fail. Collector/pause tuning
is not exposed as supported until exact-runtime validation exists. Minimum heap is
BLOCKED by the established heap policy. No arbitrary-argument diagnostic execution.

On-demand bounded hardware detection uses existing Windows bindings or Linux
/proc/meminfo, no new package. Cache for 30 seconds, refresh only on a settings
request, report age/unknowns. No startup poll, network, GPU scan, process scan,
affinity, priority or power changes. Reserve fixed OS/background/native/shared-GPU
headroom; cap the advisory target by available and total RAM and architecture.
Unknown/invalid/stale facts provide no recommendation. Never clamp saved settings.

Reuse the instance Settings Performance area, reached from global Minecraft & Java;
there is no duplicate global editor. A narrow read-only native advice command accepts
registered instance ID and mode, never paths/URLs or launch/session information.
Existing game-plan runtime selection and launch validation still determine authority.
