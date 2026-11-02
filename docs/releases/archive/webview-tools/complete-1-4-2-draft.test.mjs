import test from "node:test";
import assert from "node:assert/strict";
import { inspectDraft, inspectProvenance, recovery } from "./complete-1-4-2-draft.mjs";
import { sha256 } from "./updater-release.mjs";
const bytes = Buffer.from("diagnostic fixture, not installer bytes");
const files = [{ name: "accepted.exe", bytes }];
const asset = { id: 1, name: "accepted.exe", size: bytes.length, digest: "sha256:" + sha256(bytes) };
const draft = () => ({ id: recovery.releaseId, tag_name: "v1.4.2", target_commitish: recovery.sourceSha, draft: true, immutable: false, prerelease: false, name: "Aurora Launcher 1.4.2", body: "accepted notes", assets: [] });
test("empty fixed draft proposes only accepted missing bytes; complete draft proposes nothing", () => {
  assert.deepEqual(inspectDraft(draft(), null, "accepted notes", files), files);
  assert.deepEqual(inspectDraft({ ...draft(), assets: [asset] }, null, "accepted notes", files), []);
});
for (const [label, change] of [
  ["different ID", { id: recovery.releaseId + 1 }], ["wrong source", { target_commitish: "a".repeat(40) }],
  ["wrong version", { tag_name: "v1.4.3" }], ["public release", { draft: false }], ["immutable", { immutable: true }],
  ["prerelease", { prerelease: true }], ["wrong name", { name: "different" }], ["notes drift", { body: "changed" }],
  ["extra asset", { assets: [{ ...asset, name: "foreign.exe" }] }], ["duplicate asset", { assets: [asset, asset] }],
  ["hash drift", { assets: [{ ...asset, digest: "sha256:" + "0".repeat(64) }] }], ["size drift", { assets: [{ ...asset, size: 1 }] }],
]) test(label + " blocks completion", () => assert.throws(() => inspectDraft({ ...draft(), ...change }, null, "accepted notes", files)));
test("existing production tag blocks a draft mutation", () => assert.throws(() => inspectDraft(draft(), { object: { sha: recovery.sourceSha } }, "accepted notes", files)));
test("expired/rebuilt/wrong-run artifact and wrong key fail provenance", () => {
  const artifact = { id: recovery.artifactId, name: "aurora-launcher-1.4.2-nsis", expired: false, digest: recovery.artifactDigest, workflow_run: { id: recovery.runId, head_sha: recovery.sourceSha } };
  const run = { id: recovery.runId, head_sha: recovery.sourceSha, event: "workflow_dispatch" };
  // No production key is needed by these negative fixture checks.
  for (const bad of [{ expired: true }, { id: 1 }, { digest: "sha256:" + "0".repeat(64) }, { workflow_run: { id: 1, head_sha: recovery.sourceSha } }]) assert.throws(() => inspectProvenance({ ...artifact, ...bad }, run, "fixture key"));
  assert.throws(() => inspectProvenance(artifact, run, "fixture key"));
});
