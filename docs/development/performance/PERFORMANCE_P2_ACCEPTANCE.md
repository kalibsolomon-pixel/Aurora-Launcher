# Performance P2 — configuration-mode acceptance

**Verdict: PASS for the evidence-gated P2 configuration foundation.** Efficient,
Balanced, Performance and Custom are functional persisted advice modes. Performance
intentionally matches Balanced. Minimum-heap and collector-tuning controls remain
**BLOCKED/unsupported** under the existing policy pending exact-runtime validation.
Real Minecraft performance remains **NOT MEASURED**. No FPS, tail-latency, GC, CPU
or startup improvement is claimed. Stop after P2; no P3/release work was started.

## Repository, commits and inventory

Branch: `codex/client-3-production-authority`.
Starting HEAD: `c8951e44ec619fffc4a304f37778625f8883bab4` (matched the request).
Starting tracked tree/index were clean; all four protected untracked files and
seven registered worktrees were present. Full tracked/untracked hashes, worktree
identities and status were captured before implementation in local
`.git/performance-p2-baseline.json`.

| Local commit | Scope |
| --- | --- |
| `c05f7f6` | Native mode persistence, migration, memory advice and argument validation/tests |
| `d28d4dd` | Instance UI, typed boundary, explicit draft controls and failed-save feedback |
| `41fe8d6fd285ef8a848385afbe9d540881affba2` | Architecture/spec/guides, isolated native/browser acceptance and regression evidence |
| Commit containing this report | Final acceptance and complete changed-file inventory |

Ending implementation HEAD: `41fe8d6fd285ef8a848385afbe9d540881affba2`.
Ending review HEAD is the acceptance commit containing this report. Its exact SHA
is recorded after commit in `.git/performance-p2-final.json` and the final response
(a commit cannot embed its own content-dependent SHA). All commits are local.
Full changed-file inventory: [CHANGED_FILES.txt](../../performance-p2/CHANGED_FILES.txt),
**54 paths: 9 scoped modifications and 45 additions, no deletions**.

## P1 findings and architecture used

[P1_HANDOFF.md](../../performance-p2/P1_HANDOFF.md) separates observed P1 findings
from proposals and blockers. AGENTS/ARCHITECTURE were unchanged from their full P1
reading; source and P1 acceptance/audit/plan/baseline/direction were used directly.
P1 established one native launch pipeline, per-instance heap/raw text, no default
GC/Xms tuning, lexical legacy conflict checking, no hardware advice and no real
gameplay baseline. Earlier startup/update performance studies remain separate and
their existing scripts/evidence are unchanged.

[ARCHITECTURE_DECISION.md](../../performance-p2/ARCHITECTURE_DECISION.md) chooses one
optional field in the existing instance configuration, rather than another JVM
settings file or command builder. Mode identity needs persistence even when advice
is identical, so registry schema 6 is justified. P2 authorizes safe advice modes
without benchmark claims; this does not bypass P1's gate for measured tuning.
Existing Configuration is the explicit missing/null compatibility state, including
fresh instances until deliberately opted in.

## Implemented behavior and precedence

Modes change advice, not heap/flags automatically. Efficient targets at most 1536
MiB; Balanced, Performance and optional Custom advice target at most 2048 MiB.
Explicit Use recommendation refreshes facts, names the proposed draft value and
changes only memoryMib. Save remains the authoritative atomic write. Manual maximum
and raw custom text remain accessible in the existing instance editor, reached from
global Minecraft & Java. No global override or second navigation/editor was added.

Saved mode/maximum, unsaved mode/draft and recommendation are distinct. Mode
transitions preserve custom text and heap. Undo/Discard reverse draft changes;
after Save a previous maximum can be restored explicitly. No custom text is silently
cleared/reordered. Delayed advice cannot overwrite a later manual heap or a changed
instance/mode generation. Unsupported text rejects Save and remains editable.

The actual native order stays generated Xmx, applicable Mojang JVM groups, loader
JVM contribution, Mojang logging, custom strings in original order, main class,
game arguments. Profiles add no JVM flags. Initial heap stays ergonomic. Game-plan
managed Java selection/major/architecture, deep validation, session/entitlement,
containment, final locked validation and exact-child supervision are unchanged.

Legacy settings retain their old accepted vocabulary and effective behavior.
Opted-in modes accept conservative portable user properties/assertion options,
reserve known managed Java/Minecraft path properties, and reject heap/classpath/
entrypoint/indirection and unsupported VM switches. Repeated identical options
remain intact; differing properties retain last-wins order with generic notices.
Multiple enabled collectors are classified as mutually exclusive; single collector
or pause-target switches still fail as unsupported rather than being guessed safe.
No arbitrary user arguments are executed as a compatibility diagnostic. Property
support covers JVM syntax, not every possible mod-specific property's behavior.

See [PROFILE_SPEC.md](../../performance-p2/PROFILE_SPEC.md) and
[USER_GUIDE.md](../../performance-p2/USER_GUIDE.md) for the complete contract,
unsupported controls and reversal behavior. Four modes are not four different
benchmark-proven JVM configurations.

## Hardware recommendation algorithm

Native on-demand Windows GlobalMemoryStatusEx uses one feature on the existing
windows-sys package; Linux reads bounded meminfo. No new package/dependency, GPU
or process scan, mod-count complexity inference, power change, CPU/GC worker override,
startup profiling, polling or telemetry. Cache lifetime is 30 seconds; source,
architecture, total/available MiB, age and unknown workload/GPU pressure are displayed.

Budget is the minimum of total RAM minus 4096 MiB, currently available RAM minus
2048 MiB and architecture ceiling (1024 MiB on x86, 32768 on x64/ARM64). Saturating
arithmetic avoids overflow. Cap the mode target by that budget; below 512 MiB or
unknown/inconsistent/stale facts, provide no recommendation. This reserves OS/
background/native/shared-GPU headroom and is an explicit heuristic, not a universal
RAM percentage or measured live-set optimum. Advice never clamps saved custom heaps
and cannot guarantee future pressure or that a demanding pack fits.

## Migration, preservation and native acceptance

Schema 5/4 migrate to 6 without activating a mode or changing raw text, heap,
window, loader policy, identity or installed pins. Known 2/3 behavior remains.
Legacy files already carrying the new field, unknown fields/modes/versions and
malformed files fail deliberately. Existing revision/expected-original checks and
atomic writes remain authoritative. Config 7, installed-game/runtime schemas,
release metadata and Tauri identity/version are unchanged. Older readers do not
support registry 6; development acceptance never points them at the owner root.

Native acceptance used a fresh marked direct-temp managed/WebView root and a
configuration-only synthetic schema-5 record, **no installed Minecraft and zero
accounts**. A stored Ready fixture is not claimed as deep playable readiness.
Actual native commands preserved 3217 MiB and the exact quoted duplicate-property
text, persisted every mode, rejected a heap override without overwriting state,
and restored Custom/heap/text after a real process exit/restart. Both native
processes exited 0. No authentication bypass or real Minecraft launch occurred.

The native rejection test found that a fresh registry object reinitialized the
editor and cleared the error. The editor now initializes by derived instance ID;
the error/draft survive refresh. A browser regression returns a fresh registry
identity after failure, and real native evidence confirms the persistent message.
Child controls use parent callbacks; the final browser run has no ownership warnings.

Native debug executable SHA-256, client/outer rectangles, native minimum track,
actual CSS viewports and DPR are in
[first-verification.json](../../performance-p2/native/first-verification.json) and
[restart-verification.json](../../performance-p2/native/restart-verification.json).
Default and 1024×768 CSS clients pass at DPR 1.25. Native minimum track is 900×650
physical; actual client 882×640 physical gives **706×512 CSS**, rather than assuming
720×520 is the viewport. 200% text is test-local enlargement in that native host;
vertical scrolling is expected and controls remain reachable/horizontally contained.
Actual native Rose preference, Custom, advanced text, unsaved selection and error
states are captured. No browser viewport capture substitutes for native sizing.

## Tests, screenshots and limits

- Full Rust: **992 passed, 31 existing ignored, zero failures** (986 library + six
  icon tests). Format and all-target check pass.
- Full frontend: **282 passed**, sequential execution with unchanged assertions;
  type check has zero errors/warnings. Focused Settings set: 40 passed; final
  component subset: seven passed.
- Production frontend build and native development build pass. No installer,
  release package, version bump or production installation.
- UI-1 through UI-5 regressions pass, including UI-2B/crop/UI-3.1: 11, 17, 6, 6,
  11, 13, 13 and 14 cases/groups/checks respectively. Owner-approved UI phases
  were not reopened or redesigned.
- New browser interactions pass six groups: transitions, explicit/undoable advice,
  delayed edit protection, failed saves with fresh native state identity, responsive/
  200%/keyboard layouts and unavailable advice. Zero page errors/ownership warnings.
- Actual native initial/restart and all requested screenshot classes pass.

Detailed commands/results/limitations: [TEST_MATRIX.md](../../performance-p2/TEST_MATRIX.md)
and [VALIDATION_SUMMARY.json](../../performance-p2/VALIDATION_SUMMARY.json).
Thirteen committed native screenshots: [SCREENSHOTS.md](../../performance-p2/SCREENSHOTS.md).
All capture paths/hashes: [EVIDENCE_INVENTORY.json](../../performance-p2/EVIDENCE_INVENTORY.json).
125 duplicate fixture PNGs remain locally retained/ignored; metrics and native
screenshots are committed. Full private logs remain at `.git/performance-p2-*.txt`.

An initial parallel SSR run hit shared Vite optimizer transport timeouts; the full
sequential run passed. A stale visual optimizer dependency returned HTTP 504; the
final visual server uses an exclusive temporary cache, without deleting existing
developer caches. No assertions were replaced with weaker checks. Linux native
runtime/detection, other Java-vendor/build tuning, real workloads, and gaming
performance are **untested/unmeasured**. GC/Xms controls are explicitly unsupported.

## Repository/production isolation and next phase

All 2330 original tracked files and four protected untracked files were checked
against the pre-work hashes. Nine explicitly scoped original files changed;
the other **2325 originals are byte-identical**. No disappearance, deletion or
unrelated rewrite. Seven worktree paths/branches/other HEADs are preserved; only
this chat's branch advances. Historical architecture bytes were preserved when
inserting the current P2 section. Root docs/manifests/lockfiles/native build config,
prior acceptance evidence and all four protected developer artifacts remain intact.
Local verifier: `.git/performance-p2-check-preservation.py`.

Production Launcher 1.4.2, owner instances/worlds/modpacks/packs/settings/accounts/
credentials/Java/release artifacts were not mutated by this work. Test roots and
exact child handles are separate; no process-name termination, broad cleanup,
system-protection/power change, profiler upload, private-server connection, sign-in,
push/merge/tag/deployment/workflow dispatch or release preparation occurred. Existing
live applications can make their own normal writes; blanket hash identity of all
live owner data is not claimed. Repository/developer-file preservation is verified.

Proposed **P3 for owner review only**: freeze disposable workload/mod/world/route
locks, select and calibrate real presentation capture, record exact managed Java
vendor/build/support, arrange a quiet resource window and explicitly authorize any
isolated sign-in/tool installation needed. Reuse P1's paired repeated-run methodology
to evaluate heap advice before differentiating Performance or adding GC/Xms controls.
Current blockers remain unvalidated capture, workload locks/authenticated test
instance, quiet resource window, exact runtime tuning evidence and other hardware/
Linux coverage. No P3/P4 benchmark, automatic profile promotion or release is started.
