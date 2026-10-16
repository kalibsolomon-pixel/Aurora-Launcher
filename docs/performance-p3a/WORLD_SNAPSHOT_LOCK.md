# World snapshot locks — BLOCKED

No test world was created, loaded, copied, frozen, archived or restored in P3-A.1.
No owner world was inspected. The owner-confirmed absence of the test Windows user
blocks authenticated world creation. Existing [workloads.json](workloads.json)
remains unchanged: world/route/settings hashes are null, not fabricated digests.

| Workload | Installed-material evidence | Actual world lock / execution |
| --- | --- | --- |
| A | P3-A minimal Fabric 1.21.11 / 0.19.5, no optional mods | BLOCKED — absent |
| B | P3-A Aurora/API/Sodium/Lithium exact artifacts | BLOCKED — absent; coexistence untested |
| C | P3-A same B inventory | BLOCKED — scene intensity and world absent |

Use [WORKLOAD_SPEC.md](WORKLOAD_SPEC.md) without changing its future P3-B 300/600 s
protocol. Short-session capture is separately labeled calibration. Preserve the
specified seed 123456789, generation settings, position/yaw/pitch, creative/normal,
clear weather/time 6000, render/simulation distances, actual entity inventories,
resource/shader absence, gamerules and generated chunks. A uses superflat and B
normal terrain; a seed is not proof of the generated scene or complete chunk state.

After isolation and legitimate login, create only a disposable test-owned world.
Record actual scene conditions; wait for declared chunks/lighting to settle. Exit
Minecraft normally and prove the exact child has exited before snapshotting. Retain
the entire world including level/player/region/entity/POI/dimension data and empty
directories, plus separate options/mod configurations. Keep the immutable original
apart from working/restoration copies. If making an archive, record its exact-byte
SHA-256, relative inventory and provenance before reviewing safe extraction; no
archive extraction or restoration automation is delivered in this phase.

New read-only `tools/benchmark/readiness_checks.py` checks a **prepared directory
snapshot** and an independently prepared restoration against one full inventory.
It never creates, copies, extracts, deletes, restores or authenticates anything.
It rejects unknown schema, duplicate metadata keys, invalid conditions/camera,
digest/size drift, missing/extra files or empty directories, unsafe names, case
collisions and linked/reparse ancestors. Per-file 2 GiB, aggregate 16 GiB and 100,000
entry bounds are inspection limits, not authorization to scan owner data.

Operator-reviewed layout, entirely inside a new test-owned evidence directory:

```text
snapshot.json
conditions.json
snapshot/        # immutable original directory, complete world including level.dat
restored/        # independently restored disposable directory; no writer active
```

Manifest schema 1 has exactly `schemaVersion`, `sampleKind` (`test-world` or
`synthetic`), `workloadId` (`A`, `B`, `C`), `conditionsSha256`, sorted `files`
(`path`, integer `sizeBytes`, lowercase `sha256`) and sorted `directories`.
Relative names use forward slashes. Conditions have exactly `minecraftVersion`,
`seed`, `generationSettings`, `playerPosition`, `orientation`, `gameMode`,
`difficulty`, `weather`, `timeOfDay`, `renderDistance`, `simulationDistance`,
`entityConditions`, `resourcePacks`, `shaders`, `gamerules`, `generatedChunks`.
Structured chunk/entity/generation records must describe actual observations;
record options/config hashes in the reviewed lock/provenance as well.

```powershell
python -B tools/benchmark/readiness_checks.py <test-owned-evidence>/snapshot.json
```

Exit 0/VALID proves offline inventory/conditions consistency only. It cannot parse
NBT to prove scene claims, establish clean exit, authenticate provenance or enforce
immutability against later writes. Hash the final manifest and any archive, record
clean-exit/runtime/source-instance provenance privately, and review those separately.
All eight new tests use explicitly synthetic bytes, including fake level.dat. They
prove refusal/consistency behavior; they do not constitute a usable world or a lock.
