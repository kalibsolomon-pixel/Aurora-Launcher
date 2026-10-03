import { createHash, createPublicKey, verify } from "node:crypto";
import { readFile, writeFile, copyFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { expectedAssetName, verifyBytes, verifyNotesIdentity } from "./release-state.mjs";

export const repository = "kalibsolomon-pixel/Aurora-Launcher";
export const manifestName = "launcher-update.json";
export const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
export function versionParts(version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version ?? "")) throw new Error("Invalid production version");
  const parts = version.split(".").map(Number);
  if (parts[0] > 255 || parts[1] > 255 || parts[2] > 65535) throw new Error("Version exceeds MSI limits");
  return parts;
}
export function installerName(version, format) {
  versionParts(version);
  if (format === "nsis") return `Aurora Launcher_${version}_x64-setup.exe`;
  if (format === "msi") return `Aurora Launcher_${version}_x64_en-US.msi`;
  throw new Error("Unexpected installer format");
}
function decodeBase64(text) {
  if (typeof text !== "string" || !text.length || text.length > 8192 || !/^[A-Za-z0-9+/]+={0,2}$/.test(text)) throw new Error("Invalid signing encoding");
  const bytes = Buffer.from(text, "base64");
  if (bytes.toString("base64") !== text) throw new Error("Noncanonical signing encoding");
  return bytes;
}
export function publicKeyPacket(encoded) {
  const lines = decodeBase64(encoded).toString("utf8").trim().split(/\r?\n/);
  if (lines.length !== 2 || !lines[0].startsWith("untrusted comment: ")) throw new Error("Expected Tauri public key content, not a path");
  const packet = decodeBase64(lines[1]);
  if (packet.length !== 42 || !["Ed", "ED"].includes(packet.subarray(0, 2).toString())) throw new Error("Invalid public key packet");
  return packet;
}
/** Offline build-output check only. Runtime installation remains official Tauri.
 * Minisign's prehashed Ed25519 and authenticated-comment layout is defined by
 * minisign-verify (also used by Tauri); no private signing material is accepted. */
export function verifyUpdaterSignature(bytes, encodedSignature, encodedPublicKey, version, name) {
  const key = publicKeyPacket(encodedPublicKey);
  const lines = decodeBase64(encodedSignature).toString("utf8").trim().split(/\r?\n/);
  if (lines.length !== 4 || !lines[0].startsWith("untrusted comment: ") || !lines[2].startsWith("trusted comment: ")) throw new Error("Invalid updater signature format");
  const packet = decodeBase64(lines[1]);
  const global = decodeBase64(lines[3]);
  if (packet.length !== 74 || global.length !== 64 || packet.subarray(0, 2).toString() !== "ED" || !packet.subarray(2, 10).equals(key.subarray(2, 10))) throw new Error("Updater signature algorithm/key mismatch");
  const publicKey = createPublicKey({ key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), key.subarray(10)]), format: "der", type: "spki" });
  const signature = packet.subarray(10);
  const comment = lines[2].slice(17);
  if (!verify(null, createHash("blake2b512").update(bytes).digest(), publicKey, signature) ||
      !verify(null, Buffer.concat([signature, Buffer.from(comment)]), publicKey, global)) throw new Error("Updater signature verification failed");
  const fields = comment.split("\t");
  const files = fields.filter((field) => field.startsWith("file:"));
  const versions = fields.filter((field) => field.startsWith("version:"));
  if (files.length !== 1 || files[0].slice(5) !== name || versions.length > 1 ||
      (versions.length === 1 && versions[0].slice(8) !== version)) throw new Error("Signed installer identity differs from release");
}
export function createManifest(version, notes, signature) {
  versionParts(version);
  verifyNotesIdentity(notes, version, "2.1.5");
  if (notes.length > 8192 || /[\x00-\x08\x0b\x0c\x0e-\x1f]/.test(notes)) throw new Error("Invalid release notes");
  decodeBase64(signature);
  return { version, notes: notes.trim(), platforms: { "windows-x86_64": {
    url: `https://github.com/${repository}/releases/download/v${version}/${expectedAssetName(installerName(version, "nsis"))}`,
    signature,
  } } };
}
export function publicationDecision(previousBytes, nextBytes) {
  if (previousBytes.equals(nextBytes)) return "unchanged";
  const previous = JSON.parse(previousBytes.toString("utf8"));
  const next = JSON.parse(nextBytes.toString("utf8"));
  const left = versionParts(previous.version), right = versionParts(next.version);
  const difference = right.map((part, index) => part - left[index]).find((part) => part !== 0) ?? 0;
  if (difference <= 0) throw new Error("Refusing manifest downgrade or same-version drift");
  const target = previous.platforms?.["windows-x86_64"];
  if (target?.url !== createManifest(previous.version, `# Aurora Launcher ${previous.version}\nAurora Client 2.1.5`, target?.signature).platforms["windows-x86_64"].url) throw new Error("Unexpected previous update authority");
  return "replace";
}
export async function verifyDirectory(directory, version, sourceSha, publicKey) {
  versionParts(version); publicKeyPacket(publicKey);
  const manifest = JSON.parse((await readFile(join(directory, "release-assets.json"), "utf8")).replace(/^\uFEFF/, ""));
  if (manifest.version !== version || manifest.sourceSha !== sourceSha || !/^[0-9a-f]{40}$/.test(sourceSha) ||
      manifest.updater?.publicKeySha256 !== sha256(Buffer.from(publicKey)) || manifest.updater?.platform !== "windows-x86_64") throw new Error("Release updater provenance mismatch");
  if (!Array.isArray(manifest.artifacts) || !manifest.artifacts.some((entry) => entry.format === "nsis") ||
      new Set(manifest.artifacts.map((entry) => entry.format)).size !== manifest.artifacts.length) throw new Error("Release must contain one NSIS installer");
  const signatures = manifest.updater.signatures;
  if (!Array.isArray(signatures) || signatures.length !== manifest.artifacts.length) throw new Error("Signature inventory mismatch");
  for (const entry of manifest.artifacts) {
    if (entry.name !== installerName(version, entry.format) || entry.architecture !== "x64") throw new Error("Installer identity mismatch");
    const bytes = await readFile(join(directory, entry.name)); verifyBytes(bytes, entry);
    const sig = signatures.filter((item) => item.name === entry.name + ".sig");
    if (sig.length !== 1) throw new Error("Missing or duplicate signature");
    const signatureBytes = await readFile(join(directory, sig[0].name)); verifyBytes(signatureBytes, sig[0]);
    verifyUpdaterSignature(bytes, signatureBytes.toString("utf8").trim(), publicKey, version, entry.name);
  }
  const updaterBytes = await readFile(join(directory, manifestName));
  const updater = JSON.parse(updaterBytes.toString("utf8"));
  const nsisSignature = (await readFile(join(directory, installerName(version, "nsis") + ".sig"), "utf8")).trim();
  if (JSON.stringify(updater) !== JSON.stringify(createManifest(version, updater.notes, nsisSignature))) throw new Error("Update manifest differs from verified installers");
  return manifest;
}
async function main() {
  const [mode, version, sourceSha, directoryArg, rootArg] = process.argv.slice(2);
  if (!["pack", "verify", "config"].includes(mode)) throw new Error("Usage: updater-release.mjs pack|verify VERSION SOURCE_SHA DIRECTORY ROOT; or config OUTPUT");
  const publicKey = process.env.AURORA_UPDATER_PUBKEY?.trim(); publicKeyPacket(publicKey);
  if (mode === "config") {
    if (!process.env.TAURI_SIGNING_PRIVATE_KEY || !Object.hasOwn(process.env, "TAURI_SIGNING_PRIVATE_KEY_PASSWORD")) throw new Error("Required signing environment is missing");
    await writeFile(version, JSON.stringify({ bundle: { createUpdaterArtifacts: true }, plugins: { updater: { pubkey: publicKey } } }), { flag: "wx" });
    console.log("Release updater configuration prepared (public key only)."); return;
  }
  const directory = resolve(directoryArg), root = resolve(rootArg);
  if (mode === "pack") {
    const path = join(directory, "release-assets.json");
    const manifest = JSON.parse((await readFile(path, "utf8")).replace(/^\uFEFF/, ""));
    const signatures = [];
    for (const entry of manifest.artifacts) {
      if (entry.name !== installerName(version, entry.format)) throw new Error("Unexpected installer name");
      const name = entry.name + ".sig";
      const source = join(root, "src-tauri/target/release/bundle", entry.format, name);
      const bytes = await readFile(source);
      verifyUpdaterSignature(await readFile(join(directory, entry.name)), bytes.toString("utf8").trim(), publicKey, version, entry.name);
      await copyFile(source, join(directory, name));
      signatures.push({ name, sizeBytes: bytes.length, sha256: sha256(bytes) });
    }
    manifest.updater = { platform: "windows-x86_64", publicKeySha256: sha256(Buffer.from(publicKey)), signatures };
    const notes = await readFile(join(root, `RELEASE_${version.replaceAll(".", "_")}_NOTES.md`), "utf8");
    const signature = (await readFile(join(directory, installerName(version, "nsis") + ".sig"), "utf8")).trim();
    await writeFile(join(directory, manifestName), JSON.stringify(createManifest(version, notes, signature), null, 2) + "\n", { flag: "wx" });
    await writeFile(path, JSON.stringify(manifest, null, 2) + "\n");
  }
  await verifyDirectory(directory, version, sourceSha, publicKey);
  console.log("Signed updater artifacts and single manifest verified.");
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch(() => {
  console.error("Release updater validation failed; inspect configuration, signatures and artifact identity. No signing secret is printed."); process.exitCode = 1;
});
