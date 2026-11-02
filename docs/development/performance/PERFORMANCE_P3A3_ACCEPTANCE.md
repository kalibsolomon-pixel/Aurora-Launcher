# Performance P3-A.3 acceptance

## 1. Overall verdict

**PARTIAL. P3-B NOT READY / NOT AUTHORIZED.** The unintended AFK cap is explained
and controlled. A conforming zero-entity A world is frozen, independently restored
and validated in one final real short capture. Fixed cadence and bounded overhead
trials are complete. Uncapped Minecraft display-tail fidelity and instrumentation
interference remain insufficiently bounded, so the essential calibration gate stays
PARTIAL. No gameplay improvement, JVM tuning or full benchmark is claimed.

## 2. Starting and ending HEAD

Branch `codex/client-3-production-authority`; starting HEAD
`c653fcbf6b3644ebaec4b9ca6d0efbc4367c0d35`, matching the request. Initial tracked
tree/index clean, 2462 tracked files, four protected untracked files, seven worktrees.
Ending implementation HEAD: `aa8b2feb7bf8dd978fe9352e1c950c3753ebb1bd`.
Ending review HEAD is the local commit containing this report; its exact SHA is
recorded after commit in `.git/performance-p3a3-final.json` and the final response.
A commit cannot embed its own content-dependent hash.

## 3. Local commits

| Commit | Scope |
| --- | --- |
| `aa8b2fe` | Bounded native WGL, read-only ETW queries, marker reconciliation, strict A NBT/archive/scene checks and 13 regressions |
| Commit containing this report | Actual sanitized diagnostics/calibration/world/final-capture evidence, canonical A hashes, current readiness and acceptance |

No push, merge, tag, release, deployment, production installation or version bump.

## 4. Changed files and deletion audit

[Complete inventory](../../performance-p3a/P3A3_CHANGED_FILES.txt): **28 paths,
19 additions and nine scoped modifications, zero deletions**. Five tools/tests,
their existing README, this report, two focused reports, two A lock/route documents,
eight sanitized evidence files, raw-byte Git attributes, the inventory and seven current authority documents
comprise the change. Product Rust/Svelte/TypeScript, manifests, dependencies,
lockfiles, native command DTOs and measured schema/quality gates are untouched.

## 5. Preservation and production isolation

[Audit](../../performance-p3a/evidence/p3a3/preservation.json): 2453 unmodified original
tracked files and all four protected developer files remain byte-identical. Only
the nine authorized originals change. All seven worktrees and six other HEADs
are preserved. All 149 original private P3-A.2 evidence files and the original
disposable world remain byte-identical. The original archive/failed inspections
were never replaced. Critical root documentation/build/package files remain present.

Production binary, launcher configuration and instance-registry hashes match the
baseline. This is a limited hash scope, not an owner-world or vault inventory.
All mutations targeted the marked disposable tree. No production world, instance,
mod, Java installation, release asset, account database or browser profile was
copied/edited. No direct secret inspection/export/backup/restore, process-name
termination, cleanup/reset, global power/input/driver/security change or upload.
Normal same-account refresh may rotate the shared Credential Manager credential;
the owner accepted that risk, and production sign-in was not probed.

Home widgets can refresh authentication at startup before live WebView inspection.
The owner explicitly authorized canonical checks → startup → immediate live-profile
verification, accepting that sequencing limitation. Both starts used this reviewed
exception. It is not presented as a guaranteed pre-authentication live check.
The earlier launcher disappeared during the interruption; its exact exit is unknown.
The final restart and supervised game both exited 0. [Native/quiet evidence](../../performance-p3a/evidence/p3a3/quiet-native.json).

## 6. Root cause of the 30 Hz condition

[Diagnosis](../../performance-p3a/FRAME_PACING_DIAGNOSIS.md) combines exact installed
1.21.11 bytecode, official documentation, UI observations and three comparable
same-child captures. Focused AFK mode after at least 85.770 s without game input
produced 29.912 Hz calls/F3 30. Recent input produced 1782.304 Hz calls. Minimized
mode after at least 69.409 s stationary produced 1781.049 Hz calls. All 36 foreground
observations per diagnostic belonged to the exact game. No fake activity was used.
The installed AFK branch caps at 30 after >60000 ms, then 10 after >600000 ms.

A now freezes the reviewed individual Custom graphics, Unlimited maximum, VSync
off, Minimized limiting, 1920x1080 windowed and 12/8 distances. Heap and JVM text
are unchanged. F3 rendering observations and ETW display estimates remain distinct.

## 7. Controlled cadence results

[Actual evidence](../../performance-p3a/evidence/p3a3/cadences.json): three 26-second
WGL stimuli, each with a predeclared 20-second inner window and pre/post-roll.
Observed independent rates were **30.00055 / 60.00031 / 119.99909 Hz**. Inner
PresentMon rows/markers were 600/600, 1200/1201 and 2400/2400. The one unmatched
marker is at the boundary; no interior mismatch or duplicate-nearest marker.
Mean paired interval disagreement was 0.053–0.069 ms, maximum <=1.0391 ms.
Exact PID/creation, 10000000-Hz QPC and raw hashes are retained. Present timestamps
follow swap markers asynchronously; physical scanout is not established.

The corrected stimulus uses one plan for execution/metadata, pre/post-swap markers
and fixed deadlines. Historical stale phase/stall metadata remains preserved;
no newly completed variable/stall protocol is claimed.

## 8. Event loss and completeness

Read-only ETW QUERY succeeded without elevation/privilege changes. EventsLost,
LogBuffersLost and RealTimeBuffersLost were zero through every controlled inner
window and the final Minecraft window. **ZERO OBSERVED THROUGH WINDOW** does not
cover an unobserved shutdown tail or establish physical presentation. Earlier
pacing diagnostics lack counters and retain UNKNOWN. Missing display estimates,
boundary truncation, stream mismatch and invalid CSV remain separate outcomes.
The short parser's conservative generic warnings are retained alongside the
external counter review. Full measured-run gates are unchanged.

## 9. Instrumentation overhead and uncertainty

[Report](../../performance-p3a/INSTRUMENTATION_OVERHEAD.md): separate 80-second
PresentMon-only and sampler-only series, off/on/on/off, 120 Hz, four predeclared
14-second inner blocks. Mean on-minus-off P99 was **+0.087754 ms / −0.021433 ms**;
machine CPU **+0.25175 / +0.01964 percentage points**. Both pass the observed
2% cadence / 2 ms P99 / 2-point machine-CPU screens. All successful observers exit 0.
An initial window-close interruption at about 43 s is retained separately.

Two blocks/state and drifting background load do not establish statistical
equivalence or negligible overhead. CPU counter quantization means reported zero
is not zero cost. Startup/timer/shutdown excess and read latency are reported with
their limits. Marker retention explains stimulus RSS growth. These 120-Hz trials
do not bound uncapped Minecraft interference: **overall calibration PARTIAL**.

## 10. Canonical Workload A world

[World evidence](../../performance-p3a/evidence/p3a3/world.json): the new copy uses
legitimate in-game cleanup, removing 18 entities and then 15 drops. No direct NBT
editing or terrain/gamerule relaxation. After normal save/reload, all-dimensional
NBT/region checks verify **zero non-player entities and zero block entities**,
seed 123456789, exact plains superflat layers, creative/normal, camera 0.5/66/0.5,
yaw/pitch 0/10, time 6000/clear frozen and required gamerules. All 1024 required
chunks are full, 5329 recorded; empty A mods and resourcePacks=[] prove no shader
loader/resource packs. The final run's saved scene still conforms.

## 11. Snapshot and restoration

Normal supervised preparation exit 0 preceded two stable source inventories.
The complete new archive is **22190080 bytes**, SHA-256
`d385120835fd6b0ad0fb8757601de97b066d9e23023e32733f62e6bd56e13f4f`.
Independent safe extraction matches all **56 files / 12 directories** and the
strict scene. [Canonical identities](../../performance-p3a/workload-a-lock.json)
bind archive, inventory, conditions, route and exact options. Read-only attributes
plus repeated hashes detect drift; absolute same-owner immutability is not claimed.
Only A hashes are populated; B/C objects and installation evidence remain unchanged.

## 12. Final short capture

[Final evidence](../../performance-p3a/evidence/p3a3/final-capture.json): one normal
authenticated restored-A launch, locked Java, Balanced/2048 MiB/empty custom text,
**342.978 s** stationary warm-up, 26-second capture / 20-second inner window,
debug overlay off and all 56 focus samples target the child. Fresh 60-second
preflight: 1.232% machine CPU, AC present, >=19.68 GiB available RAM.

PresentMon exits 0 with **46170 rows**, 43026 unavailable display estimates and
2486 inner display intervals; short quality **VALID WITH WARNINGS**. Presentation
calls average 1832.378 Hz, avoiding the old AFK branch. Display estimates average
124.320 Hz with a 504.896 ms maximum gap; these are not validated physical FPS or
stutters. All gaps/unavailable rows remain retained. Full-run quality INCOMPLETE.

61 CPU/memory samples cover the full 20-second window. Process CPU is 262.506% of
one logical core, sampled peak RSS 1122340864 bytes; Java heap/GC CPU remain null.
Machine busy CPU was 14.677%, including game/OS/observer and background work.
Post-exit scene and full options hash match the canonical lock. Supervised game
and retained-handle launcher exit 0, with no failure diagnostic.

## 13. Tests actually executed

**65 benchmark tests** (all 52 existing + 13 focused) and **13 P1 tests** pass,
zero failures. New synthetic tests cover identity/QPC/incomplete metadata,
interior-marker loss, unknown counters, phase bounds, bounded malformed NBT/regions,
unexpected entities/mods, missing graphics/scene fields, altered restoration and
archive hashes. Actual captures/worlds are independently reviewed, not synthetic
substitutes. [Test evidence](../../performance-p3a/evidence/p3a3/tests.json).
Existing ResourceWarning remains in the private test log. Rust/frontend/build
tests were not rerun: product source/startup/native contracts did not change.

## 14. Remaining limitations

Uncapped display-tail fidelity and observer interference; shutdown-tail loss;
absent game GC/clock correlation; B/C gameplay/worlds; other hardware/Linux;
thermals/VRR/driver-profile visibility. The native custom-resolution placeholder
defect stays deferred; the verified saved-game override works. Startup-refresh
sequencing is an explicitly approved limitation, and the first launcher exit is
unobserved. No failed evidence is hidden or treated as a successful full run.

## 15. P3-B readiness decision

**NOT READY and NOT AUTHORIZED.** A's limiter and canonical-world gates pass, but
uncapped measurement calibration, supported equal GC instrumentation and
representative B/C readiness remain outstanding. Future comparisons need these
gates, predeclared noise/effect budgets and separate owner authorization. No
five-run comparison, heap sweep, collector experiment or preset promotion occurred.
Stop after P3-A.3; this report grants no further execution.
