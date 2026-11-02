# Performance configuration architecture audit

2026-10-19, initiative Phase D. Reviewed current source at `4984010` before adding
controls; existing Phase P2 architecture already covers configuration ownership.
No duplicate preset system, automatic GC flag bundle or custom Java picker is
introduced. The new [bounded comparison](JVM_PERFORMANCE_INITIATIVE_PROTOCOL.md)
will inform whether any policy change is warranted.

| Concern | Existing authority and behavior |
| --- | --- |
| Java | Rust resolves the exact runtime component/platform/architecture required by the composed game plan. Managed manifest/files/hashes and bounded Java diagnostic are validated; no PATH/system-Java fallback. |
| Heap | `instances/settings.rs`: default 2048 MiB, accepted 512–32768; exactly one launcher-owned Xmx. Java chooses initial heap. Custom heap aliases conflict and fail explicitly. |
| Arguments | Raw text retained; bounded Windows-style quote parser produces a vector. Save and launch revalidate. `launch/resolve.rs` assembles deterministic managed/Mojang/loader/logging/custom arguments; structured spawn, no shell. |
| Modes | Optional persisted Efficient/Balanced/Performance/Custom advice. Missing field preserves Existing Configuration. Choosing a mode preserves heap/text until explicit edits and Save. Performance matches Balanced. |
| Memory advice | Rust host snapshot cached for 30 seconds. Reserve 4096 MiB from total and 2048 MiB from available; cap by architecture. Efficient target 1536, others 2048. Missing/stale/inconsistent/insufficient facts yield no recommendation. Explicit draft apply refreshes facts and supports Undo. |
| Advanced control | Existing Configuration retains established parser policy. Opted-in modes accept portable properties/assertions; unsupported VM/GC flags explain refusal and preserve text. Heap ownership is never bypassed. |
| Persistence | Versioned schema; absent mode is compatible. Damaged/unknown documents fail without overwrite. Instance identity and installed state stay separate from desired settings. |
| Readiness | Native instance/runtime/account validation remains authoritative; save changes refresh presentation readiness. Play deeply revalidates content/runtime/session under final locks and supervises the exact returned child handle. |
| Privacy | LaunchSpec is Rust-only/nonserializable, tokens redact in Debug/output, refresh credential remains in OS-backed storage. UI Ready never authorizes spawn by itself. |

The 512 MiB lower bound is an input sanity bound, not a guarantee modern modded
Minecraft will be playable. Host memory advice is a conservative heuristic, not a
measured optimum, launch reservation or GPU-memory guarantee. No mod-count or
CPU-model multiplier is justified. Large packs remain explicit per-instance
decisions based on their live set and observed GC/memory pressure.

This architecture is retained. New collector/initial-heap controls would need
exact-runtime validation, clear conflicts and evidence before expanding the
opted-in policy. Unsupported tuning is not silently repaired or dropped. Linux
memory parsing exists, but Windows results do not establish Linux runtime/auth or
performance acceptance.
