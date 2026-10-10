import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { repository, sha256, verifyDirectory } from "./updater-release.mjs";

export const recovery = Object.freeze({
  releaseId: 408878140,
  runId: 38044389751,
  artifactId: 11666957499,
  version: "1.4.2",
  sourceSha: "cde51a00a6dbce47adbde06b361219defd963dfb",
  artifactDigest: "sha256:b9d5d10d1ee9f0f400c7c1ae3c3d39dfa3ff69d4f41fd6884625e73d20fba047",
  publicKeySha256: "bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc",
});
export const acceptedHashes = Object.freeze({
  "Aurora Launcher_1.4.2_x64-setup.exe": "2f5c9bc3cb9e7f9464151bef9835ffca26876aca9090cae7113f835fa761da2a",
  "Aurora Launcher_1.4.2_x64-setup.exe.sig": "a2ff53de467e1a882700186da0af18157ebc88e2b16f9f64794457118690b262",
  "release-assets.json": "8090ea75d67b0423037e4199bc9740b0c0f705a0f66a0498d4c04ecdb11bf271",
  "launcher-update.json": "919608033902c7030b01419b08782528b61d12c88542c3e99f13c067b61096ee",
});
const publicName = (name) => name.replaceAll(" ", ".");
export function inspectDraft(release, tag, expectedBody, expectedFiles) {
  assert.equal(release.id, recovery.releaseId);
  assert.equal(release.tag_name, "v1.4.2");
  assert.equal(release.target_commitish, recovery.sourceSha);
  assert.equal(release.prerelease, false);
  assert.equal(release.draft, true, "Only the fixed draft may receive missing assets");
  assert.notEqual(release.immutable, true);
  assert.equal(tag, null, "A draft must not have a production tag");
  assert.equal(release.name, "Aurora Launcher 1.4.2");
  const normalize = (s) => s.replaceAll("\r\n", "\n").trim();
  assert.equal(normalize(release.body), normalize(expectedBody));
  assert(Array.isArray(release.assets));
  const expected = new Map(expectedFiles.map((f) => [publicName(f.name), f]));
  const seen = new Set();
  for (const asset of release.assets) {
    assert(expected.has(asset.name), "Unexpected draft asset");
    assert(!seen.has(asset.name), "Duplicate draft asset"); seen.add(asset.name);
    const file = expected.get(asset.name);
    assert.equal(asset.size, file.bytes.length);
    assert.equal(asset.digest, "sha256:" + sha256(file.bytes));
  }
  return expectedFiles.filter((f) => !seen.has(publicName(f.name)));
}
export function inspectProvenance(artifact, run, publicKey) {
  assert.equal(artifact.id, recovery.artifactId);
  assert.equal(artifact.name, "aurora-launcher-1.4.2-nsis");
  assert.equal(artifact.expired, false);
  assert.equal(artifact.digest, recovery.artifactDigest);
  assert.equal(artifact.workflow_run.id, recovery.runId);
  assert.equal(artifact.workflow_run.head_sha, recovery.sourceSha);
  assert.equal(run.id, recovery.runId);
  assert.equal(run.head_sha, recovery.sourceSha);
  assert.equal(run.event, "workflow_dispatch");
  assert.equal(sha256(Buffer.from(publicKey)), recovery.publicKeySha256);
}

async function complete(directory) {
  assert.equal(process.env.GITHUB_ACTIONS, "true");
  assert.equal(process.env.GITHUB_REPOSITORY, repository);
  assert.equal(process.env.GITHUB_REF, "refs/heads/main");
  assert.equal(process.env.AURORA_COMPLETE_ACCEPTED_DRAFT, String(recovery.releaseId));
  const token = process.env.GH_TOKEN; assert(token && !/\s/.test(token));
  const key = process.env.AURORA_UPDATER_PUBKEY?.trim();
  const base = `https://api.github.com/repos/${repository}`;
  const headers = { Authorization: `Bearer ${token}`, Accept: "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28" };
  async function api(path, allow404 = false) {
    const response = await fetch(base + path, { headers, signal: AbortSignal.timeout(60_000) });
    if (allow404 && response.status === 404) return null;
    assert(response.ok, `Read-only API failed: ${response.status}`); return response.json();
  }
  inspectProvenance(await api(`/actions/artifacts/${recovery.artifactId}`), await api(`/actions/runs/${recovery.runId}`), key);
  const manifest = await verifyDirectory(directory, recovery.version, recovery.sourceSha, key);
  for (const [name, hash] of Object.entries(acceptedHashes)) assert.equal(sha256(await readFile(join(directory, name))), hash);
  const files = await Promise.all(Object.keys(acceptedHashes).filter((n) => n !== "launcher-update.json").map(async (name) => ({ name, bytes: await readFile(join(directory, name)) })));
  const notes = (await readFile(new URL("../../RELEASE_1_4_2_NOTES.md", import.meta.url), "utf8")).trim();
  const body = `${notes}\n\n## Installer artifacts\n\n| File | Architecture | Bytes | SHA-256 |\n| --- | --- | ---: | --- |\n${manifest.artifacts.map((a) => `| ${a.name} | ${a.architecture} | ${a.sizeBytes} | \`${a.sha256}\` |`).join("\n")}\n`;
  const draftPath = `/releases/${recovery.releaseId}`;
  const missing = inspectDraft(await api(draftPath), await api("/git/ref/tags/v1.4.2", true), body, files);
  // Existing bytes must verify before any upload. Never replace, rename or delete.
  const present = (await api(draftPath)).assets;
  for (const asset of present) {
    const file = files.find((f) => publicName(f.name) === asset.name); assert(file);
    const response = await fetch(base + `/releases/assets/${asset.id}`, { headers: { ...headers, Accept: "application/octet-stream" }, signal: AbortSignal.timeout(60_000) });
    assert(response.ok); assert.deepEqual(Buffer.from(await response.arrayBuffer()), file.bytes);
  }
  for (const file of missing) {
    // Recheck identity and inventory immediately before each bounded mutation.
    const stillMissing = inspectDraft(await api(draftPath), await api("/git/ref/tags/v1.4.2", true), body, files);
    if (!stillMissing.some((f) => f.name === file.name)) throw new Error("Draft moved concurrently; inspect before retry");
    const url = `https://uploads.github.com/repos/${repository}/releases/${recovery.releaseId}/assets?name=${encodeURIComponent(file.name)}`;
    const response = await fetch(url, { method: "POST", headers: { ...headers, "Content-Type": file.name.endsWith(".json") ? "application/json" : "application/octet-stream" }, body: file.bytes, signal: AbortSignal.timeout(60_000) });
    assert(response.ok, `Upload outcome requires inspection: ${response.status}`);
    console.log(`Added accepted missing asset: ${file.name}`);
  }
  assert.equal(inspectDraft(await api(draftPath), await api("/git/ref/tags/v1.4.2", true), body, files).length, 0);
  console.log(`Completed fixed draft ${recovery.releaseId}; publication remains the normal explicit-ID publisher's responsibility`);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3) throw new Error("Usage: complete-1-4-2-draft.mjs ACCEPTED_ARTIFACT_DIRECTORY");
  await complete(resolve(process.argv[2]));
}
