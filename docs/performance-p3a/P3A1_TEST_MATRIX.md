# P3-A.1 verification

Observed 2026-10-16, Python 3.12 on Windows. No assertion, importer, measured-run
schema or threshold was weakened. No new dependency. No product code changed.

| Check | Result |
| --- | --- |
| Original P3-A tooling suite | 36 PASS, including actual read-only Windows self-process plumbing |
| New snapshot/isolation tests | 8 PASS, synthetic snapshots/principal inputs only |
| Benchmark suite total | 44 PASS, zero failures/skips |
| Original P1 analyzer suite | 13 PASS, zero failures |
| Historical synthetic CLI fixture | Exit 0, VALID WITH WARNINGS; remains synthetic |
| PresentMon exact asset integrity | Official SHA-256 and 421,312-byte size match before execution |
| Windows Authenticode | Valid Intel Corporation signature with timestamp signer |
| Actual tool identity | Help banner 2.3.1, exit 1; no trace |
| Actual PresentMon CSV compatibility | BLOCKED — no real samples |
| Real world manifest / restoration | BLOCKED — no created world; synthetic validator tests only |
| Live test-user token / Credential Manager | BLOCKED — owner says not provisioned |
| Native launcher/game acceptance | NOT TESTED this phase; no test app/game started |
| Rust/frontend/build regressions | Not rerun: no production source, startup, boundary, dependencies or build changes |

Commands:

```powershell
python -B -m unittest discover -s tools/benchmark -p 'test_*.py' -v
python -B -m unittest discover -s docs/performance-p1/jvm-benchmark -p test_analyze.py -v
python -B tools/benchmark/quality.py docs/performance-p3a/evidence/synthetic/run.json
git diff --check
```

The original suite covers PresentMon integer-QPC units/normalization, PID/swap-chain/
version identity, undisplayed-row retention, malformed/nonfinite/duplicate timestamps,
coverage/completeness, capture/run metadata, artifact digests/containment, G1 shrinking
heap/rounded CPU, counter sampling/creation identity/exited handle and INVALID/
INCOMPLETE retention. New tests cover world metadata and exact full snapshot/restore
inventories, missing/extra/tampered files, empty directories, unsafe/reserved names,
case collisions, duplicate JSON, schema/count/byte bounds, reparse refusal, generic
error privacy, equal/wrong principal and unknown/elevated-context refusal.

Neither passing synthetic fixtures nor caller-supplied SID tests prove a live OS
credential namespace, a real NBT scene or real Minecraft capture. P3-A's 992 Rust,
40 focused frontend and native restart results remain historical, not new runs.

Private retained logs and exact-byte SHA-256 at the recorded test run:

| Artifact | SHA-256 |
| --- | --- |
| `.git/performance-p3a1-tools-tests.txt` | `53339e3f52423671fe85dd4dca0c8c0570aa6a0f7c5360deb1be7267375983e0` |
| `.git/performance-p3a1-p1-tests.txt` | `cb8f3a42add933c897d599b9c6694fd25ee3227c75af84b4379daaa39b80cc30` |
| `.git/performance-p3a1-synthetic-summary.json` | `260dadb49150d59acd3aa2ebbc5312d5886a2a9f57835f3874263eb8296ebce4` |

Final preservation/deletion/index/link checks are recorded in acceptance and the
private final record. Raw executable/help/license stay in the new tools directory;
no binary, build output, game, runtime or personal credential evidence is staged.
