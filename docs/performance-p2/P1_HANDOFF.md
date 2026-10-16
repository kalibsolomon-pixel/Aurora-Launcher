# P1 handoff — JVM configuration P2

Starting HEAD c8951e44ec619fffc4a304f37778625f8883bab4, expected branch
codex/client-3-production-authority. All seven worktrees and four protected
untracked files remain present. Full file/hash/status inventory is local at
.git/performance-p2-baseline.json. AGENTS.md and ARCHITECTURE.md are byte-identical
to the full P1 reading; the P1 acceptance/audit/plan/baseline/direction are the
technical input to this design.

Observed: registry 5 owns memoryMib (512–32768, default 2048), raw custom text,
window and Minecraft/loader intent; launcher config 7 owns no JVM preferences.
Structured launch order is Xmx, Mojang JVM groups, loader contribution, logging,
custom arguments, main class, game arguments. Fabric contributes no tuning.
No Xms, GC selection, pause target or worker-count default exists. Managed Java
is selected/verified by the exact official game plan, never system Java or paths
from Svelte. Existing Save/validation/final launch boundaries remain authoritative.

The legacy argument parser is lexical: blocked direct heap/classpath tokens, quoted
strings with Windows-like backslash rules, bounded length/count, no shell. It does
not comprehensively handle indirection, incompatible collectors or property overrides.
Changing its accepted vocabulary for existing configurations would be incompatible.
Production knows OS/architecture but has no RAM recommendation service. P1's local
Windows inventory is not reusable as another machine's hardware data.

No actual gameplay baseline, validated presentation capture, workload/mod/route lock
or isolated authenticated game trial exists. Owner load and other hardware/Linux
coverage remain blockers. Heap/GC candidates are hypotheses, not validated wins.
P1 recommends a Rust-owned per-instance policy with explicit draft/Save, preserved
custom text, no automatic configuration changes, conservative fallback and evidence
before optimization claims.

P2 explicitly authorizes functional configuration modes without measured improvement
claims. This resolves P1's evidence gate by implementing advice/intent only, not
performance tuning. Performance equals Balanced until evidence supports another
policy. Initial heap remains unsupported. New-mode advanced vocabulary is bounded;
legacy configuration retains its original effective behavior. GC tuning controls
are BLOCKED pending exact-build compatibility validation, rather than accepting
arbitrary flags or executing them as a validation command. No unresolved contradiction
requires a data-safety or launch-correctness workaround.
