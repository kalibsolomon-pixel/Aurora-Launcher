# Benchmark artifacts, schema 1

Normative executable validation is `tools/benchmark/quality.py:validate_manifest`;
the accompanying [run.schema.json](run.schema.json) documents the structural
contract. No external schema package is required. Synthetic example:
[evidence/synthetic/run.json](evidence/synthetic/run.json). An unresolved workload
proposal is not a valid measured run manifest; do not fill missing hashes with zeros.

```text
<explicit local disposable benchmark-data root>/
  <run-id>/
    run.json
    raw/frames.csv
    raw/gc.log
    raw/process.csv
    producer-diagnostics.json
    summary.json
```

Large private raw data belongs outside the repository, in a marked test-owned root;
do not put owner paths or data in a manifest. The small committed fixtures and native
screenshots are explicitly scoped evidence. Nothing uploads automatically. Raw file
names are operator-chosen relative names, never remote filenames or arbitrary URLs.
Artifact hashes use SHA-256 of exact raw bytes. Mod locks retain the publisher's
actual algorithm (SHA-1/256/512); runtime manifests retain official SHA-1. These are
different identities and must not be relabeled.

Scoped Git attributes preserve evidence bytes, including native CRLF CSV/logs,
across checkout platforms. Whitespace checks recognize CRLF as line endings;
raw files are never newline-normalized to make a digest match.

Required run fields: schema/run/workload/configuration/block IDs; synthetic/measured
kind; complete/failed/interrupted state and failure reason; timezone-qualified
start/end UTC; warm-up/measurement seconds and shared-clock window start; exact
runtime/vendor/build/architecture/manifest/executable hashes; Minecraft/loader/API;
mod inventory hashes; world/route/options/settings locks; hardware/OS/GPU/driver;
display/window/render settings; power/AC/thermal/background/quiet-preflight provenance;
capture version/binary/settings/format/semantic; process PID/creation identity and
monotonic origin; two GC clock anchors; reviewed JVM-only arguments; all three raw
artifact paths/hashes; optional startup markers; quality policy and nonsensitive
machine warning codes (lowercase letters/digits/underscores/hyphens, 1–80 characters).

Nested graphics/scene/capture settings may add reviewed nonsensitive fields; unknown
top-level fields and secret/account field names fail. Validators are not a general
privacy sanitizer: raw GC/capture producers and free-text descriptions require manual
review before sharing. No account ID/name, email, password/token, full live process
arguments, private server address or absolute personal path is allowed. Use source-
reviewed JVM-only semantic paths, never introspect child command lines to build exports.

Frame contract: pinned PresentMon 2.3.1 default CSV with `--qpc_time`, integer QPC
frequency/origin and explicit swap chain, or normalized `timestamp,pid` with explicit
s/ms/ns units. Normalized sources declare internal-render or synthetic semantics;
they cannot attest displayed presentation. PresentMon import records undisplayed
rows separately and estimates displayed timestamps using the declared pinned columns.
No generic FPS-to-frame conversion. Calculate intervals between consecutive display
anchors inside the window; retain boundary discard count. A long gap is retained,
not automatically deleted as capture loss. Completeness also requires producer review.

Process CSV: monotonic_ns, pid, creation_id, cpu_time_ns, rss_bytes, peak_rss_bytes,
system_available_bytes, system_total_bytes, system_busy_ns, system_total_ns. Times/
counters are integer ns; memory integer bytes. CPU is counter delta / wall interval,
reported as one-core percent (can exceed 100). System busy/total gives machine percent.
Background CPU is an estimate after subtracting target process CPU, not per-process
attribution. Edge intervals are time-weighted/clipped with uniform-rate assumption;
sampling gaps and boundary coverage remain explicit. Working set, OS lifetime peak,
post-GC used heap and post-GC committed heap remain separate quantities. Unavailable
live heap, paging, GPU and total GC CPU are null, never zero.

GC parser: complete unrotated HotSpot G1 unified summaries with uptime, level/tags;
Young/Full/Remark/Cleanup pauses deduplicated by phase/cycle. End heap capacity can
shrink below pre-GC usage. CPU User+Sys records are rounded process CPU accumulated
during logged phases, not isolated GC-worker CPU or total concurrent GC cost.
Retain source-clock pauses and map event endpoints
through two anchors for proximity observations. Missing collector headers/unknown
collectors/malformed summaries/overlapping boundaries fail; no invented generic parser.

Quality classifications: **VALID** (offline checks pass), **VALID WITH WARNINGS**
(e.g. small tail depth, coarse clocks, display-estimate uncertainty), **INVALID**
(structure/identity/hash/clock/cohort conflict), **INCOMPLETE** (missing files,
failed/interrupted run, insufficient coverage/warm-up/samples). Every requested run
stays in output, including errors; exit 2 signals invalid/incomplete. Neither VALID
classification proves real capture authenticity nor promotes a readiness gate.

Statistics reuse P1 type-7 interpolated percentiles, population frame variance,
mean/median/P95/P99, P99.9 only at >=10000 intervals; presentation rate = 1000/mean
interval. Stutter >50 ms is a declared operational threshold, counting both affected
frames and consecutive-frame bursts. Process means are weighted by elapsed time.
Startup JVM/main-menu/world-load seconds are separate nullable measurements with
documented marker uncertainty. GC/stutter proximity is not a causal conclusion.

Cohort locks include runtime, mods, workload, graphics, hardware, protocol, capture
and controlled conditions. Run-specific QPC origin, stream address, thermal observation
and preflight hash differ legitimately; they are retained but not pooled-lock keys.
Different runtime/build/mods/render/scene/conditions reject cohort pooling; identical
configuration IDs require identical reviewed JVM vectors and unique blocks. Equal
vectors with different profile names are equivalent treatments. Use P1 repeated-run
analysis only after a reviewed conversion to its interval format and compatible
cohorts; this P3-A tool does not automatically adapt/pool display estimates into it.
