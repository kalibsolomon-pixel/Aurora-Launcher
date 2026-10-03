import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { selectAsset, verifyBytes, expectedAssetName } from "./release-state.mjs";
import { repository, manifestName, sha256, publicationDecision, verifyDirectory, validateManifest, installerName, verifyUpdaterSignature } from "./updater-release.mjs";

export const authorityRef = "refs/heads/launcher-update-authority";
export const authorityUrl = `https://raw.githubusercontent.com/${repository}/launcher-update-authority/${manifestName}`;
export const initializationMessage = "Initialize Aurora Launcher update authority";
const refPath = "/git/ref/heads/launcher-update-authority";
const updatePath = "/git/refs/heads/launcher-update-authority";
const shaPattern = /^[0-9a-f]{40}$/;
export const blobSha = (bytes) => createHash("sha1").update(Buffer.from(`blob ${bytes.length}\0`)).update(bytes).digest("hex");
const requireSha = (sha) => { if (!shaPattern.test(sha ?? "")) throw new Error("Invalid Git object identity"); return sha; };

async function currentRef(api) {
  const ref = await api(refPath);
  if (ref?.ref !== authorityRef || ref.object?.type !== "commit") throw new Error("Owner must initialize the exact authority ref");
  return requireSha(ref.object.sha);
}
async function inspectCommit(api, sha) {
  const commit = await api(`/git/commits/${requireSha(sha)}`);
  if (commit.sha !== sha || !Array.isArray(commit.parents) || commit.parents.length > 1) throw new Error("Unexpected authority commit");
  const treeSha = requireSha(commit.tree?.sha);
  const tree = await api(`/git/trees/${treeSha}`);
  if (tree.sha !== treeSha || tree.truncated || !Array.isArray(tree.tree)) throw new Error("Incomplete authority tree");
  if (tree.tree.length === 0 && commit.parents.length === 0 && commit.message === initializationMessage) return { commit, bytes: null };
  const entry = tree.tree[0];
  if (commit.parents.length !== 1 || tree.tree.length !== 1 || entry.path !== manifestName || entry.type !== "blob" || entry.mode !== "100644") throw new Error("Unexpected authority files or history");
  const blob = await api(`/git/blobs/${requireSha(entry.sha)}`);
  if (blob.sha !== entry.sha || blob.encoding !== "base64" || !Number.isSafeInteger(blob.size) || blob.size > 16384) throw new Error("Invalid authority blob");
  const bytes = Buffer.from(blob.content, "base64");
  if (bytes.length !== blob.size || blobSha(bytes) !== entry.sha) throw new Error("Authority blob identity differs");
  validateManifest(bytes);
  return { commit, bytes };
}

async function verifyPublic({ api, download, version, sourceSha, metadata, metadataBytes, manifestBytes, publicKey }) {
  const candidate = validateManifest(manifestBytes);
  if (candidate.version !== version || metadata.version !== version || metadata.sourceSha !== sourceSha || !shaPattern.test(sourceSha) ||
      JSON.stringify(JSON.parse(metadataBytes)) !== JSON.stringify(metadata) || metadata.updater?.platform !== "windows-x86_64" ||
      metadata.updater?.publicKeySha256 !== sha256(Buffer.from(publicKey ?? ""))) throw new Error("Candidate provenance differs");
  const release = await api(`/releases/tags/v${version}`);
  if (release.draft || release.prerelease || release.immutable !== true || release.target_commitish !== sourceSha || release.tag_name !== `v${version}`) throw new Error("Public immutable release identity differs");
  if (!Array.isArray(metadata.artifacts) || !metadata.artifacts.some((a) => a.format === "nsis") ||
      new Set(metadata.artifacts.map((a) => a.format)).size !== metadata.artifacts.length || metadata.updater.signatures.length !== metadata.artifacts.length) throw new Error("Invalid artifact inventory");
  const expected = [...metadata.artifacts, ...metadata.updater.signatures,
    { name: "release-assets.json", sizeBytes: metadataBytes.length, sha256: sha256(metadataBytes) }];
  if (release.assets.length !== expected.length) throw new Error("Public asset inventory differs");
  const publicBytes = new Map();
  for (const entry of expected) {
    const asset = selectAsset(release.assets, entry);
    const url = `https://github.com/${repository}/releases/download/v${version}/${expectedAssetName(entry.name)}`;
    if (asset.browser_download_url !== url) throw new Error("Unexpected public artifact URL");
    const bytes = await download(url, entry.sizeBytes);
    verifyBytes(bytes, entry); publicBytes.set(entry.name, bytes);
  }
  for (const artifact of metadata.artifacts) {
    if (artifact.name !== installerName(version, artifact.format) || artifact.architecture !== "x64") throw new Error("Unexpected installer identity");
    const signature = publicBytes.get(artifact.name + ".sig")?.toString("utf8").trim();
    verifyUpdaterSignature(publicBytes.get(artifact.name), signature, publicKey, version, artifact.name);
    if (artifact.format === "nsis" && candidate.platforms["windows-x86_64"].signature !== signature) throw new Error("Candidate signature differs");
  }
  if ((await api("/releases/latest")).id !== release.id) throw new Error("Refusing to publish an older release");
  return release.id;
}

/** Object creation prepares unreachable objects. Non-forced ref advancement is
 * the LAST publication mutation. FF is not a literal CAS: branch protections
 * must prohibit rewind/deletion, and all writers must follow this protocol. */
export async function publishManifest(options) {
  const { api, download, manifestBytes, expectedParent, report = () => {}, publicAttempts = 7,
    wait = (ms) => new Promise((done) => setTimeout(done, ms)) } = options;
  if ((options.repository ?? repository) !== repository || (options.ref ?? authorityRef) !== authorityRef) throw new Error("Unexpected authority repository/ref");
  if (!Number.isInteger(publicAttempts) || publicAttempts < 1 || publicAttempts > 7) throw new Error("Invalid verification bound");
  const releaseId = await verifyPublic(options);
  const observed = await currentRef(api);
  if (expectedParent !== undefined && expectedParent !== observed) throw new Error("Stale expected parent");
  const previous = await inspectCommit(api, observed);
  const unchanged = previous.bytes && publicationDecision(previous.bytes, manifestBytes) === "unchanged";
  let published = observed;
  if (!unchanged) {
    const blob = await api("/git/blobs", { method: "POST", body: { content: manifestBytes.toString("base64"), encoding: "base64" } });
    if (blob.sha !== blobSha(manifestBytes)) throw new Error("Prepared blob differs");
    const tree = await api("/git/trees", { method: "POST", body: { tree: [{ path: manifestName, mode: "100644", type: "blob", sha: blob.sha }] } });
    const commit = await api("/git/commits", { method: "POST", body: { message: `Publish Aurora Launcher ${options.version} update authority`, tree: requireSha(tree.sha), parents: [observed] } });
    published = requireSha(commit.sha);
    const prepared = await inspectCommit(api, published);
    if (prepared.commit.tree.sha !== tree.sha || prepared.commit.parents[0]?.sha !== observed || !prepared.bytes?.equals(manifestBytes)) throw new Error("Prepared commit differs");
    report({ state: "PREPARED", sha: published, parent: observed });
    if ((await currentRef(api)) !== observed || (await api("/releases/latest")).id !== releaseId) throw new Error("Authority/latest moved during preparation");
    try {
      await api(updatePath, { method: "PATCH", body: { sha: published, force: false } });
    } catch {
      // A timeout may follow acceptance. Recover through GET only; never resend.
      const actual = await currentRef(api);
      if (actual !== published) throw new Error(actual === observed ? "Ref update failed; authority unchanged" : "Concurrent authority movement; inspect before retry");
    }
  }
  if ((await currentRef(api)) !== published) throw new Error("Authority moved after publication; inspect before retry");
  report({ state: "PUBLISHED", sha: published, unchanged: Boolean(unchanged) });
  for (let attempt = 0; attempt < publicAttempts; attempt++) {
    if ((await currentRef(api)) !== published) throw new Error("Authority moved during public verification");
    let bytes;
    try { bytes = await download(authorityUrl, manifestBytes.length); }
    catch { /* Bounded read-only retry for unavailable/stale raw content. */ }
    if (bytes?.equals(manifestBytes)) {
      if ((await currentRef(api)) !== published) throw new Error("Authority moved during public verification");
      report({ state: "VERIFIED", sha: published, sha256: sha256(bytes) });
      return unchanged ? "unchanged" : "published";
    }
    if (attempt + 1 < publicAttempts) await wait(60_000);
  }
  throw new Error("PUBLISHED but public bytes not VERIFIED; retain ref and inspect CDN before retry");
}

async function main() {
  const [version, sourceSha, directoryArg] = process.argv.slice(2);
  if (!directoryArg || process.env.GITHUB_REPOSITORY !== repository || !process.env.GH_TOKEN || process.env.GITHUB_ACTIONS !== "true" || process.env.GITHUB_EVENT_NAME !== "workflow_dispatch") throw new Error("Protected manual release environment is required");
  const directory = resolve(directoryArg);
  const publicKey = process.env.AURORA_UPDATER_PUBKEY?.trim();
  const metadata = await verifyDirectory(directory, version, sourceSha, publicKey);
  const api = async (path, options = {}) => {
    if (!path.startsWith("/") || path.includes("..")) throw new Error("Unexpected API path");
    const response = await fetch(`https://api.github.com/repos/${repository}${path}`, {
      method: options.method ?? "GET", body: options.body ? JSON.stringify(options.body) : undefined,
      headers: { Authorization: `Bearer ${process.env.GH_TOKEN}`, Accept: "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28", "Content-Type": "application/json" },
      redirect: "error", signal: AbortSignal.timeout(30_000),
    });
    if (!response.ok) throw new Error(`Authority API HTTP ${response.status}`);
    return response.json();
  };
  const download = async (url, size) => {
    if (!Number.isSafeInteger(size) || size <= 0 || size > 512 * 1024 * 1024) throw new Error("Invalid public size");
    const raw = url === authorityUrl;
    const response = await fetch(url, { redirect: raw ? "error" : "follow", headers: raw ? { "Cache-Control": "no-cache" } : {}, signal: AbortSignal.timeout(300_000) });
    if (!response.ok) throw new Error(`Public verification HTTP ${response.status}`);
    const chunks = []; let count = 0;
    for await (const chunk of response.body) {
      count += chunk.length; if (count > size) throw new Error("Public bytes exceed expected bound"); chunks.push(chunk);
    }
    return Buffer.concat(chunks);
  };
  console.log(`Single authority ${await publishManifest({ version, sourceSha, metadata, publicKey,
    metadataBytes: await readFile(join(directory, "release-assets.json")), manifestBytes: await readFile(join(directory, manifestName)), api, download,
    report: (state) => console.log(JSON.stringify(state)) })}.`);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch((error) => {
  console.error("Authority publication incomplete; retain immutable artifacts/ref and inspect PREPARED/PUBLISHED/VERIFIED status.");
  const apiStatus = /^Authority API HTTP ([0-9]{3})$/.exec(error.message);
  if (apiStatus) console.error(`Authority API HTTP ${apiStatus[1]} (response body withheld).`);
  console.error(error.message.startsWith("PUBLISHED but") ? error.message : "Validation/API failure; do not blindly retry mutation.");
  process.exitCode = 1;
});
