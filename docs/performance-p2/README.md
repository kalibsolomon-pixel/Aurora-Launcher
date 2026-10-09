# P2 background audit attribution evidence

Report: [PERFORMANCE_RESEARCH_P2.md](../../PERFORMANCE_RESEARCH_P2.md).
Instrumentation is independently revertible commit
`b8591ae78f9ba319dfd07f2d56dbe12085c272b4`; reporting/tools/evidence are separate.
P1 optimized source is `0d68340f0e280978f675c6f83330a10fb97bf678`.
No optimization, installation, game launch, publication or push is performed.

## Probe contract

P0.2's eleven unsigned numeric fields, opt-in environment, exclusive exit-time
write, 16,384-record native bound, dropped counter and 2,048 frontend-marker bound
are unchanged. Async context still installs/restores on every poll and blocking
jobs inherit their submitting context. Ordinary builds have no-op probes.
No string, result body, path, URL, identity, error or credential field was added.
The ordinary startup command has no frontend `Probe`; its unique root span ID
identifies the actual native request and child `parent` IDs establish ownership.
Its `correlation` field remains zero; zero correlation does not mean parentless.

| Event | Boundary |
|---|---|
| 39 | Actual `startup_update_check` native command request |
| 40 | Actual manual `check_for_updates` request |
| 41 | Actual explicit `preview_client_update` request |
| 42 | Client discovery helper, including manifest and full preview |
| 43 | Synchronous full Client transaction preview |
| 44 | Complete read-only instance validation |
| 45 | Update approval fingerprint construction, including its mod scan |
| 46 | Separate Launcher update discovery |
| 47 | Client published-manifest transport/body/parse interval |
| 48 | Final update overview construction and event emission |

All span durations are inclusive. A synchronous scope ending means returned or
unwound, not a recorded success result. No failure is inferred from timestamps.
Existing events 20/21/23 are game audit/game metadata/mod inventory. The analysis
requires actual ancestry `20 -> 44 -> 43 -> 42 -> 39`, balanced spans, bounded
numeric schema and zero overflow; timing proximity is not an attribution input.

## Reproduce

`preserve.py capture` records Git status/history and all original tracked and
untracked hashes, ignored prior evidence/diagnostics, managed files and installed
executable in ignored `private/`. `verify` reports differences and never repairs
or deletes. Only the four probe/test source files are allowed to change. Prior
investigated WebView and server-enrichment operational writes are counted
separately; instances, account/config files, runtimes and artifacts stay protected.

`check.py` runs frontend check/tests/build and ordinary/research Rust format,
all-target checks and tests, exporting sanitized receipts and keeping logs private.
`build.py` requires a clean tracked source tree, records the exact commit and
every tracked source-input hash, builds two byte-checked source copies in a new
OS-temporary `aurora-p0-2-p2-*` root, and re-hashes/copies the retained P1 optimized
research artifact. The retained P1 root is a local reproduction input; update its
explicit `PREVIOUS` path when reproducing elsewhere. It verifies the approved
updater public-key digest and uses the same key/configuration for both builds.
No installer runs. Two bounded concurrent builds use separate targets and finish
before collection. Existing dependencies/compiler caches are copied, never cleaned.

Use `start-trial.py <new-research-root> <baseline|attribution|control> <number>`.
It checks the executable digest and refuses any competing Launcher/Java. All
application stdio goes to the OS null sink. It records OS creation time without
reading process arguments. Source-built window activation, accessibility polling
and normal close use the Computer Use skill. Load the functions in
`ui-observer-notebook.js` **directly into the persistent JS session**; a separate
VM context does not bind this notebook into that session. Pass the current root
explicitly to `p2observe`, inspect the sanitized result, then call `p2close` in a
separate call. Raw UI text and screenshots are not retained.

Protocol: one excluded attribution pilot (trial 0), six adjacent alternating
pairs (odd baseline then attribution; even attribution then baseline), then two
feature-off boot controls. Poll delay 60 ms; observe at least eight seconds;
sustained Ready requires a one-second uninterrupted Ready hold. Complete traces
and normal closes are required. No Play or owner data modification is part of
this protocol. These are source-matched attribution/overhead observations,
not an optimized-versus-unoptimized comparison. No p95 is reported at n=6.

`analyze.py <research-root>` exports sanitized trials and strict numeric traces;
`analyze.py` alone regenerates the summary without app/owner I/O. It imports the
unchanged P0.2 validator and extends only its closed event vocabulary. Additional
checks prove parent containment, root ancestry, correlated scans and absence of
metadata descendants. `validate.py [research-root]` validates privacy patterns,
four malformed-trace rejection cases, reproducibility, links, tracked-file and
critical-file presence, and optionally re-hashes all artifacts/frontend files.
Pattern checks supplement the closed numeric contract and human review; they are
not a claim to prove arbitrary text safe. Private inventories/logs remain local.

## Evidence index

- [Preflight](evidence/preflight.json): initial HEAD, P1 ancestry, protected-domain counts.
- [Source snapshot](evidence/source-snapshot.json): exact instrumented source commit, tracked inputs and hashes.
- [P1 baseline](evidence/baseline-artifact.json), [attribution](evidence/attribution-artifact.json), [feature-off control](evidence/control-artifact.json): executable and embedded frontend identities, modes and updater-key identity.
- [Frontend equivalence](evidence/frontend-equivalence.json): exact source comparison and narrowly normalized generated build identifiers.
- [Environment](evidence/environment.json): warm/online conditions, toolchains and uncontrolled factors.
- [Trials](evidence/trials.json): numeric OS/UI observations, pilot exclusion and normal closes.
- `evidence/trace-*.json`: unchanged numeric envelope with P2 event vocabulary.
- [Summary](evidence/summary.json): each actual request lineage, counts, inclusive timings, overlaps, Ready ordering and paired probe deltas.
- [Check receipts](evidence/verification-checks.json): frontend and both native configurations.
- [Preservation](evidence/preservation.json): all original-file comparisons and JNA checks.
- [Validation](evidence/document-validation.json): schema/privacy/provenance/link/preservation receipt.
- [Failures and limits](evidence/failures.json): pilot and setup exclusions; no hidden game or optimization acceptance.

P0/P0.1/P0.2/P1 historical timings are not pooled with this startup series.
