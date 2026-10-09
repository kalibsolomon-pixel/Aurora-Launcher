# P0.1 preflight evidence

The [P0.1 report](../../PERFORMANCE_RESEARCH_P0_1.md) records an owner-interaction
boundary, not a completed packaged benchmark. No packaged trial or new profiling
recording ran. Existing Launcher/Minecraft processes were only enumerated by
process name and parent ID; no window interaction, process termination, credentials,
arguments, account files, server history or network payloads were inspected.

| Artifact | Contents |
|---|---|
| `evidence/preflight.json` | Source/build reconciliation, binary digests, declared version, process counts and isolation preconditions |
| `evidence/protection-verification.json` | Counts from final hash comparisons; path-level details remain private |
| `private/launcher-baseline.json` | Complete initial status, branch, HEAD/history, tracked and protected untracked inventory, hashes and live remote queries |
| `private/client-baseline.json` | Same for Client |
| `private/verification-details.json` | Local path-level verification, never committed |

The private directory is ignored. P0's artifacts and tools remain unchanged.
`snapshot.py` reuses P0's snapshot implementation but excludes only the new P0.1
directory; it protects all pre-existing P0 tools and evidence. Run snapshots before
work, never overwrite an inventory being used as a preservation baseline. A new
investigation needs a separate destination. This P0.1 wrapper accepts only its
ignored private directory and refuses to replace either existing baseline.

```powershell
python docs/performance-p0-1/snapshot.py docs/performance-p0-1/private
python docs/performance-p0-1/preflight.py
python docs/performance-p0-1/verify.py
```

These commands are read-only with respect to the owner repositories and managed
content; outputs are limited to this new research directory. `preflight.py` is a
specific P0.1 reconciliation tool, not a general launcher or a timing driver. It
does not launch a binary or establish source provenance for an old local executable.
`verify.py` protects all pre-existing tracked/untracked files and additionally
compares owner content to the retained P0 inventory when present. A difference
stops verification; it never triggers cleanup or automatic restoration.

The report contains the prerequisite owner procedure, separate test-profile
validation gates, timing schema, proposed instrumentation points and trial matrix.
The isolated packaged instrumentation design remains unimplemented and unvalidated.
Do not launch a release executable with `AURORA_DIAGNOSTIC_DATA_ROOT` expecting
isolation: that override is compiled out of release builds.
