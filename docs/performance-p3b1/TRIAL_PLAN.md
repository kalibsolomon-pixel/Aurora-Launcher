# P3-B.1 bounded heap experiment

Declared before the baseline preparation capture. Owner scope is the attached
P3-B.1 request; older P3-A authorization statements remain historical.

Only saved instance `memoryMib` may differ between treatments: 2048 versus 1536.
Existing Configuration (`performanceMode: null`), empty custom JVM text, null
window override, no explicit Xms or collector flags. Microsoft OpenJDK
21.0.7+6-LTS, default G1, exact existing six-JAR Aurora diagnostic stack.
Native configuration Save and supervised Play remain the only launch path.

## Preparation gate

Prefer Option A: a new disposable copy of the preserved normal-terrain snapshot
`de973b21dc49c5ea2a71d60f452b1c8f28e8383264892f021b38f5feb07892f6`,
seed 123456789, camera (128.5,96,128.5), yaw135/pitch15, frozen clear noon,
creative/normal, natural mob spawning off and random ticks3. Increase render /
simulation distance to 24/12 before the baseline process. This loads more ordinary
terrain and existing natural entity/block-entity content without artificial Java
allocation, a new mod or gameplay automation. Record actual resulting inventories
and observable loaded counts; persisted counts are never loaded counts.

One 2048MiB baseline preparation process, >=300s stationary in-world warm-up,
30s Overlay capture and a separately scoped bounded JFR observation using the
unchanged privacy-filtered profile. Resource sampler at0.5s with exact process
creation identity. No forced GC. Record initial/end-warm heap, actual maximum,
post-GC use, commitment, pause/allocation indicators, RSS and available RAM.
First loading and any chunk generation are preparation, outside the stationary
window. Freeze the resulting world only after normal supervised exit. Canonical
A and the prior normal-terrain snapshot remain byte-identical.

Assess relevance and safe headroom before a candidate. Commitment below1536MiB
alone neither passes nor fails relevance. Consistently low use/little GC with no
credible mechanism stops the experiment. A baseline live set/burst envelope that
makes1536MiB unsafe also stops it. At most one reasoned workload adjustment;
otherwise PARTIAL. Do not increase load repeatedly or run a candidate for its own
sake. No full comparison is authorized by passing parser tests alone.

## Conditional comparison sequence

After passing the baseline gate, lock the new archive/options/configuration and
declare the practical resource goal and common JFR analysis window before the
first1536MiB run. The fixed order is:

| Pair | First | Second |
| --- | --- | --- |
| 1 | B1:2048MiB | C1:1536MiB |
| 2 | C2:1536MiB | B2:2048MiB |
| 3 | B3:2048MiB | C3:1536MiB |

Each uses a fresh process, byte-equal disposable world/options/mod configuration,
the same isolated launcher/root/profile, >=300s stationary warm-up after live
camera verification, identical observers and30s Overlay measurement. Native
Unlimited260, VSync off, inactivity Minimized, Aurora cap0/adaptive false/PARK,
1920x1080 windowed. Capture actual durations and warm-up/setup gaps. Treat any
nonaligned JFR/resource coverage as separate aggregates, never equal whole-export
GC rates. No duration/order changes after candidate results.

Maximum six valid comparison runs and one justified diagnostic retry per failure
category. Retain every failed/excluded attempt. Stop for OOM, candidate-caused
crash, repeated severe pacing/GC regression, insufficient headroom, unreliable
loading, material environmental drift, corrupt observers or failed preservation.
No unrelated-process termination, power changes, cap changes or extra JVM flags.

## Interrupted-setup diagnostic retry

The first preparation warm-up (PID23108, creation134366984825874295) completed
300.000149s but had only424/601 foreground samples. Before recording, the owner
interrupted Computer Use, and the resumed game had a changed camera/window.
No Overlay/JFR comparison capture or candidate ran. Retain this attempt as
INVALID for the baseline gate. One diagnostic retry for interrupted setup is
declared before any retry measurements: normal supervised exit, retain the
world/options/configuration, restore a byte-equal fresh copy of the original
source and24/12 settings, then a fresh2048MiB process and uninterrupted>=300s
warm-up. Another interruption in this category stops the experiment as PARTIAL.
The workload itself is unchanged; this consumes no workload-adjustment allowance.

## Baseline gate decision and comparison lock

Declared after the preparation retry, before any comparison/candidate process.
The retry passed601/601 foreground samples over300.000107s, normal supervised
exit0, camera/rules checks, coherent Overlay row/footer/summary checks, and readable
privacy-filtered JFR with zero reported DataLoss. The capture has1,457MiB committed
heap, approximately772–796MiB after GC, approximately1,386MiB maximum before-GC
use, ten collections (including concurrent G1 cycles), and thirteen top-level
pauses totaling36.6236ms. RSS is approximately2,376.45MiB. These are separate
quantities. At least16,320MiB physical memory remained available during capture.

This passes relevance: ordinary loaded terrain/content roughly doubles the prior
scene's commitment/live set and has observable allocation/GC activity. Changing
Xmx could change G1 expansion/collection ergonomics, including adversely. The
after-GC envelope leaves approximately740MiB beneath1536MiB; the new frozen
world removes preparation generation from the comparison. This supports a bounded
candidate attempt, not a promise of safety in loading/traversal or every workload.
No OOM objective, forced GC or workload adjustment is used.

Frozen comparison archive SHA-256:
`e53e838b0136a03dbdfa8848321063b001e44a33c3dc42d76001b85dfacaf287`.
Workload identity:`workload-heap-static-24-12-v1`; options SHA-256:
`33695bc0df527fbfe49ec2177cc75bed7b46cadfc581ab542eedaa16ca20fe87`.
All2,401 required full chunks in[-16,-16]–[32,32] exist;5,625 total recorded
chunks,199 persisted entities and803 persisted block entities are not live counts.
Preparation began from the earlier source archive and generated additional terrain;
it is excluded from all matched pairs. No first-run generation metric is compared.

The practical memory goal is a repeatable saving of at least128MiB in committed
heap or time-weighted process RSS across all three pairs, exceeding the new
baseline/paired variation, without material pacing, GC or stability regressions.
This is a declared use-case goal, not a statistical detection guarantee or a
universal threshold. Insufficient sensitivity to rule out that goal remains
INCONCLUSIVE. Small CPU differences remain inconclusive. Review P99/P95/severe
tails against this workload's actual repeat range; do not impose the older scene's
noise range as an equivalence bound. A lower RSS alone cannot excuse worse pacing.

Each comparison uses the same combined observers: existing Overlay30s, JFR45s
safety duration/maxage60s/maxsize32MiB with normal named stop immediately after
the30s resource sampler. Analyze fully contained JFR events over the resource
sampler's actual approximately30s UTC window, retaining and reporting boundary-
crossing events separately. Full exports and all pre/post-roll remain retained;
whole-export rates with unequal duration are not compared. These aggregate windows
do not establish precise frame/GC causality. Start sampling immediately after F10;
record key/resource bracketing and reject a material mixed-recorder window.

The fixed B1–C1–C2–B2–B3–C3 order above is unchanged. Every comparison starts from
byte-equal frozen world/options/configuration, fresh native-supervised process,
same exact managed Java/mods and>=300s uninterrupted stationary warm-up. Stop if
candidate after-GC use persistently leaves less than256MiB of1536MiB headroom,
or for the other declared safety/regression conditions. This conservative
experiment-specific headroom screen does not certify loading/traversal safety.

## Interpretation and completion

Report all paired absolute/relative differences and observed baseline variation.
Prioritize P99/P95/median intervals and >50ms bursts; these are eligible internal
HUD callbacks, never physical displayed frames. Heap commitment, after-GC live
use, top-level stop-the-world pauses and sampled allocations remain distinct from
RSS and maximum reservation. Do not sum nested GC phases or claim precise GC/frame
causality. Small CPU/RSS changes near prior variation remain inconclusive; combined
observer effects are unresolved and no assumed overhead is subtracted.

Promising requires repeated meaningful benefit beyond ordinary variation without
material regression; only then one bounded relevant stability follow-up. No
material improvement requires adequate sensitivity. Insufficient workload/data
means PARTIAL / INCONCLUSIVE. Keep presets/defaults/UI/schema/Client/production
unchanged. Restore only this experiment's saved memory to2048 via native Save if
changed. Stop after P3-B.1; local reviewed commit only, no publication or push.
