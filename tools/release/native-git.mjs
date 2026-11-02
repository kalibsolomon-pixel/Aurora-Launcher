import {createHash} from "node:crypto";
export const emptyTreeSha="4b825dc642cb6eb9a060e54bf8d69288fbee4904";
export const blobSha=bytes=>createHash("sha1").update(Buffer.from(`blob ${bytes.length}\0`)).update(bytes).digest("hex");
export function authorityToken(env) {
  const token = env.AURORA_AUTHORITY_TOKEN;
  if (typeof token !== "string" || !token.length || /\s/.test(token) ||
      token === env.GH_TOKEN || token === env.GITHUB_TOKEN) throw new Error("Dedicated authority credential is required");
  return token;
}
