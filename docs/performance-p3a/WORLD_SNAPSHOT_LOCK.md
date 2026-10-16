# P3-A.2 world snapshots — inventory PASS, workload lock PARTIAL

Workload A now has a complete actual test-world snapshot, archive and independently
extracted restoration. [Sanitized proof](evidence/p3a2/world-snapshot.json).
B/C have no worlds or execution locks. The canonical [workloads.json](workloads.json)
world/route/settings hashes remain null because A does not conform to its zero-entity
scene; a nonconforming capture must not silently become the proposed P3-B workload.

Minecraft saved and exited normally; Aurora's exact-child supervision reported
exited/0/no diagnostic before any copy. The isolated launcher subsequently exited 0.
No owner world was inspected, copied or overwritten. The only source was the
exclusively created disposable A world in the marked test instance.

| Lock / validation | Actual result |
| --- | --- |
| Complete inventory / restoration | VALID, existing read-only world checker |
| Files / directories | 56 / 12, including complete level/player/region/entity/POI/dimension content and empty directories |
| Archive size | 22,159,360 bytes |
| Archive SHA-256 | `6e77ef39a966e4175469d06ba0f503500f0fd4f269a9bbbc0b1e03aed4354c33` |
| Inventory manifest SHA-256 | `2570658c952055bceaaf8b66bac9b1b036110ce4d61b408ddfb41c896db1d7e1` |
| Conditions SHA-256 | `be3c6f81e0d2386a61e0b1d4fe21d4c6e5f63c1f3f21b7fe8c9897e98e986371` |
| Options SHA-256 | `8d6484f7cc489795a6966bdd757511447939081448b45da7ab07afcc1819d15d` |
| Required 512-block square | All 1,024 chunks from (-16,-16) through (15,15) have full status; 5,329 total recorded chunks |
| Exact A entity condition | FAIL, 19 non-player entities (chicken/horse/item/pig), zero block entities |

The archive was constructed from the full directory snapshot and restored into a
new separate verification directory. Every member was checked for safe relative
names, file/directory type and containment before extraction; links/special members
were refused. The checker rehashed both complete inventories. Options and instance
config were retained separately. Snapshot files/archive are read-only and originals
remain intact; later hash checks detect drift, while the same owner can still change
read-only attributes. Absolute immutability is not guaranteed.

Read-only bounded NBT/region inspection after exit verifies actual seed 123456789,
creative/normal, time 6000, clear weather, advance_time/advance_weather/spawn_mobs off,
random tick speed 3, position (0.5,66,0.5), yaw/pitch (0,10), superflat plains layers
(1 bedrock, 128 dirt, 1 grass), no generated structures, and render/simulation 12/8.
Temporary pre-generation used 23 chunks before restoring 12. This explicit surface
supports the specified grounded camera at y=66. No resource/shader/optional-mod pack
was added. Actual entity data, player state and generated chunks were frozen; the
world is not claimed reproducible from its seed alone.

The world and raw inventory contain private player identifiers and remain outside
committed evidence. Public summaries retain hashes, counts and reviewed conditions.
No entity was removed or snapshot modified to hide the failed condition. A future
separately authorized correction must create/review a new lock and retain this one.

The following P3-A.1 checker contract remains applicable; its synthetic tests do not
replace the actual P3-A.2 snapshot/provenance checks above.

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
