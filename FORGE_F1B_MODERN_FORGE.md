# F1B — Modern Minecraft Forge integration

F1B report, 2026-10-11 (America/New_York). Branch
`codex/client-3-production-authority`, starting HEAD `13d6101` (the F3
final state). Local commits: `40109be` (implementation, 19 files,
+4,445/−53), `a304332` (acceptance-discovered lifecycle fixes), plus the
closing documentation commit containing this report.

**F1B verdict: COMPLETE** — the owner subsequently confirmed authenticated
Forge launch, working Minecraft gameplay, normal exit, return to Ready,
and no further observed issues. This is owner-reported acceptance supplied
on 2026-10-12, not gameplay independently observed by the agent or newly
established by machine logs. The original installation and automated
evidence below remains separately attributed: a real, officially supported modern
Forge release installs end-to-end through the official pipeline, its
processor chain executes under the unchanged F1A boundary with every
output verified against Forge's own published SHA-1s, the instance
reaches Ready with deep validation passing, existing loaders are
unaffected (natively and across 913 automated tests), and no schema
migrated, no production Launcher replaced, nothing pushed.

## 1–3. Verdict, HEAD, commits
As above; final HEAD is the documentation commit after `a304332`.

## 4. Exact supported versions

- **Primary (natively verified):** Minecraft **1.21.11** + Forge
  **1.21.11-61.2.0** — the official promotions document's
  `1.21.11-recommended` build at implementation time; installer
  9,330,490 bytes, sidecar SHA-1
  `ca682f0d537c65e9a112e1e7ab337095d486298f` (verified against the
  live repository).
- **Fixture generation (deterministically tested, not natively
  installed):** Minecraft 1.21.1 + Forge 1.21.1-52.1.16 — the F0
  research sample, same installer-generation shape (spec 1,
  ForgeBootstrap, three client processors); installer SHA-1
  `076aad17209fe7f153887f7f3992506ab58472f5` re-verified live.
- **Negative matrices:** versions with no recommended promotion (e.g.
  1.21.6 → automatic policy refuses), a Minecraft version with no Forge
  builds (empty listing), an invalid combined Forge pin (`47.4.26`),
  spec≠1 profiles, non-`/version.json` embedded documents, malformed
  data values, unknown output tokens, wrong published digests, unsafe
  paths, and a declared per-loader logging configuration (deliberately
  unsupported).

Scope is the modern ForgeBootstrap generation (the 52.x/61.x spec-1
line). Older mapping-pipeline Forge (1.20.x and earlier), LaunchWrapper
generations, and server installs are explicitly out of scope and
rejected/absent, not silently half-handled.

## 5. Sources and provenance

Discovery: `https://maven.minecraftforge.net/…/maven-metadata.xml` (full
listing) + `https://files.minecraftforge.net/…/promotions_slim.json`
(stability). Installers and libraries from `maven.minecraftforge.net`
with `.sha1` sidecars; launch libraries carry inline official SHA-1s in
`version.json`. No third-party mirrors, no frontend URLs, no invented
digests: the one artifact with no upstream URL — the patched client —
exists only as the processor chain's output, verified against the
SHA-1 Forge publishes for it.

## 6–7. Installer formats; unsupported generations

Implemented (verified against the two real installers' documents):
spec-1 profiles; side-mapped `data` with the three real value forms
(bracketed Maven references incl. `@tsrg`/`@zip` extensions,
single-quoted literals, `/`-prefixed installer entries);
brace-wrapped `outputs` keys resolving through the data map to
published SHA-1s; `sides`-filtered processors (server-only steps never
run on client installs); the `version.json` launch profile
(`inheritsFrom`, mainClass, JVM/game arguments, libraries, empty
`logging`, `_comment`); tool libraries from the profile's library list.
Unknown mandatory shapes — spec ≠ 1, non-`/version.json` documents,
unknown data-value forms, a declared logging configuration, unknown
output-token references — fail as explicit unsupported/malformed
errors. Parser fixtures are abridged verbatim shapes of the real
documents.

## 8–10. Planner, processor chain, output verification

`forge::plan::ForgePlan` composes through `game::plan` as the `Forge`
`LoaderPlan`/`GameLibrary` variant (added in F3's closed enums): Mojang
libraries first then Forge's version-profile libraries, exact-duplicate
collapse, identity conflicts hard-fail; Mojang's Java requirement stays
authoritative (Forge publishes no floor); Forge's JVM/game arguments
join the composed effective groups; the platform pin derives as
`forge/<mc>-<build>`.

The client chain — `DOWNLOAD_MOJMAPS` → ForgeAutoRenamingTool (client
side) → binarypatcher — runs through `forge::processors` on the
unchanged F1A runner: private SHA-1-re-verified installer copy (the
shared store is never a tool argument), regular-file inputs hashed
before and after each step, link-aware before/after snapshots,
concurrent bounded output capture, per-step wall-clock timeout,
cooperative cancellation, process-tree termination, receipts at every
stage. Chain integrity: each step's published outputs verify against
their official SHA-1 before the next step runs; a failing step stops
the pass; a crash between steps leaves the receipt at `processing` and
the retry rebuilds without trusting partial output. Generated outputs
that matched published digests are recorded `expectedDigestVerified(sha1)`
(the digest was known in trusted metadata before the step ran); the
natively installed manifest proves all three chain outputs plus the
universal carry exactly Forge's published digests.

One honest verification note: the mappings step performs its own Mojang
download inside the supervised process; its result is verified against
the digest Forge published before the step ran, which is the extent of
that step's verification (same-user execution; no network sandbox is
claimed).

## 11. Java runtime behavior

`GameJavaRequirement::without_loader_floor` — Minecraft 1.21.11's
`java-runtime-delta`/21 governs both the game and the processors; the
lifecycle ensures the managed runtime before installation (the
acceptance run really installed it). No Java 21 hardcoding beyond the
game document's own requirement; unsupported combinations fail through
the existing runtime resolver.

## 12. Launch specification

`LaunchPlan::from_game_plan` consumes the composed views unchanged:
main class `net.minecraftforge.bootstrap.ForgeBootstrap`,
`-Djava.net.preferIPv6Addresses=system` JVM group,
`--launchTarget forge_client` game group, classpath from the composed
library set (patched client included as an ordinary classpath library)
with the vanilla client last, structured arguments, redaction intact —
all covered by deterministic tests. Authenticated launch and gameplay were
subsequently confirmed by the owner; the agent did not observe that playthrough.

## 13. Instance lifecycle and UI

Creation, pinned and automatic version selection (automatic = the
promotions-recommended build; no recommendation → explicit refusal),
installation, deep validation, Ready, retry — all natively exercised.
Forge appears in the platform catalog and the create/settings forms
with its own version combobox (live listing with honest stable/unstable
markings); Aurora remains Fabric-exclusive and disables itself on
Forge. Compatibility evaluation maps the Modrinth `forge` slug and the
Maven-range grammar (shared with NeoForge's evaluator, keyed by
family). Error codes added: `forge_metadata_unavailable`,
`forge_resolution_failed`.

## 15. Schema compatibility

No persisted schema changed: registry 5 (existing Forge pin
representation round-trips — proven by the pre-existing
future-platforms test, now with real combined ids), installed-game 2
(the validator's platform vocabulary admits Forge pins), receipt 1.
No migration of any kind.

## 16. Automated tests

913 lib tests, 0 failed (29 pre-existing ignores) + 6 icon tests;
`cargo fmt --check` clean; `cargo check --all-targets` clean;
`svelte-check` 0/0. New coverage: 24 forge metadata/plan tests over the
verbatim-shaped real documents (discovery/promotions stability,
combined ids, processor/output/data semantics, negative matrices,
jar-embedded reading and extraction), plus the two full-install tests —
a synthetic-but-faithful world where the real JDK runs the real
three-step chain through the loopback fixture (chained outputs,
published-digest trust classification, valid commit), and a
failing-chain test (nonzero step → no commit, no complete-looking
state, honest retry recovers). The pre-existing F1A/F3 suites
(processors, receipts, recovery, composition, launch) all pass
unmodified.

## 17. Native acceptance

See `.zcode-diag/phase-f1b-modern-forge/evidence-index.md` and the five
screenshots. Real installation of Forge 1.21.11-61.2.0 on Minecraft
1.21.11 through the live official pipeline to Ready with deep
validation passing; published-digest proof in the committed manifest;
two genuine mid-acceptance failures each left honest retryable state
(the fixes are commit `a304332`); a cache-warm Vanilla 1.21.11 instance
in the same root confirms existing loaders natively. The original agent run
did not perform authenticated gameplay. The owner subsequently reported a
successful authenticated launch, working gameplay, normal exit, return to
Ready, and no further issues, closing that acceptance gate.

## 18. Performance

Cold root to two Ready instances ≈ 20 minutes, dominated by the
4,727-object asset set (~558 MB) and the Java 21 runtime; processor
chain ≈ 90 s; cache-warm Vanilla regression ≈ 15 s; no additional
metadata fetches beyond the two discovery documents (listing +
promotions) and the installer sidecar.

## 19. Security limitations (unchanged from F1A, restated)

Processor execution remains same-user code with bounded, verifiable,
recoverable supervision — **not** an OS-enforced filesystem/network
sandbox. The mappings step performs a supervised network download whose
result is digest-verified against the published expectation. No second
runner exists; no shell is involved; no generated output is treated as
upstream-verified without an authoritative published digest (with one,
it is — honestly and by name).

## 20. Preservation

Production Launcher 1.4.2 untouched (not replaced, not launched by this
work; release authority unchanged); owner data/instances/auth/caches
untouched (isolated root only); developer-local files and all prior
phase evidence preserved; `git diff --name-status` reviewed before each
commit with zero deletions; no push, no publish. Final tracked tree is
clean except this report (committed with the documentation commit).

## 21–22. Remaining decisions and next steps

The owner has closed authenticated gameplay acceptance and authorized F1C
investigation and implementation for Minecraft 1.20.1 and 1.20.4. Older
generations remain investigation-only. The F0 OS-isolation/trust-policy
question remains the standing caveat on same-user processor execution.
Next: bounded F1C expansion with exact official pins, native installation
evidence and separately attributed gameplay acceptance for each target.
