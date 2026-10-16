# P3-A readiness matrix

Overall **PARTIAL**. P3-B **NOT READY / NOT AUTHORIZED**. Offline correctness
and native installation do not substitute for actual authenticated game capture.

| Gate | Status | Evidence / exact remaining condition |
| --- | --- | --- |
| Frame capture selected | PASS | [Decision](FRAME_CAPTURE_DECISION.md): PresentMon 2.3.1 console primary candidate; Windows/Linux alternatives and semantics defined |
| Frame capture calibrated | BLOCKED | No approved installed binary, actual native presentation stimulus or raw calibration output; follow [protocol](CALIBRATION_PROTOCOL.md) |
| Minecraft capture validated | BLOCKED | Zero real Minecraft samples; needs isolated authorized authentication and capture; Aurora/Sodium coexistence untested |
| Java runtime locked | PASS | [Actual lock](evidence/java-runtime.json): Microsoft 21.0.7+6-LTS, exact manifest/executable hashes, deep native validation; nondefault collectors untested and out of scope |
| Workload A ready | PARTIAL | Base content/Fabric/Java actually installed and validated; exact scene/protocol specified; no world snapshot/settings/route hash or authenticated execution |
| Workload B ready | PARTIAL | Exact base/mod artifacts actually installed and rehashed; world snapshots and coexistence execution pending |
| Workload C ready | PARTIAL | Same B inventory installed/rehashed plus bounded scene specification; world/intensity/execution pending |
| Isolated environment ready | PASS | Fresh marked direct-temp managed/WebView root, native installed instance, registry 6, actual restart; worlds/logs remain test-root scoped and unpopulated until authorized |
| Authentication ready | BLOCKED | Zero accounts; same-account Credential Manager namespace risk requires separate Windows test user or reviewed dedicated-account policy, then explicit owner sign-in |
| GC capture ready | PARTIAL | G1 synthetic/real standalone log ingestion validated; fixed Java logging works; no instrumented game capture or game clock anchors |
| CPU/memory capture ready | PARTIAL | Actual Windows self-process/system sampling and offline ingestion pass; not yet exact-child game sampling; live heap/paging/GPU/thermal sources unknown |
| Artifact integrity | PASS | Required metadata/digests/containment/identity/clock/cohort checks tested; reviewed privacy/producer authenticity still manual |
| Quiet-system readiness | BLOCKED | Actual 30 s ambient evidence says NOT READY; owner Java activity, uncontrolled preparation, unknown AC/GPU/thermals and shorter-than-planned observation |
| P3-B execution plan | PASS | [Review-only protocol](P3B_PROTOCOL.md): effective-vector equivalence, A/A, >=5 repetitions, order rotation, warm-up/retention/promotion gates |
| Linux native/capture | NOT TESTED | Separate runtime, credential-store limitation, capture/CPU accounting adapter and Wayland/X11 calibration required |

Owner actions are concrete and separate: approve exact capture-tool installation/
permissions; choose a safe Windows test-user/account arrangement; authorize short
legitimate Minecraft/material/capture validation; choose a quiet resource window;
then review updated gates and separately authorize P3-B. No approval is inferred
from this matrix or from passing parser tests.
