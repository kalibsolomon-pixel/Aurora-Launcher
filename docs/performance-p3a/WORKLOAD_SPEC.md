# P3-A.4a.4 practical normal-terrain stationary subset

[Workload evidence](evidence/p3a4a4/workload.json) freezes seed123456789 normal
Overworld terrain and the exact B mod stack, plus diagnostic Overlay/Cloth.
The archive SHA-256 is
`de973b21dc49c5ea2a71d60f452b1c8f28e8383264892f021b38f5feb07892f6`.
Fresh world/config/options inventories must equal the lock before every process;
reset and verify the live camera after loading, then warm stationary for >=300s.
See [trial plan](P3A4A4_TRIAL_PLAN.md) and
[acceptance](../../PERFORMANCE_P3A4A4_ACCEPTANCE.md).

This is a reproducible forest/plains/water stationary view, more representative
than the prior superflat collector scene. It excludes traversal, generation bursts,
servers and memory pressure. Whole-world persisted entity/block-entity counts are
not loaded counts; live F3 entities are boundary observations, loaded chunks and
block entities remain UNKNOWN. Dynamic entities can evolve after identical restores.
No strict canonical B promotion or canonical A mutation is implied.

# Current P3-A.3 Workload A lock (historical)

A now has a conforming, independently restored zero-entity canonical world.
Exact archive/route/options identities are in [workloads.json](workloads.json) and
[workload-a-lock.json](workload-a-lock.json). The actual 1.21.11 UI calls the reviewed
individual graphics Custom; the earlier Fancy proposal is superseded for A only.
A uses Reduce FPS when: Minimized, Unlimited maximum, VSync off, 1920x1080 windowed,
render/simulation 12/8 and the otherwise unchanged scene. This prevents the confirmed
AFK cap during stationary warm-up/capture without simulating input.

Normal in-game entity cleanup preserved terrain and required gamerules. Complete
NBT/region inspection, save/reload, archive extraction and the final run's persisted
state verify zero entities/block entities, exact camera and required full chunks.
See [world evidence](WORLD_SNAPSHOT_LOCK.md) and [final capture](MINECRAFT_CAPTURE_VALIDATION.md).
B/C proposals and objects are unchanged and remain unvalidated for gameplay/worlds.
Full 300/600-second benchmarking remains unauthorized; high-rate calibration/GC
and representative-workload readiness remain blocked. Historical proposal follows.

> P3-A.2 created and restored an actual A snapshot, but its 19 entities and capture
> pacing do not meet this proposed workload lock. Canonical hashes remain unresolved.
> See [current world status](WORLD_SNAPSHOT_LOCK.md) and [capture report](MINECRAFT_CAPTURE_VALIDATION.md).

# Reproducible workload specification — execution pending

Machine-readable proposals and unresolved material hashes: [workloads.json](workloads.json).
No world has been created or copied. Version declarations are not compatibility
execution evidence; null snapshot/route/settings hashes block a measured run.

| Workload | SPECIFIED | MATERIALS AVAILABLE | ISOLATED INSTANCE READY | EXECUTION VALIDATED |
| --- | --- | --- | --- | --- |
| A lightweight | Scene/protocol specified; actual world hashes pending | Official base game/Fabric/Java installed; world absent | Base content and Java validated; gameplay **BLOCKED** | NOT TESTED |
| B Aurora representative | Exact mod identities/hashes and scene specified | Game and all four exact mod jars installed/rehashed; world absent | Base content validated, shared locked Java available; gameplay **BLOCKED** | NOT TESTED |
| C demanding | Same B mod inventory, controlled heavier scene | Game and all four exact mod jars installed/rehashed; world absent | Base content validated, shared locked Java available; gameplay **BLOCKED** | NOT TESTED |

All target Minecraft 1.21.11, exact Fabric Loader 0.19.5, and the Windows Java
lock in [JAVA_RUNTIME_LOCK.md](JAVA_RUNTIME_LOCK.md). A has **no Fabric API or
optional mods**. B/C propose Aurora Client 3.0.0, release-required Fabric API
0.141.6+1.21.11, Sodium 0.8.14 for 1.21.11 Fabric, Lithium 0.21.4 for 1.21.11 Fabric.
Publisher metadata declares matching game/loader labels and no listed dependencies
for the latter two; that does not prove coexistence with Aurora. Frozen version IDs
and official SHA-1/SHA-512 are retained in workloads.json; no SHA-256 was fabricated.
Both B/C were actually installed through native creation and provider preview/
fingerprint-reviewed installation. [mod-environment.json](evidence/mod-environment.json)
rechecks release SHA-256 and Sodium/Lithium official SHA-1/SHA-512 on actual files,
and records additional locally observed SHA-256 identities without relabeling them
as published expectations. Native complete-content validation returned Ready for
both; neither game was launched. The world and execution gates remain pending.
No auto-upgrades or "latest" selection occurs during execution. B/C compatibility
must be proved in an authorized launch before materials are called validated.

Mod inputs: checked-in `src-tauri/production/aurora-releases.json` for Aurora/API;
creator-published [Sodium version metadata](https://api.modrinth.com/v2/version/rkdTcxoT)
and [Lithium version metadata](https://api.modrinth.com/v2/version/Ow7wA0kG), retrieved
2026-10-16. Acquisition used Aurora's normal preview/verified installer;
the separate metadata query never materialized an artifact or implemented an installer.

Common fixed conditions: creative single-player; seed 123456789; normal difficulty;
time 6000 fixed; clear weather fixed; no natural mob spawning; Fancy graphics,
clouds off, particles All, FOV 70, GUI scale 2, default textures; no shaders/resource
packs; no dynamic resolution; 1920x1080 actual client render target, windowed;
VSync off, unlimited FPS. Record monitor/VRR/compositor state separately; desktop
2560x1440/240 Hz inventory is not proof the game ran at 1920x1080. A/B use render
distance 12 and simulation 8; C uses 24/12. Normal random tick speed 3. Freeze exact
options/mod-config bytes, resource/shader absence and entity inventory in snapshot.

A: superflat, fixed camera `(0.5,66,0.5)`, yaw 0, pitch 10; zero spawned entities
and zero redstone machines. Generate/load the 512-block square around origin and
allow lighting to settle before freezing the pristine disposable snapshot.

B: normal terrain, fixed camera `(128.5,96,128.5)`, yaw 135, pitch 15. Generate the
512-block square around that point and freeze the actual generated chunks, lighting
and entities. No claim that the seed necessarily contains a particular village or
forest at those coordinates. Record what the generated scene actually contains.

C: separately built normal-terrain snapshot, camera `(256.5,96,256.5)`, yaw 225,
pitch 20; same B mods. Proposed bounded stressor: 16 armor stands and 64 loaded
hopper blocks in a fixed documented arrangement, no other spawned entities. Freeze
arrangement/block-entity inventories, item contents and world bytes after construction.
The scene is intended to be demanding; actual resource pressure is NOT ESTABLISHED.
If it fails to exercise the intended bottleneck, revise the workload lock before
comparisons. Mod count is never used as a complexity proxy.

Each uses a **stationary camera**: owner positions once, releases all movement,
waits 300 s in-world warm-up, then captures 600 s without movement. This is a deliberate
P3-A simplification of P1's proposed walking routes to reduce manual route variance
without an automation mod. It measures a stationary scene, not general traversal
or chunk-generation performance. Record start/end coordinates and yaw/pitch; >0.1
block or >0.5 degree deviation is a provisional execution warning requiring review.
Do not discard runs solely because their frame metrics look bad. If traversal is
added, use a separately versioned keyframe route with predeclared checkpoint tolerances
and measured manual timing deviations, not a hidden change to these workloads.

World lock procedure after authorization: create only in the test user's disposable
instance; settle/pre-generate declared chunks; stop the exact game normally; archive
that test-created world and configs; inventory relative files/sizes/hashes, then
SHA-256 the immutable snapshot/archive and route/settings documents. Seeds alone
are insufficient. Restore into a **new disposable run instance** through the reviewed
native preparation/copy procedure; never overwrite owner worlds or recursively reset
unknown trees. No reset/deletion helper is delivered. Keep startup/chunk generation,
main menu and world-load timings outside the stationary measurement cohort.

Use unchanged AC/power/driver/display/overlay/thermal conditions, verified at preflight.
No power-setting writes, private servers or gameplay automation mods. If uncontrolled
entities/lighting/weather/window changes occur, retain the run and its reason; do
not silently repair the snapshot or blend different workload hashes.
