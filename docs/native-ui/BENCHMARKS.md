# NATIVE-0 framework benchmarks

Bounded prototypes, measured on 2026-10-31. Raw per-run JSON is in
[`evidence/native0/`](evidence/native0). The probe is
[`native/tools/measure-process.ps1`](../../native/tools/measure-process.ps1);
the run matrix is [`native/tools/run-native0-matrix.ps1`](../../native/tools/run-native0-matrix.ps1).

## Conditions

| Item | Value |
| --- | --- |
| OS | Windows 11 Home 10.0.26300 (build 26300) |
| CPU | Intel Core i7-14700F (20 cores / 28 threads) |
| RAM | 31.8 GiB |
| GPU | AMD Radeon RX 9070 XT, driver 32.0.32015.7029 |
| Displays | two 2560×1440 monitors at 125% scaling |
| WebView2 runtime | 156.0.4314.50 |
| Rust | 1.98.1 (MSVC) |
| Build profile | all candidates `--release` with the launcher's profile: `lto = true`, `codegen-units = 1`, `opt-level = 3`, `panic = "abort"`, `strip = true` |
| Tauri baseline | `npx tauri build --no-bundle` of `9b99db3` (production frontend bundle, no dev server), identifier overridden to `com.aurora.launcher.native-baseline` so it never opened owner data |
| Window | 1120×760 logical (1400×950 physical), same content in every prototype |
| Runs | 3 interleaved runs per candidate (A, B, C, D, A, B, …) on an otherwise idle machine |

Prototype content (same in Slint and GPUI): 96 px navigation rail, Home with
an instance hero, four instance tiles and a Play button that drives a
3-second async progress update, a Mods list with 300 rows and toggles,
Settings with theme switching (three palettes), a toggle, a slider and a
dropdown (Slint only — see below), and a modal Accounts dialog.

Probe method: spawn the exact executable, wait for its first visible
top-level window ("window"), then until the whole process tree used < 20 ms
CPU in a 500 ms window ("quiescent"), settle 8 s, then measure a 20 s idle
window (process-tree CPU time delta, working set and private bytes of every
descendant, GPU engine utilization and GPU process memory counters), then
minimize and measure 10 s. The probe only ever closes or terminates the PIDs
it spawned. "Warm" applies to all runs: binaries and the WebView2 runtime
were already in the OS file cache. A true post-reboot cold start was not
measured.

## Results (medians of 3)

| Candidate | Processes | First window | Quiescent | Idle working set | Idle private | GPU dedicated / shared | Idle CPU (% of one core) | Minimized CPU |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| **Tauri 2 + WebView2** (production build) | 7 | 25.7 ms | 762 ms | **408.4 MiB** | **205.8 MiB** | 60.6 / 11.7 MiB | 2.52 % | 1.59 % |
| **Slint, software renderer** | 1 | 25.7 ms | 677 ms | **104.5 MiB** | **85.9 MiB** | 0 / 0 MiB | 0.36 % | 0.29 % |
| Slint, FemtoVG (OpenGL) | 1 | 25.1 ms | 1255 ms | 156.5 MiB | 390.7 MiB | 231.6 / 2.6 MiB | 0.22 % | 0.29 % |
| GPUI 0.2.2 (DirectX 11) | 1 | 166.6 ms | 829 ms | 49.3 MiB | 70.2 MiB | 34.4 / 2.7 MiB | 0.94 % | 2.60 % |

Per-run spread: Tauri working set 406–425 MiB and idle CPU 0.5–5.7 %;
Slint software 104.5 MiB in every run and idle CPU 0.29–0.58 %; GPUI working
set 49–84 MiB and idle CPU 0.8–3.0 %, minimized 2.3–2.6 % in every run.

Tauri's process tree (run 1): `aurora-launcher` 45.6 MiB working set plus six
`msedgewebview2` processes (119.6, 97.4, 85.0, 39.9, 19.0, 18.3 MiB).

"First window" is the moment a top-level window exists; for the WebView it
precedes page content, so "quiescent" (CPU settles after first layout) is
the better startup proxy. GPU engine utilization averaged 0.00 % for every
candidate at idle (counter resolution); GPU memory is reported separately.

Slint navigation latency (`--bench`: 400 programmatic state changes across
pages and the dialog, timed from the change to the end of the next rendered
frame) was p50 13.98 ms / p95 164 ms in an unoptimized debug build; it is a
sanity check, not a release figure.

## Interpretation

* The WebView host's footprint is dominated by the WebView2 process tree
  (≈362 of 408 MiB working set), not by the Rust backend (45.6 MiB).
* Slint's OpenGL renderer is a poor fit on this hardware: the AMD OpenGL
  driver commits ~390 MiB private and ~232 MiB dedicated GPU memory. The
  software renderer has no GPU allocations and the lowest committed memory.
* GPUI uses the least resident memory, but schedules frames continuously:
  it consumed ~1 % CPU idle and ~2.6 % while **minimized** in every run.
* Software rendering cost is paid only when something changes; at idle the
  Slint window does not redraw.

## Decision

**Slint 1.18 with the software renderer** (winit backend). Rationale and
rejected alternatives are in [`FRAMEWORK_DECISION.md`](FRAMEWORK_DECISION.md).

## NATIVE-6: full native launcher vs Tauri (release builds)

Raw JSON: [`evidence/native6/`](evidence/native6). Same host and probe as
above; both on empty data roots; 3 interleaved runs (medians).

| Build | Processes | First window | Quiescent | Idle working set | Idle private | GPU dedicated | Idle CPU | Minimized CPU |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Tauri 2 + WebView2 | 7 | 23.4 ms | 1615 ms | 407.2 MiB | 213.0 MiB | 73.6 MiB | 0.20–0.28 % | 0 % (one run 13.7 %) |
| Native (Slint software) | 1 | 25.3 ms | 874 ms | 106.8 MiB | 85.7 MiB | 0 MiB | 0.00 % | 0.00 % |

First runs after each build (cold-ish, not post-reboot): Tauri 48 ms / 1451 ms,
native 534 ms / 1361 ms.

Navigation scenario (`native/tools/nav-scenario.ps1`, UIA Invoke on the
sidebar, 30 cycles): Tauri 398 → 510 MiB working set, settled 472 MiB / 246 MiB
private, 8.8 s CPU for 120 switches; native 106 → 115 MiB, settled 87 MiB
private, 0.5 s CPU for 150 switches. Both returned to ≈0 % idle CPU.

Native interaction scenario on a populated root (`native-scenario.ps1`: 120
navigations, 20 account-dialog cycles, a Discover search with 20 live results):
private 89.8 → 92.8 MiB, idle CPU 0 % afterwards, UIA dispatch p50 1.1 ms /
p95 1.9 ms.
