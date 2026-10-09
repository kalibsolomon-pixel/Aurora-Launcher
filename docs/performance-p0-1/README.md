# P0.1 evidence and reproduction index

The [report](../../PERFORMANCE_RESEARCH_P0_1.md) now records the owner's
2026-10-09 authorization to benchmark the current installation **as-is**.
No separate account or storage isolation is claimed. The original 2026-10-08
BLOCKED preflight and the first resume preflight remain historical observations.
Their account-creation boundary was superseded by the owner's later instruction.

## New as-is evidence

| Artifact | Contents / limits |
|---|---|
| [Resume preflight](evidence/resume-2026-10-09-preflight.json) | Exact source/build identities, absent old game PID, zero Java/Launcher before pilot, retained content comparison; account boundary is historical |
| [Environment](evidence/as-is-environment.json) | Physical CPU/GPU/OS/driver/power state, capture sizes, normal network/cache condition and observer limitations |
| [Identities](evidence/as-is-identities.json) | Collection source HEADs, installed payload digest, tool/source-document hashes; compiled payload provenance and trust identity remain unknown |
| [UI observations](evidence/as-is-ui-observations.jsonl) | Numeric automation-request/capture intervals, outcomes and phase markers; 29 first-Ready starts, 20 one-second sustained-Ready checks, 20 repeats per content domain |
| [Game observations](evidence/as-is-game-observations.jsonl) | Two Workspace and one Home Play; window/menu observations, manual gaps disclosed |
| [Sanitized process timings](evidence/as-is-process-timings.json) | 29 Launcher creation pairs and three game-child pairs; no PIDs, paths or arguments |
| [Supervised exits](evidence/as-is-supervised-exits.json) | Three independent log-creation timestamps and exit-code-0 header observations; maximum read 96 bytes, no child output |
| [Deployed Client identity](evidence/as-is-client-artifact.json) | Existing Aurora JAR SHA-256, source revision unknown |
| [Idle Home](evidence/as-is-idle-home.jsonl), [idle Workspace](evidence/as-is-idle-workspace.jsonl) | Six ~10-second intervals each, exact Launcher ancestry/WebView descendants, stable tree and no Minecraft |
| [Numeric summary](evidence/as-is-summary.json) | Sample counts, first/median/nearest-rank p95 where n≥20, worst/repeat worst, sample variance/deviation, uncertainty and input hashes |
| [First preservation comparison](evidence/as-is-before-report-protection.json) | Retained failing comparison: two JNA temporary natives disappeared during game use; no repository or executable change |
| [Scoped restoration](evidence/as-is-jna-preservation.json) | Two exact original missing temporary files restored from byte-identical surviving siblings; SHA-256 checked before/after; no other mutation |
| [Post-restoration comparison](evidence/as-is-after-jna-restoration-protection.json) | All 15,574 pre-existing Launcher files, 426 Client files and 11,821 protected managed files matched |
| [Final comparison](evidence/as-is-final-protection.json) | Same protection baseline; only report/index edits allowed; no tracked removal |
| [Artifact checks](evidence/as-is-document-validation.json) | JSON, privacy-field, link and critical-file checks plus deterministic summary regeneration |

Content distributions use the series marked by `content_series_metadata` and
`content_series_end`: the first entry into each tab in the final process is
separate, followed by 20 repeats. Earlier process/pilot and later refresh/recovery
observations are not pooled into those distributions. Raw trial numbers can
include earlier pilot numbering; use phase and series markers, not trial number
alone, to select a cohort.

Three later unsuccessful observation records remain visible: resource-tab
navigation stayed on Mods for the observation timeout, and two explicit refreshes
had no observed busy transition. They are not latency samples. The navigation
recovery used an inspected screenshot and coordinate action; refresh probes are
inconclusive. Earlier pilot problems included a wrong workspace predicate, an
incorrect three-pack expectation, an offscreen tab, and a reused expired polling
deadline. Geometry failures before one repeated Play and two normal startup-close
attempts were re-observed before retrying and are not launch/close latency samples.
No game was launched twice while Starting/Running.

## Reproduce numeric results without opening the app

```powershell
python docs/performance-p0-1/analyze-as-is.py
```

The committed sanitized process/exit inputs suffice; private parent/PID data is
not required to regenerate the public summary. The first execution during this
session exported these correlations from the private observer file. Auxiliary
Java children precede each game's independently reserved supervisor log; they
must not be mislabeled as Minecraft spawns. Exact parentage, full inventories and
restoration procedure remain local under ignored `private/resume-2026-10-09/`.
Scoped `.gitattributes` preserves the exact new research-tool and as-is evidence
bytes, including mixed collection line endings, so Git does not rewrite input hashes.

`analyze-as-is.py` validates cohort sizes and unique paired process identities.
It overwrites only the derived summary. Raw observations and preservation records
are retained. Its process/log pairing is an external inference, not native
returned-handle instrumentation. First-menu samples are coarse screenshot upper
bounds; previous screenshot return times are not strict presentation lower bounds.

## Reproduce collection in another authorized session

1. Record source/installed artifact identities and fresh repository/content
   inventories in a new ignored directory. Inspect critical files, status,
   history, tracked set and any deletion. Never overwrite retained baselines.
2. Verify the previous exact game session has exited normally and Launcher is
   closed. Do not kill Java by name. Current same-account authorization applies
   to this run; do not assume a data-root override isolates other storage domains.
3. Use the Computer Use skill with `node_repl` and `@oai/sky`. Select the returned
   normal installed app/window uniquely; exclude diagnostic/review apps. Keep one
   action per inspected state, refreshing immediately. Pause for owner handling
   if interactive authentication is required; never automate its dialogs.
4. [Notebook helpers](ui-observer-notebook.js) retain the actual collection
   functions. Loading them performs no action. Initialize `sky`, `fsPerf`, an empty
   `observations`, `savedObservations=0`, a **new** `uiEvidencePath` created
   exclusively, and `launcherProcessApp` from the selected returned window.
   Determine indexes from each fresh UI state. Functions depend on that state;
   they are not a standalone bulk input driver. Do not save raw UI text/screenshots.
5. [Process observer](observe-processes.ps1) reads executable identity/parentage and
   OS creation times only. Give it the exact installed executable and a **new**
   output file below an ignored private root. It runs for the bounded requested
   duration, never closes/terminates an app, and must not collect command lines,
   environment, tokens or child output. Retain its exact version/hash with a run.
6. Close via the observed Launcher Close control, verify zero Launcher/Java
   processes, then perform one `runStartup()`/`startAndCapture()` call. The latter
   continues through the one-second Ready hold. Inspect before any next action.
   First nine recorded starts used only first-Ready; later 20 used the hold.
   Window discovery is not paint. Verify later navigation responses separately.
7. Use the existing real instance. Obtain current expected row counts with a
   pilot; `measureTab` defines this run's 31/4/1 workload. Separate first entries
   and repeat series. Never change selection/mods/settings to manufacture a sample.
   Same-instance picker dismissal is not a selection-change timing.
8. For Play, record its actual request clock, verify no active game, and observe
   the newly returned unique managed-Java window. Inspect loading/menu screenshots
   directly; never treat Launcher Running or a visible loading menu as usable.
   Finish with visible Quit and confirm the same process exited. Correlate exact
   parentage, log creation and bounded exit header without exporting arguments.
9. Reuse [P0 resource sampler](../performance-p0/sample-resources.ps1) with the exact
   current Launcher PID for six 10-second intervals per settled view. Verify its
   descendants/start identities, keep Java absent, and do no hashing/build/UI
   navigation during the interval. Record foreground/visibility and background
   load limits; CPU is a one-core equivalent.
10. Compare preserved files before any commit. [Resume verifier](verify-resume.py)
    takes a fresh label, reads this run's private inventories, and refuses to
    overwrite a comparison. Only the report/index are allowed repository edits.
    Differences stop the verifier; it never repairs or cleans. Investigate before
    restoration. Game-created JNA temporaries can change even when no user data
    or trusted installed artifact changes; retain failed and subsequent comparisons.

Do not rerun a game over the final restored temporary files merely to check the
report: that would repeat JNA's cleanup and invalidate final preservation. No
cache eviction, offline/reboot/auth-expiry manipulation, update/install, new
profiling, performance implementation or publication was done here. Missing
conditions require a separately reviewed collection plan, not invented timings.

## Historical P0.1 preflight

| Artifact | Historical contents |
|---|---|
| [Original preflight](evidence/preflight.json) | Original source/build/process/isolation prerequisites; no packaged trials |
| [Original preservation](evidence/protection-verification.json) | Passing initial P0.1 comparison against the earlier inventories |
| `private/launcher-baseline.json`, `private/client-baseline.json` | Ignored original status/history/tracked/untracked/hash inventories |
| `private/verification-details.json` | Original local path-level comparison |

Original helpers `snapshot.py`, `preflight.py`, `verify.py` are unchanged. They
refer to their original baseline destinations; use new destinations for resumed
work rather than replacing historical evidence. P0's report, native harness,
summaries, JFR aggregates and resource records are unchanged and retain their own
methodology. Native component times and conditional source audit counts are not
substitutes for new packaged timings or observed invocation counts.
