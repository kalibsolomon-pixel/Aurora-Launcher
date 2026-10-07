import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

// Deterministic build-source identity; never fetch publication authority here.
export function productionClientIdentities(catalog) {
  if (catalog?.schemaVersion !== 1 || !Array.isArray(catalog.releases)) throw new Error("Invalid production Client catalog");
  const versions = catalog.releases.filter((release) => release.channel === "stable").map((release) => release.auroraVersion);
  if (!versions.length || versions.some((version) => typeof version !== "string" || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version) ||
      !version.split('.').every((part) => Number.isSafeInteger(Number(part)))) || new Set(versions).size !== versions.length) {
    throw new Error("Production Client identities must be unique stable semantic versions");
  }
  return versions.sort((left, right) => {
    const a = left.split('.').map(Number), b = right.split('.').map(Number);
    return b[0] - a[0] || b[1] - a[1] || b[2] - a[2];
  });
}
export const productionClientVersions = Object.freeze(productionClientIdentities(
  JSON.parse(readFileSync(new URL('../../src-tauri/production/aurora-releases.json', import.meta.url), 'utf8')),
));
export const productionClientVersion = productionClientVersions[0];

export function selectRelease(releases, tag, sourceSha, resumeId = null, tagSha = null) {
  if (tagSha && tagSha !== sourceSha) throw new Error(`${tag}: conflicting tag target`);
  const matches = releases.filter((release) => release.tag_name === tag);
  if (matches.length > 1) throw new Error(`${tag}: multiple releases`);
  if (matches.length === 0) {
    if (resumeId !== null || tagSha) throw new Error(`${tag}: expected draft is missing`);
    return { kind: "create" };
  }
  const release = matches[0];
  if (release.target_commitish !== sourceSha || release.prerelease) {
    throw new Error(`${tag}: release target or prerelease status conflicts`);
  }
  if (resumeId !== null && release.id !== resumeId) throw new Error(`${tag}: release ID conflicts`);
  if (release.draft) {
    if (tagSha) throw new Error(`${tag}: draft has an unexpected tag ref`);
    return { kind: "resume", release };
  }
  if (resumeId !== null && release.id !== resumeId) throw new Error(`${tag}: published release ID conflicts`);
  if (!tagSha) throw new Error(`${tag}: public release has no tag ref`);
  return { kind: "public", release };
}

export function expectedAssetName(name) {
  return name.replaceAll(" ", ".");
}

export function verifyNotesIdentity(sourceNotes, version, clientVersion = productionClientVersion) {
  const heading = sourceNotes.split(/\r?\n/, 1)[0];
  const clientIdentities = [...sourceNotes.matchAll(/\bAurora Client ([0-9]+\.[0-9]+\.[0-9]+)(?![\w+-]|\.[\w])/g)].map((match) => match[1]);
  if (!(heading === `# Aurora Launcher ${version}` || heading.startsWith(`# Aurora Launcher ${version} `)) || !clientIdentities.includes(clientVersion)) {
    throw new Error(`Release notes identity mismatch: expected Aurora Launcher ${version} naming the current production Aurora Client ${clientVersion}`);
  }
}

export function selectAsset(assets, expected) {
  const expectedName = expectedAssetName(expected.name);
  const candidates = assets.filter((asset) => asset.name === expected.name || asset.name === expectedName);
  if (candidates.length !== 1) throw new Error(`Expected exactly one asset candidate for ${expected.name}`);
  const candidate = candidates[0];
  if (candidate.size !== expected.sizeBytes) throw new Error(`Asset size mismatch: ${expected.name}`);
  if (candidate.digest && candidate.digest.toLowerCase() !== `sha256:${expected.sha256.toLowerCase()}`) {
    throw new Error(`Asset digest mismatch: ${expected.name}`);
  }
  return candidate;
}

export function verifyBytes(bytes, expected) {
  if (bytes.length !== expected.sizeBytes || createHash("sha256").update(bytes).digest("hex") !== expected.sha256.toLowerCase()) {
    throw new Error(`Downloaded asset bytes mismatch: ${expected.name}`);
  }
}
