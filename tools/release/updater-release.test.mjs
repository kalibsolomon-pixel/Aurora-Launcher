import test from "node:test";
import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { mkdtemp, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { installerName, publicKeyPacket, verifyUpdaterSignature, createManifest, publicationDecision, sha256, verifyDirectory, repository } from "./updater-release.mjs";
import { publishManifest, authorityMarker } from "./publish-manifest.mjs";

// DIAGNOSTIC unit keys exist only in memory, never persisted or supplied to a production build.
const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const keyId = Buffer.from("0102030405060708", "hex");
const rawPublic = publicKey.export({ type: "spki", format: "der" }).subarray(-32);
const encodedPublic = Buffer.from(`untrusted comment: diagnostic unit key\n${Buffer.concat([Buffer.from("Ed"), keyId, rawPublic]).toString("base64")}\n`).toString("base64");
const version = "1.4.0", sourceSha = "a".repeat(40), name = installerName(version, "nsis");
const bytes = Buffer.from("DIAGNOSTIC installer bytes: not executable, not a release");
const notes = "# Aurora Launcher 1.4.0\n\nAurora Client 2.1.5 remains unchanged.";
function signature(comment = `timestamp:1\tfile:${name}\tversion:${version}`) {
  const signature = sign(null, createHash("blake2b512").update(bytes).digest(), privateKey);
  const packet = Buffer.concat([Buffer.from("ED"), keyId, signature]);
  const global = sign(null, Buffer.concat([signature, Buffer.from(comment)]), privateKey);
  return Buffer.from(`untrusted comment: diagnostic unit signature\n${packet.toString("base64")}\ntrusted comment: ${comment}\n${global.toString("base64")}\n`).toString("base64");
}
const encodedSignature = signature();
test("verifies exact prehashed installer and authenticated identity", () => verifyUpdaterSignature(bytes, encodedSignature, encodedPublic, version, name));
test("rejects modified installer bytes", () => assert.throws(() => verifyUpdaterSignature(Buffer.from("tampered"), encodedSignature, encodedPublic, version, name)));
test("rejects trusted-comment tampering", () => {
  const changed = Buffer.from(Buffer.from(encodedSignature, "base64").toString().replace("version:1.4.0", "version:9.9.9")).toString("base64");
  assert.throws(() => verifyUpdaterSignature(bytes, changed, encodedPublic, version, name));
});
test("rejects different public key and malformed signing inputs", () => {
  assert.throws(() => publicKeyPacket("C:/keys/public.pub"));
  assert.throws(() => verifyUpdaterSignature(bytes, "invalid", encodedPublic, version, name));
  const changed = Buffer.from(`untrusted comment: diagnostic\n${Buffer.concat([Buffer.from("Ed"), Buffer.alloc(8), rawPublic]).toString("base64")}\n`).toString("base64");
  assert.throws(() => verifyUpdaterSignature(bytes, encodedSignature, changed, version, name));
});
test("rejects mismatched signed filename/version", () => {
  assert.throws(() => verifyUpdaterSignature(bytes, signature(`file:foreign.exe\tversion:${version}`), encodedPublic, version, name));
  assert.throws(() => verifyUpdaterSignature(bytes, signature(`file:${name}\tversion:1.3.1`), encodedPublic, version, name));
});
test("supports current locked CLI filename-only authenticated comment", () => verifyUpdaterSignature(bytes, signature(`timestamp:1\tfile:${name}`), encodedPublic, version, name));
test("one static manifest derives exact NSIS public URL/signature", () => {
  const manifest = createManifest(version, notes, encodedSignature);
  assert.deepEqual(Object.keys(manifest.platforms), ["windows-x86_64"]);
  assert.equal(manifest.platforms["windows-x86_64"].url, `https://github.com/${repository}/releases/download/v1.4.0/Aurora.Launcher_1.4.0_x64-setup.exe`);
  assert.equal(manifest.platforms["windows-x86_64"].signature, encodedSignature);
});
test("rejects stale notes, development versions and unsafe Windows versions", () => {
  assert.throws(() => createManifest(version, "# Aurora Launcher 1.3.1\nAurora Client 2.1.5", encodedSignature));
  for (const value of ["1.4.0-diag.1", "01.4.0", "256.0.0", "1.0.65536"]) assert.throws(() => installerName(value, "nsis"));
});
test("identical manifest is a no-op; downgrade and same-version drift refuse", () => {
  const current = Buffer.from(JSON.stringify(createManifest(version, notes, encodedSignature)));
  assert.equal(publicationDecision(current, current), "unchanged");
  assert.throws(() => publicationDecision(current, Buffer.from(JSON.stringify({ ...JSON.parse(current), notes: notes + "changed" }))));
  const older = Buffer.from(JSON.stringify(createManifest("1.3.1", "# Aurora Launcher 1.3.1\nAurora Client 2.1.5", encodedSignature)));
  assert.throws(() => publicationDecision(current, older));
  assert.equal(publicationDecision(older, current), "replace");
});
function fixture(previous = null) {
  const sigBytes = Buffer.from(encodedSignature + "\n");
  const artifact = { name, format: "nsis", architecture: "x64", sizeBytes: bytes.length, sha256: sha256(bytes) };
  const sig = { name: name + ".sig", sizeBytes: sigBytes.length, sha256: sha256(sigBytes) };
  const metadata = { version, sourceSha, artifacts: [artifact], updater: { platform: "windows-x86_64", publicKeySha256: sha256(Buffer.from(encodedPublic)), signatures: [sig] } };
  const metadataBytes = Buffer.from(JSON.stringify(metadata));
  const manifestBytes = Buffer.from(JSON.stringify(createManifest(version, notes, encodedSignature)));
  const publicBytes = new Map([[name, bytes], [sig.name, sigBytes], ["release-assets.json", metadataBytes]]);
  const release = { id: 10, tag_name: "v1.4.0", target_commitish: sourceSha, draft: false, prerelease: false,
    assets: [...publicBytes].map(([file, content], index) => ({ id: index + 1, name: file.replaceAll(" ", "."), size: content.length, browser_download_url: `https://github.com/${repository}/releases/download/v1.4.0/${file.replaceAll(" ", ".")}` })) };
  let current = previous;
  const asset = () => current ? [{ id: 50, name: "launcher-update.json", size: current.length, browser_download_url: `https://github.com/${repository}/releases/download/launcher-updates/launcher-update.json` }] : [];
  const calls = [];
  const api = async (path, options = {}) => {
    calls.push([options.method ?? "GET", path]);
    if (path === "/releases/tags/v1.4.0" || path === "/releases/latest") return release;
    if (path === "/releases/tags/launcher-updates" || path === "/releases/20") return { id: 20, tag_name: "launcher-updates", draft: false, prerelease: false, body: authorityMarker, assets: asset(), upload_url: `https://uploads.github.com/repos/${repository}/releases/20/assets{?name,label}` };
    if (options.method === "DELETE") { current = null; return null; }
    if (options.method === "POST") { current = options.body; return {}; }
    throw new Error("Unexpected diagnostic API call");
  };
  const download = async (url) => {
    calls.push(["download", url]);
    if (url.includes("/launcher-updates/")) return current;
    return publicBytes.get(url.split("/").at(-1).replace("Aurora.Launcher", "Aurora Launcher"));
  };
  return { version, sourceSha, metadata, metadataBytes, manifestBytes, api, download, calls, release, publicBytes };
}
test("verifies every public immutable byte before first authority write", async () => {
  const value = fixture(); assert.equal(await publishManifest(value), "published");
  const writeIndex = value.calls.findIndex(([method]) => method === "POST");
  assert.equal(value.calls.slice(0, writeIndex).filter(([method, url]) => method === "download" && url.includes("/v1.4.0/")).length, 3);
  assert.equal(value.calls.filter(([method]) => method === "DELETE").length, 0);
});
test("bad or missing public artifact prevents any authority mutation", async () => {
  const value = fixture(); value.publicBytes.set(name, Buffer.from("corrupt"));
  await assert.rejects(publishManifest(value));
  assert.equal(value.calls.some(([method]) => ["POST", "DELETE"].includes(method)), false);
});
test("identical publication rerun never mutates manifest", async () => {
  const base = fixture(); const value = fixture(base.manifestBytes);
  assert.equal(await publishManifest(value), "unchanged");
  assert.equal(value.calls.some(([method]) => ["POST", "DELETE"].includes(method)), false);
});
test("replacement touches only the mutable manifest, after artifact verification", async () => {
  const previous = Buffer.from(JSON.stringify(createManifest("1.3.1", "# Aurora Launcher 1.3.1\nAurora Client 2.1.5", encodedSignature)));
  const value = fixture(previous); assert.equal(await publishManifest(value), "published");
  assert.deepEqual(value.calls.filter(([method]) => method === "DELETE"), [["DELETE", "/releases/assets/50"]]);
});
test("newer publication, changed authority and upload failure fail safely", async () => {
  for (const problem of ["newer", "changed", "upload"]) {
    const value = fixture(); const original = value.api;
    value.api = async (path, options) => {
      if (problem === "newer" && path === "/releases/latest") return { id: 99 };
      if (problem === "changed" && path === "/releases/20" && !options) return { draft: true, assets: [] };
      if (problem === "upload" && options?.method === "POST") throw new Error("DIAGNOSTIC failed upload");
      return original(path, options);
    };
    await assert.rejects(publishManifest(value));
    assert.equal(value.calls.some(([method]) => method === "DELETE"), false);
  }
});
test("verifies transferred signed directory and rejects traversal/tampering", async () => {
  const directory = await mkdtemp(join(tmpdir(), "aurora-release-diagnostic-unit-"));
  const value = fixture();
  for (const [file, content] of value.publicBytes) await writeFile(join(directory, file), content);
  await writeFile(join(directory, "launcher-update.json"), value.manifestBytes);
  await verifyDirectory(directory, version, sourceSha, encodedPublic);
  await writeFile(join(directory, name), "tampered");
  await assert.rejects(verifyDirectory(directory, version, sourceSha, encodedPublic));
  value.metadata.artifacts[0].name = "../escape.exe";
  await writeFile(join(directory, "release-assets.json"), JSON.stringify(value.metadata));
  await assert.rejects(verifyDirectory(directory, version, sourceSha, encodedPublic));
  // Explicit disposable fixture root retained; no recursive cleanup of user files.
  assert.ok((await readFile(join(directory, "launcher-update.json"))).equals(value.manifestBytes));
});
test("unowned or changed authority never permits a manifest write", async () => {
  for (const moment of ["initial", "recheck"]) {
    const value = fixture(); const original = value.api;
    value.api = async (path, options) => {
      const result = await original(path, options);
      if (path === (moment === "initial" ? "/releases/tags/launcher-updates" : "/releases/20")) return { ...result, body: "unrelated owner data" };
      return result;
    };
    await assert.rejects(publishManifest(value));
    assert.equal(value.calls.some(([method]) => ["POST", "DELETE"].includes(method)), false);
  }
});
test("failed mutable replacement preserves every immutable asset", async () => {
  const previous = Buffer.from(JSON.stringify(createManifest("1.3.1", "# Aurora Launcher 1.3.1\nAurora Client 2.1.5", encodedSignature)));
  const value = fixture(previous); const original = value.api;
  value.api = async (path, options) => {
    if (options?.method === "POST") throw new Error("DIAGNOSTIC replacement upload failure");
    return original(path, options);
  };
  await assert.rejects(publishManifest(value));
  assert.deepEqual(value.calls.filter(([method]) => method === "DELETE"), [["DELETE", "/releases/assets/50"]]);
  assert.equal(value.publicBytes.size, 3);
  assert.equal(await value.download(`https://github.com/${repository}/releases/download/launcher-updates/launcher-update.json`), null);
});
test("missing owner signing configuration fails without exposing a secret", () => {
  const result = spawnSync(process.execPath, ["tools/release/updater-release.mjs", "config", "unused-output.json"], {
    env: { ...process.env, AURORA_UPDATER_PUBKEY: "", TAURI_SIGNING_PRIVATE_KEY: "", TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "" }, encoding: "utf8",
  });
  assert.equal(result.status, 1); assert.match(result.stderr, /validation failed/);
});
