# F1C local evidence index — 2026-10-12

Scope and verdict: see `docs/compatibility/FORGE_F1C_COMPATIBILITY_EXPANSION.md` at repository root.
This index is the only curated diagnostic file committed for F1C. Executables,
raw installers, logs, screenshots, managed caches and game/runtime data remain
local evidence, not source/release artifacts. No secret-bearing launch arguments
or authenticated token material is included.

| Evidence files in this directory | Purpose / result |
| --- | --- |
| `baseline-status.txt`, `baseline-history.txt`, `baseline-tracked.txt`, `baseline-critical.json`, `baseline-untracked-hashes.json` | Starting branch/HEAD/status, original 824 tracked paths, critical-file presence and pre-existing untracked byte hashes |
| `preservation-final.json` |15,169 pre-existing untracked hashes unchanged; zero missing tracked/critical files |
| `research.log`, `upstream/<exact-pin>/summary.json` and official installer/profile files | Official promotions, installer byte/hash checks and generation analysis; executable inputs were verified before inspection |
| `baseline-rust-lib.log`, `baseline-rust-tests.log` |913 baseline library passes; full debug test encountered preserved running executable lock |
| `baseline-frontend-tests.log`, `frontend-tests-sequential.log` | Default parallel SSR setup stalled; same 265 baseline tests passed sequentially |
| `frontend-check-final.log`, `frontend-tests-final.log`, `frontend-build-final.log` |0 type errors/warnings;266 tests passed/0 failed/0 skipped; production static build passed |
| `rust-fmt-final.log`, `rust-check-final.log`, `rust-lib-final.log` | Format/check passed; 923 unit passes/0 failed/30 ignored |
| `rust-tests-release-final.log` | Full release-profile suite 923 unit +6 integration passes,0 failures,30 ignored |
| `native-production-build-final.log` | Final `npm run tauri build -- --no-bundle`; includes frontend build and native release build |
| `owner-build-final.log`, `owner-app-final.stderr.log`, `owner-app-final-pid.txt` | Final custom-protocol diagnostic desktop build and real backend boot into owned isolated root |
| `final-native-instances.jpg`, `final-native-instances.txt` | Final real desktop UI: exact Forge 1.20.1/47.4.10 and1.20.4/49.2.0 both Ready; regression instances Ready; no account signed in |
| `native-acceptance-resume2.log` | Seven official native installs/deep validations passed,404.24s; initial acquisition was resumed, not a cold benchmark |
| `interrupted-children.json`, `interrupted-state.json`, `native-interruption.log` | Exact real managed Java child observed; runner interrupted; child ended, registry Installing, incomplete processing receipt and no staged completion marker |
| `native-recovery-final.log` | Retry/reinstall, all seven deep Ready, both real Java 17 checks, native-file launch assembly, user sentinel preservation and warm-cache counters;288.67s |

Retained acceptance root:
`C:\Users\kalib\AppData\Local\Temp\aurora-f1c-acceptance-feaa62d6-705c-467e-99dc-e186e4d08201`.
Managed root is its `managed` child; WebView uses its separate `webview` child.
The root has a phase-owned marker. Existing user managed-data/credential roots
were not reused. Final isolated executable:
`C:\Dev\aurora-launcher\.zcode-diag\phase-f1c-compatibility\aurora-f1c-owner-final.exe`.
It is a diagnostic build, not a distributable release. Launcher 1.4.2 and the
pre-existing debug launcher process were preserved.

Reproducible deterministic commands are in report section13. The explicit live
test is `instances::lifecycle::tests::forge_f1c_native_acceptance`, ignored by
default. It creates a fresh owned direct temp child unless `F1C_ACCEPTANCE_ROOT`
identifies an existing marked phase root. Network acceptance must be deliberate;
normal tests use deterministic fixtures. The final live run executed the copied
test binary `native-runner-final.exe` with `--ignored --exact` and `--nocapture`;
`AURORA_INSTALL_DIAGNOSTICS=1` enabled non-secret cache/timing counters.
Fixture-session launch assembly never spawned Minecraft or wrote account state.

Pending owner evidence: authenticated menus, disposable-world rendering /
movement/input, normal exit and Launcher Ready for both new Forge versions.
F1B owner-confirmed gameplay is recorded separately; it does not prove F1C.
