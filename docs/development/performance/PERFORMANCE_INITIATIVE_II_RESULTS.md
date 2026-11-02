# Aurora Performance Initiative II — Launcher results

2026-10-19. Full cross-repository report:
[Realistic Workloads, Startup, and Latency](../../../../Aurora-Client/PERFORMANCE_INITIATIVE_II_FINAL_REPORT.md).
The user explicitly authorized both repositories for this initiative.

The accepted production change fixes omitted automatic readiness. Update discovery
can supersede initialization's launcher-state read before an instance is selected.
The first result is correctly discarded, but startup previously never requested
runtime/readiness. Initialization now waits for the winning reload before selection.
Native integrity, account, runtime, content and final locked validation remain
authoritative; freshness guards are preserved.

| Fresh-process cohort | First Ready after process creation returned |
| --- | --- |
| Reference b1/b2/b3 | absent; no runtime/readiness request, observed beyond 30s |
| Candidate c1/c2/c3 | 29.777 / 11.386 / 10.915 seconds |
| Final expanded native n2/n3/n4 | 11.822 / 11.352 / 11.788 seconds |

There is no percentage speedup claim from a censored baseline. Initial Home paint
is about 0.32–0.35 seconds and can precede restoration. The final restored usable
Home marker is 779.180/781.226/781.953 ms. It requires native state and account
discovery. Double RAF checkpoints indicate frontend scheduling, not scanout.
Windows QPC brackets correlate numeric native probes with the exact child spawn
span. Frontend performance.now remains a separate clock. First owned window
observations (22–43 ms, 20 ms polling) may describe helper/blank windows.

These are warm OS cache, shared isolated WebView-profile boots of an unoptimized
native debug executable embedding production frontend assets with research mode.
They are not cold-cache or signed shipping performance results. Required integrity
work dominates later startup and launch preparation; no mtime-only trust cache is
introduced. Research probes compile away in ordinary builds, accept bounded
numeric/closed vocabulary data, and flush only at exit. No identities enter traces.

The final local native binary completes an expanded 10.5-minute Client/world test,
normal game exit 0 and Ready recovery. A fresh Launcher then supervises an induced
exact-child exit 23 at the main menu, reports failure and returns Ready. Another
fresh Launcher successfully relaunches to menu and exits 0. All parents exit 0;
171 saved-world files remain hash-identical across menu-only cycles. This tests
native UI, installed Loader 0.19.5, managed Java 21.0.7+6, G1/2048, launch supervision
and the remapped Client. The Client is a local unmanaged test jar, not an official
release association; official manifests and distribution authority are unchanged.

Native Play-to-spawn brackets are 10.331–10.354 / 10.978–10.983 / 11.042–11.048 s.
Initial child-to-menu is 19.235–19.258 s, child-to-interactive-world 27.787–27.811 s,
including a deliberate 3-second menu dwell. Successful warm relaunch menu is
15.813–15.818 s. Aurora initialization is 209/175 ms in those native runs. First
child-window observation is invalid as creation timing in the first run because
the observer starts late; later relaunch observes it about 9.038 s after spawn
return with 50 ms polling. Diagnostic java.exe differs from actual javaw.exe;
the observer now resolves the manifest's launchExecutable and retains exact handles.

One stable nine-process family minute uses 30.89% of one core / 1.103% of the
28-thread machine, summed working sets 2618.9–2619.2 MiB. Shared pages are counted
multiple times; this is not heap or unique physical memory. Observer overhead is
uncalibrated. G1/2048 and Efficient/Balanced/Performance settings are unchanged.

Preserved limitations/workarounds:

- Explicit native window dimensions fail real metadata's `${resolution_width}`
  substitution. Failed n1 has no child and remains preserved. Disposable config
  uses null dimensions and saved options, without bypassing validation. Fix the
  validated width/height substitutions and add a real conditional-argument test.
- Native fixture omitted copying options initially: actual sustained framebuffer
  2560×1415 and GUI cap 120 differ from the Client's 1280×720, 30-GUI-cap matrix.
  The native run is a separate sustained/integration diagnostic. The helper now
  copies options; historical evidence is not rewritten or pooled.
- A later n3 revalidation briefly showed Needs attention and recovered on automatic
  focus refresh. Its completed trace proves initial automatic Ready at 11.352 s.
  No manual repair or trust bypass was used; further reproducibility is required
  before attributing a new product defect.
- PresentMon/software-input results do not establish physical input-to-photon
  latency. One ambiguous frame association prevents a completeness claim. No input,
  Frame Pacer or Reflex optimization is promoted from that diagnostic.

Validation: 28 startup tests including two new race regressions; original source
fails the regression. Full frontend 284 tests pass, type/style check zero errors
or warnings; Rust format/check succeed, 997 native tests pass with 31 existing
ignored. Production frontend, ordinary native and final research native builds
succeed. Windows native boot/failure/relaunch tested; Linux runtime is untested.
Existing STATIC_VCRUNTIME and other unrelated native warnings remain.

Branch `codex/performance-initiative-ii` starts from
`ecf5ea8f23ffd7e0c36ac79a6020a0a92dd46ce7`. Changes are committed locally;
delivery includes the resulting commit ID. No push or publication. The original
2629-file / 731,719,665-byte baseline has no missing or protected changed files.
Pre-existing untracked developer files/caches remain untouched and excluded.
Private raw roots: OS Temp `aurora-p0-2-ii` and `aurora-p0-2-i`; private baseline,
build logs and copied binaries: `.git/performance-initiative-ii`. No account data,
credentials, browser state or raw logs are committed.

Client-side ownership was explicitly authorized in the same user request. Its
accepted primitive-key RegionCache change removes lookup allocation with preserved
LRU lifecycle; six matched gameplay processes show no demonstrated FPS gain.
No separate handoff is blocking this joint initiative. Larger packs, other hosts,
shipping/cold startup, custom resolution and physical latency remain unresolved.
