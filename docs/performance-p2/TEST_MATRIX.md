# P2 validation matrix

| Area | Evidence/result |
| --- | --- |
| Rust format | cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check: PASS |
| Rust all-target check | cargo check --manifest-path src-tauri/Cargo.toml --all-targets --offline: PASS |
| Full Rust tests | cargo test --manifest-path src-tauri/Cargo.toml --offline: 986 library + 6 icon tests passed, 31 existing ignored, zero failures; bin/doc targets have no tests |
| Frontend types | npm run check: zero errors/warnings |
| Full frontend suite | node --test --test-concurrency=1 "src/lib/**/*.test.ts": 282 passed, zero failures |
| Focused settings tests | Performance, instance settings/status and UI-5 settings: 40 passed |
| Frontend production assets | npm run build: PASS; no installer/package/publication |
| Native development executable | cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --offline: PASS, compiled production frontend embedded in debug executable |
| New browser interactions | [browser/results.json](browser/results.json): six groups; transitions, explicit/undoable recommendation, delayed manual-edit protection, failed-save preservation with fresh registry identity, responsive/text/keyboard, missing advice; zero page errors and ownership warnings |
| UI-1 | [11 cases](regressions/ui1/functional.json): PASS |
| UI-2 | [17 cases](regressions/ui2/functional.json): PASS |
| UI-2B / crop | [6 cases](regressions/ui2b/functional.json), [6 crops](regressions/ui2b1-crop/crop-tests.json): PASS |
| UI-3 / UI-3.1 | [11 groups](regressions/ui3/functional.json), [13 groups](regressions/ui31/functional.json): PASS |
| UI-4 | [13 checks](regressions/ui4/results.json): PASS; owner approval is not reopened |
| UI-5 | [14 checks](regressions/ui5/results.json): PASS |
| Actual native first session | [first-verification.json](native/first-verification.json): Windows detection, untouched legacy values, four modes, failed-save feedback, minimum/text200, real Rose accent: PASS |
| Actual native restart | [restart-verification.json](native/restart-verification.json): exact Custom / 3217 MiB / quoted duplicate-property text retained: PASS |
| Native exits | Both exact isolated processes exited 0; private retained-handle records in .git/performance-p2-native-*-exit.json |
| Real Minecraft performance | NOT MEASURED; no gameplay or FPS/GC/startup/CPU improvement claim |
| Linux/macOS native | NOT TESTED. Linux bounded memory adapter exists; other hosts return unknown memory. No cross-platform performance/boot claim |

New native tests cover all four targets on 8/16/32/64 GiB and very large reports,
low/unknown/inconsistent RAM, stale facts, overflow-safe arithmetic, architecture
ceilings, lexical/GC/indirection/managed-property rejection, duplicates/quoted strings,
mode transition/restart round trips, schema-5 preservation/migration, unknown fields/
modes, rejected/malformed/future-file writes and unchanged unrelated fields. The
launch resolver test proves identical vectors for every mode and Existing, with one
Xmx first and original custom order last. Existing launch/process/runtime suites stay
intact. Fixtures are synthetic and do not establish collector/vendor compatibility.

Native sessions use fresh marked direct-temp roots, a schema-5 **configuration-only
synthetic record**, no installed game and zero accounts. A stored Ready fixture is
not claimed as a deeply validated playable installation. Saves use real commands,
disk persistence and native refresh; screenshots use the actual compiled host,
never a browser fixture as a native substitute. Both sessions use the same hashed
debug executable. The old 3217 MiB and raw text survive migration and restart.

Native Windows geometry: default 1120×760 CSS and 1024×768 CSS, DPR 1.25. The native
minimum track is 900×650 physical (720×520 logical outer); actual client is 882×640
physical, with **706×512 CSS viewport**. Geometry comes from Win32 client/window
rectangles and WM_GETMINMAXINFO plus independent WebView measurements. 200% text
is test-local CSS text enlargement inside that host, not an OS-wide setting change.
Vertical scrolling remains necessary; mode, heap and advanced controls are reachable
and horizontally contained. Keyboard/forced colors/reduced motion/transparency and
alternate accents are additionally exercised by browser regressions.

Failures found and resolved: default parallel SSR tests collided in shared Vite
optimizer caches (transport timeouts); sequential full run passes. A stale visual
optimizer dependency produced HTTP 504; the final visual server uses an exclusive
temporary cache without deleting developer caches. Browser fixture proxy cloning
was corrected to serialize like the real native boundary. Actual native refresh
exposed an editor error-clearing bug, fixed by instance-ID-based initialization and
covered with fresh-registry browser and real-native rejection tests. Child edits use
parent callbacks, avoiding Svelte ownership warnings. No assertion was weakened.

All full visual captures remain in new P2 folders. To avoid committing over 100 MB
of duplicate fixture screenshots, browser/regression PNGs remain locally retained
and ignored; JSON results and the 13 native screenshots are committed. Earlier UI
evidence and developer caches are preserved. The acceptance report links the complete
changed-file and screenshot inventories. Private full command logs remain under
.git/performance-p2-*.txt. No production accounts/instances/worlds were used.
