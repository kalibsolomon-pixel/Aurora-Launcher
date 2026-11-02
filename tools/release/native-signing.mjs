import { createHash, createPublicKey, verify } from "node:crypto";
export const repository = "kalibsolomon-pixel/Aurora-Launcher";
export const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
// Preserve a BOM so manifests reject it like serde_json; inventory callers may
// explicitly strip the existing PowerShell BOM after strict decoding.
export const strictUtf8 = (bytes) => new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
export function versionParts(version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version ?? "")) throw new Error("Invalid production version");
  const parts = version.split(".").map(Number);
  if (parts[0] > 255 || parts[1] > 255 || parts[2] > 65535) throw new Error("Version exceeds MSI limits");
  return parts;
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
/** Offline signature verification; runtime installation uses the native verified updater.
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
