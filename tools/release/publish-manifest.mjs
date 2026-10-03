import { readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { selectAsset, verifyBytes, expectedAssetName } from "./release-state.mjs";
import { repository, manifestName, sha256, publicationDecision, verifyDirectory } from "./updater-release.mjs";

export const authorityMarker = "Aurora Launcher production update authority.";
/** Called only after immutable release publication. Injected I/O permits offline
 * ordering/failure tests; this is release tooling, never a frontend boundary. */
export async function publishManifest({ version, sourceSha, metadata, metadataBytes, manifestBytes, api, download }) {
  const release = await api(`/releases/tags/v${version}`);
  if (release.draft || release.prerelease || release.target_commitish !== sourceSha || release.tag_name !== `v${version}`) throw new Error("Public release identity differs from accepted build");
  const expected = [...metadata.artifacts, ...metadata.updater.signatures,
    { name: "release-assets.json", sizeBytes: metadataBytes.length, sha256: sha256(metadataBytes) }];
  if (release.assets.length !== expected.length) throw new Error("Public release asset inventory differs");
  for (const entry of expected) {
    const asset = selectAsset(release.assets, entry);
    const url = `https://github.com/${repository}/releases/download/v${version}/${expectedAssetName(entry.name)}`;
    if (asset.browser_download_url !== url) throw new Error("Unexpected public installer location");
    verifyBytes(await download(url, entry.sizeBytes), entry);
  }
  if ((await api("/releases/latest")).id !== release.id) throw new Error("Refusing to expose an older public release");
  const authority = await api("/releases/tags/launcher-updates");
  if (authority.draft || authority.prerelease || authority.tag_name !== "launcher-updates" ||
      !authority.body?.includes(authorityMarker) || authority.assets.length > 1 ||
      authority.assets.some((asset) => asset.name !== manifestName)) throw new Error("Owner must initialize the dedicated update authority");
  const old = authority.assets[0];
  const url = `https://github.com/${repository}/releases/download/launcher-updates/${manifestName}`;
  let previous;
  if (old) {
    if (old.browser_download_url !== url) throw new Error("Unexpected authority asset URL");
    previous = await download(url, old.size);
    if (publicationDecision(previous, manifestBytes) === "unchanged") return "unchanged";
  }
  // Re-read immediately before changing the mutable authority. Never delete a
  // release, a tag, a signed installer or a versioned signature asset.
  const fresh = await api(`/releases/${authority.id}`);
  if (fresh.id !== authority.id || fresh.draft || fresh.prerelease || fresh.tag_name !== "launcher-updates" ||
      !fresh.body?.includes(authorityMarker) || fresh.upload_url !== authority.upload_url ||
      fresh.assets.length !== authority.assets.length || fresh.assets[0]?.id !== old?.id ||
      (await api("/releases/latest")).id !== release.id) throw new Error("Authority changed during review");
  if (old && !(await download(url, old.size)).equals(previous)) throw new Error("Authority content changed during review");
  const upload = authority.upload_url.replace(/\{.*$/, "");
  if (!new RegExp(`^https://uploads\\.github\\.com/repos/${repository}/releases/[0-9]+/assets$`).test(upload)) throw new Error("Unexpected authority upload URL");
  if (old) await api(`/releases/assets/${old.id}`, { method: "DELETE" });
  await api(`${upload}?name=${manifestName}`, { method: "POST", body: manifestBytes, contentType: "application/json" });
  // An interrupted replacement can leave discovery unavailable, never pointing
  // to missing/unsigned bytes. Identical reruns do not mutate the authority.
  const final = await api(`/releases/${authority.id}`);
  const entry = { name: manifestName, sizeBytes: manifestBytes.length, sha256: sha256(manifestBytes) };
  if (final.id !== authority.id || final.draft || final.prerelease || final.tag_name !== "launcher-updates" ||
      !final.body?.includes(authorityMarker) || final.assets.length !== 1) throw new Error("Unexpected final update inventory");
  selectAsset(final.assets, entry);
  verifyBytes(await download(url, manifestBytes.length), entry);
  return "published";
}

async function main() {
  const [version, sourceSha, directoryArg] = process.argv.slice(2);
  if (!directoryArg || process.env.GITHUB_REPOSITORY !== repository || !process.env.GH_TOKEN) throw new Error("Release environment is incomplete");
  const directory = resolve(directoryArg);
  const metadata = await verifyDirectory(directory, version, sourceSha, process.env.AURORA_UPDATER_PUBKEY?.trim());
  const metadataBytes = await readFile(join(directory, "release-assets.json"));
  const manifestBytes = await readFile(join(directory, manifestName));
  const base = `https://api.github.com/repos/${repository}`;
  const api = async (path, options = {}) => {
    const endpoint = path.startsWith("https://") ? path : base + path;
    const response = await fetch(endpoint, { method: options.method ?? "GET", body: options.body,
      headers: { Authorization: `Bearer ${process.env.GH_TOKEN}`, Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28", "Content-Type": options.contentType ?? "application/json" },
      signal: AbortSignal.timeout(30_000) });
    if (!response.ok) throw new Error(`Update authority API HTTP ${response.status}`);
    return response.status === 204 ? null : response.json();
  };
  const download = async (url, size) => {
    if (!Number.isSafeInteger(size) || size <= 0 || size > 512 * 1024 * 1024) throw new Error("Public artifact size is invalid");
    const response = await fetch(url, { signal: AbortSignal.timeout(300_000) }); // No authorization token.
    if (!response.ok) throw new Error(`Public verification HTTP ${response.status}`);
    const chunks = []; let count = 0;
    for await (const chunk of response.body) {
      count += chunk.length; if (count > size) throw new Error("Public artifact exceeds expected size"); chunks.push(chunk);
    }
    if (count !== size) throw new Error("Public artifact size differs");
    return Buffer.concat(chunks);
  };
  console.log(`Single update manifest ${await publishManifest({ version, sourceSha, metadata, metadataBytes, manifestBytes, api, download })}.`);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch(() => {
  console.error("Update publication failed; retain immutable artifacts and inspect the authority. No credentials are printed."); process.exitCode = 1;
});
