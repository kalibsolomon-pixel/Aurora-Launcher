# F3R curated acceptance record

Windows x86_64, 2026-10-12. Branch `codex/client-3-production-authority`.
Baseline `db2d18e`; final implementation `82225f7`; final closing commit is
recorded in the local `final-head.txt` after commit. No push or publication.

This small index is committed. Raw logs, executables, hashes and isolated game
trees remain local diagnostics; they are intentionally not Git artifacts.

## Checks

| Command | Result | Local log |
| --- | --- | --- |
| `npm run check` | PASS, 0 errors / 0 warnings | `frontend-check-final.log` |
| `npm test` (the equivalent `node --test` runner) | 266 passed, 0 failed; 139.271s | `frontend-tests-final.log` |
| `npm run build` | PASS, static production output | `frontend-build-final.log` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS, final source | `rust-fmt-closing.log` |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | PASS, final source | `rust-check-closing.log` |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | 941 passed, 0 failed, 30 ignored; 54.07s, before final leaf guards | `rust-lib-closing.log` |
| `cargo test --release --manifest-path src-tauri/Cargo.toml` | 941 unit + 6 integration passed, 0 failed, 30 ignored; unit execution 54.25s, compilation 4m52s | `rust-tests-closing.log` |
| `npm run tauri build -- --no-bundle` | PASS, final source; 7m54s including build-directory wait behind the test compilation | `production-build-closing.log` |
| `cargo build --manifest-path src-tauri/Cargo.toml --example f3r_acceptance` | PASS, final source, 30.15s | `native-runner-closing-build.log` |

The release test profile preserves the owner's running debug executable.
No installer signing, packaging deployment or release publication was performed.
The ignored tests retain their existing opt-in status. Native engine acceptance
below independently exercises real official installations and processor bytes.

## Native evidence

Root: `C:\Users\kalib\AppData\Local\Temp\aurora-f3r-acceptance-b1ef57c9-68dc-4cfe-ba93-8e1cef17a8c7`.
The exact root marker is `F3R isolated native acceptance`. Setup copied 42,654
files / 6,871,641,577 bytes from the read-only F1C test source in 98.015s.

`native-acceptance-closing.log`: PASS for independent native engine processes.
The isolated root's `native-evidence/results.json` records each process,
checkpoint, UUID, stage, old/new manifest/tree hashes, directory existence and
replay result. The opt-in example links the production library; the observer
parks and the driver kills the exact process. These are real OS process deaths,
not unit-test exceptions.

- Same target: holder 21712, busy contender 27256 (16ms); recovery 22332 passed.
- Independent Vanilla target 20940 reached Prepared while Forge stayed locked.
- AfterBackup 9616 / transaction `1409602ff2ab4544bf92e13eb85ac33e`:
  recovery 18308 restored old; repeat 25748 passed.
- AfterPromotion 20432: recovery 25100 restored old; repeat 24996 passed.
- AfterCommit 25704: recovery 18100 kept new; repeat 6860 passed.
- Recovery interruption: 26224 at Restoring, 3184 after quarantine, 19032 after
  restore; finish/repeat processes passed each case.
- Actual processor kill: runner 27152, retained live Java handle 18416. Java
  exited after launcher termination. Journal Prepared, receipt processing,
  original manifest unchanged, no staged completion marker. Recovery passed.
- Final-source post-kill retry: transaction `e4504e9efb0c4e129aaa1c459f011a0e`,
  Ready; 3,915 cache hits / zero downloads. `native-warm-retry-closing.log`.
- Seven loaders all deeply Ready with no problems in `final-all-loaders.log`
  under the native evidence root; final-source revalidation is recorded in
  `native-all-loaders-closing.log` (96.530s).
- Quiet-host, uncontended final-source repeat: transaction
  `f8b90b366fca4575af343b4459f8519d`, Ready in 128.348s. Installer 111.066s,
  staging validation 27.320s, commit 51.379s, target lock 388µs, journal writes
  0.912–1.545ms; 3,915 cache hits / zero downloads. Managed Java reused after
  checking 403 files / 91,160,397 bytes in 1.510s. `native-warm-quiet.log`.
  No before/after cold-install overhead percentage is inferred.

Processor detail: `native-processor-kill-final.log` and isolated
`native-evidence/{processor-result.json,processor-recovery.log}`. Each driver
keeps all prior failed-attempt logs and only terminates handles it created.

The full crash matrix binary predates the final directory/leaf hardening;
closing automated tests and final-source retry/revalidation cover that delta.
Full Tauri GUI A–D fault acceptance is not established by the engine harness.

## Desktop observation

Separate diagnostic Tauri app PID 1560 booted against isolated `boot-managed`
and `webview-boot`, logged backend Ready and materialized default config.
`native-boot-final.stderr.log` and `desktop-boot.txt` record boot plus actual
Home accessibility: “No instances yet,” “Not signed in,” Create instance,
Settings and normal window controls. No authentication interaction occurred.
The original owner debug PID 25212 and F1C diagnostic PID 5020 stayed alive.
Final-source desktop build: `cargo rustc --manifest-path src-tauri/Cargo.toml
--bin aurora-launcher --features tauri/custom-protocol -- -o <isolated-executable>`
passed in 26.83s with a diagnostic-only TAURI_CONFIG merge (empty updater key,
isolated WebView, diagnostic title). PID 4348 booted to backend Ready against
`boot-managed-closing`, rendered Home and stayed signed out. Logs:
`desktop-closing-build2.log`, `desktop-closing.stderr.log`, `desktop-closing.txt`.
The first invocation used the wrong feature spelling and failed before compiling;
that log is retained separately. No repository build configuration changed.

## Preservation

`preservation-closing.json`: PASS. SHA-256/size comparison found no differences
in 15,230 pre-existing untracked files (2,445,086,902 bytes), or in all 42,654
original F1C managed files (6,871,641,577 bytes). The five copied instance files
outside game/staging/history (5,504,125 bytes, including user/Aurora content)
also match. Every original tracked file and critical build/document file is
present; no baseline deletion diff exists.

The helper initially expected `vite.config.ts`; baseline inventory actually
tracks `vite.config.js`, which is present. The original result and correction
are retained. This corrected a verifier filename assumption, not repository
content or a failed hash check.

Existing auth credentials, normal owner instances, `.minecraft`, production
installation, caches and prior diagnostics were not destructive targets.
Generated binaries, games, runtimes and build output were not staged.

## Scope and verdict

F3R PARTIAL: Windows engine crash/replay evidence passed, final source regression
passed, desktop boot observed. Linux execution and two full desktop GUI fault
scenarios remain unproven. No universal power-loss, network-filesystem or actual
host-disk-exhaustion guarantee. Disk-full/cross-device journal errors are injected;
Windows file-lock failures are real. All old/rejected game backups are retained.
F1C authenticated Forge 1.20.1/1.20.4 gameplay remains independently pending.
