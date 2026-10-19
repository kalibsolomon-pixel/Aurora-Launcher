# Launcher overhead and integrated validation — fixed before collection

2026-10-19. Current embedded frontend + native **debug** build, Windows/WebView2.
No comparison to an optimized release binary is valid. Record binary/source hashes.
Use a fresh isolated managed root copied from an existing test-owned Fabric
instance/cache/runtime; final Client jar is a local unmanaged test mod. Production
instances, original worlds, browser profiles and release authority remain untouched.
Only opaque saved account metadata is copied; the native account/session flow
restores credentials normally. Never inspect tokens, full arguments or auth UI.

Observe actual startup until the home screen is usable; record spawn and first
observed readiness timestamps as an upper bound including observation delay, not
precise startup latency. Preserve failures. Allow >=60s settled home state, then
capture 60s at 0.5s for the exact native process and its observed descendants.
Repeat one 60s settled home capture after a normal restart to check persistence
and whether idle work recurs. No builds, copies, exports or screenshots inside
resource windows. Discord stays in the copied disabled state. If a material
repeated idle cost appears, attribute it before changing production source.

Use retained read-only process handles and creation identities. Family membership
is observed at capture start/end without reading command lines. Child churn or
incomplete sampling makes the family result partial, not a hidden zero. Sum
process CPU time and working sets with their limits: RSS sums can double-count
shared pages and are not unique physical memory or Java heap. Host activity,
available RAM and sampler overhead remain contextual. No idle or startup gain is
claimed if production source is retained. Historical P1/P2 evidence stays separate.

Through the normal native UI, validate existing settings, edit/save/reopen a safe
memory value, and restore 2048 MiB/Balanced. Exercise an invalid/conflicting custom
heap argument and recover without loss. Native Play must resolve the managed Java,
validate installation/session and launch the final Client with Fabric/Sodium/OpenGL.
After a stationary >=60s world warmup, collect a 60s internal Overlay capture and
same-window process-family resources with Launcher still open. Inspect actual VM
collector/heap (only selected numeric/boolean flags), logs and normal shutdown.
Check return to ready and a subsequent launch; a disposable deliberate abnormal
exit may exercise supervision if safe, with saved world already closed. Preserve
all test-root logs/configuration/evidence. Tests separately cover corrupt/unknown
schemas and exact-child crash handling; do not damage original files to test them.

Integrated capture is a compatibility/resource observation, not a matched FPS
improvement trial: native classpath, Fabric Loader version, session and launch
route differ from the dev JVM matrix. Report those differences. No synthetic
metrics, input-to-photon claims, Linux acceptance, public release or distribution.
