# P3-A.4a.1 — completeness recovery and independent validation

No primary metric accepted. The original P3-A.4a 1.873352-second row gap and
3,356 independently recorded swaps remain intact. This phase adds concrete
evidence of PresentMon consumer-buffer overflow; ETW session counters and
consumer record retention are distinct gates.

## Correction to the preceding review

The earlier validator and review missed producer stderr warnings saying
`overflowed present events detected`. Original logs report 30,711 in the uncapped
Minecraft capture, and 3,350/4,802/6,592/4,412 in the prior controlled observer
captures (PresentMon1/both1/both2/PresentMon2). These captures now classify INVALID
under the corrected shared short reader. Earlier `VALID WITH WARNINGS` meant
offline consistency, but was also too permissive in the presence of this explicit
loss diagnostic. Prior raw bytes, reports and acceptance remain unchanged;
this correction supersedes their permissive quality classification. Their
conservative rejection of a primary metric remains appropriate.

One synthetic regression first reproduced the validator failure; then the shared
diagnostic rejection vocabulary gained overflow/overflowed. Full measured-run
importer/schema, ETW thresholds and loss semantics are unchanged. Unknown loss
does not become zero, and missing display estimates do not become dropped frames.

## New independent WGL trials

| Target Hz | Inner rows / swap markers | Unmatched / duplicate nearest assignments | Largest nearest difference ms | Review |
|---:|---:|---:|---:|---|
| 240 | 3360 / 3360 | 4 / 4 | 1.5102 | REVIEW REQUIRED |
| 500 | 7001 / 7000 | 4 / 4 | 0.9061 | REVIEW REQUIRED |
| 750 | 10500 / 10500 | 4 / 4 | 0.7682 | REVIEW REQUIRED |
| 1000 | 13997 / 13998 | 37 / 36 | 0.6763 | REVIEW REQUIRED |

Single 80-second stimulus; 14-second inner windows. Same tool, full default
tracking, native WGL before/after-swap QPC markers. No overflow warning in this
trial. Aggregate counts nearly match but nearest-event association is not unique;
none passes the existing strict reconciliation screen. The baseline synthetic
test suite overlapped this first trial briefly: retain that background-work
limitation, and do not use it to establish accuracy or observer cost.

Targeted 1,800 Hz default tracking reported **8,343 consumer-overflowed events**.
Its 20-second inner window contains 29,876 rows versus 35,796 swap markers,
eight >50 ms observed row gaps and a maximum 775.1731 ms interval. These are
failure-diagnostic observations from an INVALID capture, not workload stalls.
The following submission-only condition (`no_track_display`, `no_track_gpu`,
`no_track_input`) exited 0 but emitted no CSV. It therefore provides no usable
submission stream on this path. Analysis failed on that absent CSV; raw output,
markers and failure record are retained. ETW snapshots and endpoint readings
were only in memory when analysis failed and are **UNKNOWN**, not reconstructed.
No favorable retry of that condition was performed.

After checking the same binary's documented buffer option, one full-tracking
1,800 Hz confirmation changed only process-local present buffer capacity from
default 2048 to 8192. Inner 20 seconds: 35,821 rows and 35,821 swap markers;
observed independent rate 1791.028 Hz, ETW rate 1791.033 Hz. Maximum observed ETW
interval 1.8187 ms, zero >50 ms gaps, no consumer-overflow warning, zero ETW loss
observed through the inner endpoint. Display estimates unavailable for 91.9634%
of raw rows. Nearest matching still has 1,556 duplicate/unmatched assignments;
mean interval disagreement 0.03025 ms, maximum 0.955 ms. Aggregate recovery is
not a one-to-one timing certificate.

## Supported mechanism and unresolved questions

Pinned PresentMon 2.3.1 consumer source declares a 2048-entry present ring;
overwriting an occupied tracked slot calls `RemoveLostPresent`. Its own warning
recommends increasing `set_circular_buffer_size`. The controlled capacity change
supports consumer retention capacity as a cause of this high-rate missing-row
failure. It does not establish that every previous gap has the same cause, or
that every frame retained with a larger buffer is timed accurately. This is a
single bounded mechanism confirmation, not a performance improvement claim.

Process identity, one `Other` / swapchain0 stream, raw integer QPC ticks and
frequency, exported interval arithmetic and inner boundaries were checked where
retained. Those checks do not detect all missing events. ETW zero counters cannot
rule out loss inside the consumer. OpenGL `Other` timestamps are downstream ETW
events rather than direct API entry. Startup/shutdown tail loss remains unknown.
High-rate scheduling jitter, matching ambiguity and observer cost remain separate.

No independent Minecraft swap markers exist in these trials. Standalone WGL
markers never become Minecraft evidence. Minecraft defaults retain the original
2048 buffer; the diagnostic confirmation does not silently change the matrix
instrument. Full raw fields/hashes and available counters are in phase evidence.

Source semantics: [PresentMon 2.3.1 consumer](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/PresentData/PresentMonTraceConsumer.cpp)
and [consumer header](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/PresentData/PresentMonTraceConsumer.hpp).
Pinned local copies and same-binary help were inspected; the linked tag paths
were also verified against the upstream repository.
