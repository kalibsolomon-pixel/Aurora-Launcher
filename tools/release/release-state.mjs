import { createHash } from "node:crypto";

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
