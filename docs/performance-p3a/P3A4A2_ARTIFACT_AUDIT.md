# Released artifact and compatibility audit

Publisher metadata retrieved on 2026-10-17. Files downloaded only into private
evaluation storage for inspection; published SHA-1/SHA-512 and sizes checked before
inspection. Runtime installation used the existing native verified-store installer.
Observed SHA-256 is an additional local identity, not a publisher SHA-256 claim.
[Exact identities and publisher digests](evidence/p3a4a2/artifacts.json).

| Project | Exact release / provider ID | Observed SHA-256 |
| --- | --- | --- |
| Valence | 1.1.0 / K8qKDXKF | `aef9788dc393aefd07ddf4fab4cdbefc19301c5ed20c83b7b76a49cf0d38a3b0` |
| Performance Overlay | 1.0.2+mc1.21.9-1.21.11-fabric / qyzs7zWa | `46c9500f67824996cd81b00a4d3da2dc09d5c6d56495cff98276c961d74804c2` |
| Cloth Config | 21.11.153+fabric / xuX40TN5 | `8a40c84e5ecde525acfb6c4e13b8002dacfca3ef9534d7faf6e8049656f423c8` |

Valence filename is `valence-benchmark-mc1.19-1.21.11-1.1.0.jar`, 152731 bytes.
Actual fabric.mod.json requires Loader >=0.15.0, Minecraft >=1.19 <=1.21.11,
Java >=17, fabric-command-api-v2 and fabric-lifecycle-events-v1; fabric-api and
fabric-rendering-v1 are suggestions. Representative bytecode major is 61
(Java 17). The project page's older 1.21.1/Loader 0.16 wording differs from the
release/JAR declaration. Only the declaration is verified; runtime compatibility
cannot be established from a broad version range or filename. No official source
link exists in publisher metadata/JAR contact; targeted discovery found none.
This does not prove no public source exists.

Overlay JAR is 188784 bytes, embedded mod version 1.0.2, bytecode major 65
(Java 21). It requires Loader >=0.18.4, Minecraft >=1.21.9 <=1.21.11,
Java >=21, fabric-api `*`, cloth-config `*`; Mod Menu recommended, not required.
Neither JAR declares incompatibilities with the pinned Aurora/Sodium/Lithium set.
That absence alone is no runtime proof. The release build properties use Fabric
API 0.141.1+1.21.11 and Cloth Config 21.11.153+fabric. Those are build pins, not
declared minimum dependency versions. Actual test used release-required Aurora
Fabric API 0.141.6+1.21.11 and Cloth Config 21.11.153; no dependency downgrade.

Official Overlay source current commit:
`8490600d67a4694c1f7ab6325e1328eb99535615`; release tag v1.0.2 resolves to
`9e923693ece057f84e2a0cd69e86f52ab3bce14b`. Read its AGENTS.md before source
inspection. FpsTracker and BenchmarkCsvWriter have no diff between these commits;
entrypoint/UI/test changes elsewhere do exist. Audited released bytecode separately
confirms the callback clock, row objects, queue and formatting. No reproducible
whole-JAR/source-build equality is claimed; no candidate build, fork or modification.
[Private input/disassembly identities](evidence/p3a4a2/source-audit.json).

Official sources: [Valence project](https://modrinth.com/mod/valence-benchmark),
[Valence release metadata](https://api.modrinth.com/v2/version/K8qKDXKF),
[Overlay release metadata](https://api.modrinth.com/v2/version/qyzs7zWa),
[release build properties](https://github.com/itsgeorge22/performanceoverlay/blob/9e923693ece057f84e2a0cd69e86f52ab3bce14b/gradle.properties),
[Cloth Config metadata](https://api.modrinth.com/v2/version/xuX40TN5).
