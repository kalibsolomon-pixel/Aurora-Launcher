import { selectRelease } from "./release-state.mjs";

const [version, sourceSha, resumeText] = process.argv.slice(2);
const repository = process.env.GITHUB_REPOSITORY;
const token = process.env.GH_TOKEN;
if (!/^\d+\.\d+\.\d+$/.test(version ?? "") || !/^[0-9a-f]{40}$/.test(sourceSha ?? "") || !/^[\w.-]+\/[\w.-]+$/.test(repository ?? "") || !token) {
  throw new Error("Release state check requires version, source SHA, repository and token");
}
const resumeId = resumeText ? Number(resumeText) : null;
if (resumeText && (!Number.isSafeInteger(resumeId) || resumeId <= 0)) throw new Error("Invalid resume ID");
const base = `https://api.github.com/repos/${repository}`;
const headers = { Authorization: `Bearer ${token}`, Accept: "application/vnd.github+json", "User-Agent": "aurora-launcher-release-check" };
async function get(path, allow404 = false) {
  const response = await fetch(`${base}${path}`, { headers });
  if (allow404 && response.status === 404) return null;
  if (!response.ok) throw new Error(`GitHub state lookup failed: HTTP ${response.status}`);
  return response.json();
}
const releases = [];
for (let page = 1; page <= 20; page++) {
  const batch = await get(`/releases?per_page=100&page=${page}`);
  releases.push(...batch);
  if (batch.length < 100) break;
  if (page === 20) throw new Error("Release listing exceeded safety bound");
}
const tag = `v${version}`;
const ref = await get(`/git/ref/tags/${tag}`, true);
if (ref && ref.object.type !== "commit") throw new Error("Annotated tag requires independent resolution");
const selected = selectRelease(releases, tag, sourceSha, resumeId, ref?.object.sha ?? null);
console.log(`${tag}: ${selected.kind}${selected.release ? ` release ID ${selected.release.id}` : ""}`);
