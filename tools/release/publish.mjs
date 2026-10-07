import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { selectRelease, selectAsset, verifyBytes, verifyNotesIdentity } from "./release-state.mjs";
import { verifyDirectory, repository as productionRepository } from "./updater-release.mjs";

const [version, sourceSha, assetsDirectory, resumeIdText, mode] = process.argv.slice(2);
if (!/^\d+\.\d+\.\d+$/.test(version ?? "") || !/^[0-9a-f]{40}$/.test(sourceSha ?? "") || !assetsDirectory) {
  throw new Error("Usage: publish.mjs VERSION SOURCE_SHA ASSETS_DIRECTORY [RESUME_RELEASE_ID]");
}
const resumeId = resumeIdText ? Number(resumeIdText) : null;
if (resumeIdText && (!Number.isSafeInteger(resumeId) || resumeId <= 0)) throw new Error("Invalid resume release ID");
if (mode && mode !== "verify-only") throw new Error("Unknown publication mode");
const repository = process.env.GITHUB_REPOSITORY;
const token = process.env.GH_TOKEN;
if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? "") || !token) throw new Error("GitHub release environment is incomplete");
const base = `https://api.github.com/repos/${repository}`;
const tag = `v${version}`;
const headers = { Authorization: `Bearer ${token}`, Accept: "application/vnd.github+json", "User-Agent": "aurora-launcher-release", "X-GitHub-Api-Version": "2022-11-28" };
async function api(url, options = {}) {
  const response = await fetch(url.startsWith("https://") ? url : `${base}${url}`, { ...options, headers: { ...headers, ...options.headers } });
  if (!response.ok) throw new Error(`GitHub API HTTP ${response.status}: ${options.method ?? "GET"} ${new URL(response.url).pathname}`);
  return response;
}
async function listReleases() {
  const all = [];
  for (let page = 1; page <= 20; page++) {
    const batch = await (await api(`/releases?per_page=100&page=${page}`)).json();
    all.push(...batch);
    if (batch.length < 100) return all;
  }
  throw new Error("Release listing exceeded safety bound");
}
async function tagSha() {
  const response = await fetch(`${base}/git/ref/tags/${tag}`, { headers });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`Tag lookup failed: HTTP ${response.status}`);
  const ref = await response.json();
  if (ref.object.type !== "commit") throw new Error("Annotated tag requires independent resolution");
  return ref.object.sha;
}
async function state() { return selectRelease(await listReleases(), tag, sourceSha, resumeId, await tagSha()); }
const directory = resolve(assetsDirectory);
if (repository !== productionRepository) throw new Error("Unexpected production repository");
await verifyDirectory(directory, version, sourceSha, process.env.AURORA_UPDATER_PUBKEY?.trim());
const manifestPath = join(directory, "release-assets.json");
const manifestBytes = await readFile(manifestPath);
const manifest = JSON.parse(manifestBytes.toString("utf8"));
if (manifest.version !== version || manifest.sourceSha !== sourceSha || !Array.isArray(manifest.artifacts) || manifest.artifacts.length === 0) {
  throw new Error("Validated artifact manifest does not match source");
}
const expected = [...manifest.artifacts, ...manifest.updater.signatures, { name: "release-assets.json", sizeBytes: manifestBytes.length, sha256: (await import("node:crypto")).createHash("sha256").update(manifestBytes).digest("hex") }];
for (const entry of expected) verifyBytes(await readFile(join(directory, entry.name)), entry);
const notesFile = join(import.meta.dirname, `../../RELEASE_${version.replaceAll(".", "_")}_NOTES.md`);
const sourceNotes = (await readFile(notesFile, "utf8")).trim();
verifyNotesIdentity(sourceNotes, version);
const notes = `${sourceNotes}\n\n## Installer artifacts\n\n| File | Architecture | Bytes | SHA-256 |\n| --- | --- | ---: | --- |\n${manifest.artifacts.map((a) => `| ${a.name} | ${a.architecture} | ${a.sizeBytes} | \`${a.sha256}\` |`).join("\n")}\n`;
const normalized = (value) => value.replaceAll("\r\n", "\n").trim();
async function verifyRelease(release, publicDownload = false) {
  if (release.id !== (resumeId ?? release.id) || release.target_commitish !== sourceSha || release.tag_name !== tag || release.prerelease) throw new Error("Release identity changed");
  if (normalized(release.body ?? "") !== normalized(notes)) throw new Error("Release notes differ from verified source");
  if (release.assets.length !== expected.length) throw new Error("Release asset count differs from build");
  for (const entry of expected) {
    const asset = selectAsset(release.assets, entry);
    // Public verification is deliberately unauthenticated, including redirects.
    const publicUrl = `https://github.com/${repository}/releases/download/${tag}/${entry.name.replaceAll(" ", ".")}`;
    if (publicDownload && asset.browser_download_url !== publicUrl) throw new Error("Public asset URL differs from intended release");
    const response = publicDownload
      ? await fetch(publicUrl, { headers: { Accept: "application/octet-stream" }, signal: AbortSignal.timeout(300_000) })
      : await api(`/releases/assets/${asset.id}`, { headers: { Accept: "application/octet-stream" } });
    if (!response.ok) throw new Error(`Public asset HTTP ${response.status}`);
    const bytes = Buffer.from(await response.arrayBuffer());
    verifyBytes(bytes, entry);
    if (publicDownload) console.log(`Public asset verified: ${asset.name} (${bytes.length} bytes, ${entry.sha256})`);
  }
}

let selected = await state();
if (mode === "verify-only") {
  if (selected.kind !== "resume") throw new Error("Verify-only requires the expected draft");
  await verifyRelease(await (await api(`/releases/${selected.release.id}`)).json());
  console.log(`Verified draft release ID ${selected.release.id} without mutation`);
  process.exit(0);
}
if (selected.kind === "create") {
  const created = await (await api("/releases", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ tag_name: tag, target_commitish: sourceSha, name: `Aurora Launcher ${version}`, body: notes, draft: true, prerelease: false }) })).json();
  selected = await state();
  if (selected.kind !== "resume" || selected.release.id !== created.id) throw new Error("Created draft cannot be uniquely rediscovered");
}
if (selected.kind === "resume") {
  let release = await (await api(`/releases/${selected.release.id}`)).json();
  if (release.draft !== true) throw new Error("Draft state changed");
  for (const entry of expected) {
    if (release.assets.length && release.assets.some((a) => a.name === entry.name || a.name === entry.name.replaceAll(" ", "."))) continue;
    if (resumeId !== null) throw new Error(`Existing draft is missing asset ${entry.name}; refusing to upload during recovery`);
    const bytes = await readFile(join(directory, entry.name));
    const uploadUrl = release.upload_url.replace(/\{.*$/, "") + `?name=${encodeURIComponent(entry.name)}`;
    await api(uploadUrl, { method: "POST", headers: { "Content-Type": entry.name.endsWith(".json") ? "application/json" : "application/octet-stream" }, body: bytes });
    release = await (await api(`/releases/${release.id}`)).json();
  }
  await verifyRelease(release);
  if ((await state()).kind !== "resume") throw new Error("Release state changed immediately before publication");
  release = await (await api(`/releases/${release.id}`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ draft: false }) })).json();
  if (release.draft) throw new Error("GitHub did not publish the draft");
  console.log(`Published existing release ID ${release.id}`);
}
const final = await state();
if (final.kind !== "public") throw new Error("Public release/tag state did not resolve");
const publicByTag = await (await api(`/releases/tags/${tag}`)).json();
if (publicByTag.id !== final.release.id) throw new Error("Public tag resolves to different release");
await verifyRelease(publicByTag, true);
console.log(`Verified public release ${publicByTag.html_url} at ${sourceSha}`);
