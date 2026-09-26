import type { HeadAvatar } from "../backend";

/** Front-facing Minecraft UVs. Rendering is cosmetic; no network or identity decisions. */
export function playerPixels(avatar: HeadAvatar | null | undefined): Uint8ClampedArray | null {
  if (!avatar || ![32, 64].includes(avatar.skinHeight) ||
      !["classic", "slim"].includes(avatar.model) ||
      avatar.skinRgba?.length !== 64 * avatar.skinHeight * 4 ||
      !avatar.skinRgba.every(n => Number.isInteger(n) && n >= 0 && n <= 255)) return null;
  const output = new Uint8ClampedArray(20 * 32 * 4);
  const modern = avatar.skinHeight === 64;
  const arm = modern && avatar.model === "slim" ? 3 : 4;
  function face(sx: number, sy: number, w: number, h: number, dx: number, dy: number, mirror = false) {
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const source = ((sy + y) * 64 + sx + (mirror ? w - 1 - x : x)) * 4;
      const dest = ((dy + y) * 20 + dx + x) * 4;
      const a = avatar!.skinRgba[source + 3] / 255;
      const b = output[dest + 3] / 255;
      const alpha = a + b * (1 - a);
      for (let c = 0; c < 3; c++) output[dest + c] = alpha === 0 ? 0 : Math.round((avatar!.skinRgba[source + c] * a + output[dest + c] * b * (1 - a)) / alpha);
      output[dest + 3] = Math.round(alpha * 255);
    }
  }
  face(8, 8, 8, 8, 6, 0);
  face(20, 20, 8, 12, 6, 8);
  face(44, 20, arm, 12, 6 - arm, 8);
  face(modern ? 36 : 44, modern ? 52 : 20, arm, 12, 14, 8, !modern);
  face(4, 20, 4, 12, 6, 20);
  face(modern ? 20 : 4, modern ? 52 : 20, 4, 12, 10, 20, !modern);
  face(40, 8, 8, 8, 6, 0);
  if (modern) {
    face(20, 36, 8, 12, 6, 8);
    face(44, 36, arm, 12, 6 - arm, 8);
    face(52, 52, arm, 12, 14, 8);
    face(4, 36, 4, 12, 6, 20);
    face(4, 52, 4, 12, 10, 20);
  }
  return output;
}
