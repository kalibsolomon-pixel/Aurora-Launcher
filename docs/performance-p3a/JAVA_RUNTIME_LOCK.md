# Java runtime lock — Windows x64

**PASS for the exact isolated Windows runtime identity; game instrumentation PARTIAL.**
Actual evidence: [java-runtime.json](evidence/java-runtime.json). The normal native
`create_instance` and `ensure_instance_runtime` commands resolved Minecraft 1.21.11 /
Fabric 0.19.5, then installed/verified the official runtime into a fresh marked
temporary root. Native restart deep validation rechecked 402 files / 99,041,788 bytes.
No owner Java was copied, executed for diagnostics, replaced or changed.

| Lock | Observed value |
| --- | --- |
| Component / required major | java-runtime-delta / 21 |
| Vendor/build | Microsoft-11369940 OpenJDK 21.0.7+6-LTS, 64-Bit Server VM |
| Platform | windows-x64; no architecture fallback |
| Official runtime manifest SHA-1 | cb4394a27089d19f65d5baa6cf0482c27c3c7865 |
| javaw.exe SHA-256, observed executable identity | 1ba6a4fb32a6a3d5c925fe837ae6717e2f75743a71e013e668e6dadb909245ba |
| java.exe SHA-256, observed diagnostic identity | 0afb1170b83c156dfdb7f54bb34c9b058064175c329a5d80b782bcc9dbaa2164 |
| Default collector diagnostic | G1 |

Manifest/files were verified against published SHA-1 through the established store.
The executable SHA-256 values are additional observed identities, not invented
official SHA-256 expectations. The feed can change; future preparation must compare
the exact manifest/build/executable identities and stop on drift. Never select a
different vendor or reuse a filename as proof of the lock. Reacquire through Aurora's
official resolver/verified installer in a fresh root if unavailable; do not reproduce
the installer in benchmark Python.

Fixed, bounded credential-free diagnostics succeeded: `-version`, `-Xlog:gc -version`,
`-XX:+PrintFlagsFinal -version`, `-Xlog:help`, `--list-modules`. PrintFlagsFinal exposes
G1/Serial/Parallel/Z/Shenandoah selector names; that is **not** a validated successful
launch of every collector. Only default G1 executed. No alternative collector,
pause target, Xms or production configuration was activated. Available jcmd/jfr/
jstat/jmap executables and management/JFR modules were observed, not attached to games.

A bounded standalone Java source-file probe with 128 MiB diagnostic maximum and two
small explicit collections generated [real G1 logs](evidence/standalone-gc.log).
[gc-plumbing.json](evidence/gc-plumbing.json) records parser results. Its diagnostic
heap bound is not a product preset or Minecraft treatment. Source compilation/GC
plumbing is not gameplay, presentation calibration or a performance baseline.
The parser window ends at the last logged VM uptime, not an invented process lifetime.

Future JVM-only baseline, source-derived from the checked-in 1.21.11 fixture:
Xmx from saved memory; Mojang's Windows HeapDumpPath; managed native extraction
properties; launcher brand/version; ordered classpath; installed logging property;
then explicit user text. Paths become semantic `<natives>`, `<classpath>` and
`<logging>` labels. Required metadata/main class/session handling remain untouched.
The existing native resolver test verifies the effective vector/order for all P2
modes. No full live argument vector or game/session arguments are exported here.

P2's four modes disallow GC logging switches. Future instrumented laboratory
instances must deliberately use Existing Configuration with identical bounded GC
logging and explicit heap values derived from reviewed advice. Record stored mode
and advice source separately. No validator exception, hidden flag injection or
claim that differently named but identical vectors are distinct treatments.

[Java 21 logging/source-file reference](https://docs.oracle.com/en/java/javase/21/docs/specs/man/java.html).
Linux needs its own official-platform runtime lock/build/hash and diagnostics;
the Windows runtime is never used as Linux validation.
