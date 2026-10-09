# P1 startup readiness evidence

The optimization is the independent commit
`0d68340f0e280978f675c6f83330a10fb97bf678`. The baseline is the unchanged P0.2
instrumentation commit `6cbabd38bc1c5268b9f6c80034f18da3a26f1e21`.
The report is [PERFORMANCE_OPTIMIZATION_P1.md](../../PERFORMANCE_OPTIMIZATION_P1.md).

## Reproduction and boundaries

Use a fresh private OS-temporary directory starting `aurora-p0-2-p1-`, an isolated
checkout at the optimization commit, and the retained P0.2 artifact directory.
`build.py` verifies the approved updater public-key digest, copies the previous
disposable compiler cache into a new target, verifies/copies all three baseline
executables, and builds candidate research, feature-off production and separate
IPC artifacts. It never installs an application or changes old artifacts.
Its four arguments are the new output root, source checkout, old P0.2 research root
and a disposable compiler-cache source. Two bounded concurrent builds use separate
byte-checked source copies and targets; no build activity overlaps primary trials.
The public-key configuration is reused without changing trust or release identity.

Before substantial work, `preserve.py capture` records complete tracked/untracked
file hashes, Git identity/status/history/remotes, every existing managed file and
the installed executable in ignored `private/`. `preserve.py verify` allows only
the requested store and architecture changes among pre-existing repository files.
The first packaged pilot's 40 changed managed files were investigated: WebView
profile/cache/log writes and derived server-enrichment cache refreshes from normal
startup. Those operational domains are counted separately, never described as
byte-preserved. Every other managed file remains protected. It reports
differences; it never restores or deletes. Historical managed hashes are also
checked independently. Private inventory paths and full build/test logs are not
committed. No credentials or process argument lists are read by these tools.

For each start, `start-trial.ps1` refuses any competing Launcher/Java, then
`start-trial.py` starts exactly one structured executable with application stdio
sent to the OS null sink. OS creation time comes from Windows process times.
The capture opt-in is passed only to that child. Trial directories are exclusive.
Use `ui-observer-notebook.js` under the Computer Use skill, with `sky` and `fs`
initialized. Pass the research root explicitly to `p1observe(cohort, trial, root)`
to avoid retaining a prior notebook binding. Observe one process, inspect its sanitized result, and close
in a separate call through the normal window shortcut. No Play or owner mutation
is part of this protocol. Raw UI text/screenshots are never retained.

Poll with a 60 ms delay, activate each uniquely discovered window promptly, and
observe for at least eight seconds. Ready requires Play/Ready without Checking;
sustained Ready requires a one-second hold, reset by any later non-Ready state.
The eight-second observation prevents an early hold from hiding late runtime
completion. Observational timing includes polling, accessibility and scheduling
uncertainty. Native/frontend clocks are reported separately from OS creation.

`analyze.py <research-root>` validates traces through the unchanged P0.2 numeric
schema analyzer, copies only numeric traces, and writes `evidence/trials.json` and
`evidence/summary.json`. It requires successful UI outcomes, recorded normal
window closes, complete non-overflow traces, no Play/spawn spans and no trace from
feature-off controls. Window close plus completed trace is not a collected process
exit code. It retains pilots separately and emits no p95 below 20 observations.

The parent-zero background audit remains unattributed with this probe set. No
background work, hash verification, metadata resolution, inventory scan or native
launch validation is altered by P1. No persistent validation cache is introduced.

## Evidence index

- `evidence/*-artifact.json`: executable SHA-256, frontend file hashes/tree,
  exact source, modes/features and unchanged updater public-key identity.
- `evidence/source-identities.json`: source/tree, input/document hashes and tools.
- `evidence/source-snapshot.json`: every compiled source input's relative path/hash.
- `evidence/source-build-verification.json`: post-build source-copy and native-boundary checks.
- `evidence/environment.json`: matched host and observation conditions.
- `evidence/trials.json`: sanitized OS/UI observations, pilot/primary/diagnostic
  classifications and observed normal window closes.
- `evidence/trace-*.json`: unchanged P0.2 numeric-only record contract.
- `evidence/summary.json`: counts, audit correlations, clock-specific latency
  distributions, candidate-minus-baseline pairs, transitions and IPC diagnostics.
- `evidence/verification.json`: checks, test counts, build results and log hashes.
- `evidence/document-validation.json`: final schema/privacy/identity validation receipt.
- `evidence/preservation.json`: original-file comparison and protected-domain counts.
- `evidence/failures.json`: exclusions, observer setup failures and the Play limitation.
- `evidence/pre-correction/`: all 23 rejected-source starts, six artifact manifests
  and the reason they are excluded. Superseded source `f1e42c2` is retained on the
  local `codex/performance-p1-pre-correction` branch. Candidate trial 7 had two
  readiness requests because equal state objects were compared by identity.
  The corrected implementation compares DTO values and separately tracks mutation
  invalidation. Neither these timings nor their pilots enter final acceptance.

No P0/P0.1 historical timing is pooled with the matched P1 observations. The
known JNA preservation blocker remains a gate on Minecraft/Play acceptance.
