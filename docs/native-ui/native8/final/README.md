# NATIVE-8 Windows acceptance evidence

The final matrix uses the optimized acceptance executable whose hash is in
`builds.json`, a signed-out populated disposable root, 1120×760 logical pixels
(1400×950 physical pixels at 125% scaling), and three runs of each scenario.
Every resource interval is approximately 10 seconds; memory is sampled every
250 ms. CPU percentages in the raw data explicitly distinguish one logical
processor from the whole 28-processor machine. System/DWM and benchmark-helper
resources are outside Launcher process accounting. No child processes were
observed at interval ends; the probe rejects unexpected children.

`matrix/resources.json` retains process creation identity, HWND/owner, state,
CPU time and memory samples. `matrix/*.trace.txt` contains numeric events only.
`matrix/validated.json` records all 33 validated intervals, cadence and pixel
comparisons. Paused phases have zero ticks; resumption preserves the frozen
phase/update count; active intervals contain 18 ticks in the interior window.
One reusable timer advances approximately every 500 ms. Ordinary foreground
captures establish actual visible motion. Optional forced-active intervals
exercise the foreground bypass and hidden/minimized guard; they are not used as
proof of visible animation or literal zero-cost rendering.

`ui-*.png` records the interactive UX pass. The general UX captures precede the
final small card-fit/copy correction; `ui-instances-corrected.png`,
`ui-instances-minimum.png`, `ui-settings-corrected-minimum.png` and the matrix
use the final executable. Both binary hashes and this distinction are recorded
in the root acceptance report. Media and account-bearing screenshots are not
included. The original private refinement images remain ignored in their
original local directory.

`checks/` contains sanitized command output. The two controlled install probes
use synthetic loopback responses and an intentional 18 ms per-request delay.
Those results separate cold acquisition, warm reinstall, second instance,
materialization/extraction/validation and runtime reuse. They do not establish
real internet throughput. The original real-install evidence remains one level
above in `install-acceptance-results.json`, including failed attempts.

Reproduce with PowerShell 7 and an existing disposable populated root whose
parent contains `NATIVE8-DISPOSABLE.txt` (never point at owner data):

```powershell
cargo build --release --manifest-path native/Cargo.toml --features disposable-data-root
rustc --edition=2024 -O -C strip=symbols native/tools/focus-sink.rs -o <temp>/focus-sink.exe
native/tools/native8-benchmark.ps1 -Exe <acceptance-exe> -FocusSink <temp>/focus-sink.exe -DataRoot <disposable>/managed -Out <new-evidence-dir> -Runs 3 -Seconds 10
native/tools/validate-native8.ps1 -Evidence <new-evidence-dir>
```

Windows may refuse activation by the background probe. When the probe emits an
`activation-request.json`, activate the uniquely matching returned window through
the native helper (or manually), verifying its HWND against the request. The
neutral `Aurora benchmark focus sink` is test-owned and is used to unfocus the
visible, restored Launcher. Never substitute minimization for this state or
use the foreground override to manufacture an ordinary active sample. The
probe rejects stale/ambiguous windows or incorrect states and restores original
configuration bytes in `finally`. Keep the measurement free of build/test work.

Historical and rejected harness attempts remain privately preserved under the
continuation temp directory; only the final validated matrix is acceptance data.
See `docs/native-ui/NATIVE_8_ACCEPTANCE.md` at the repository root for interpretation and limits.
