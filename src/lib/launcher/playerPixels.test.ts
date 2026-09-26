import assert from "node:assert/strict";
import { it } from "node:test";
import { playerPixels } from "./playerPixels.ts";
import type { HeadAvatar } from "../backend";

function fixture(height: 32 | 64 = 64, model: "classic" | "slim" = "classic"): HeadAvatar {
  return { rgba: Array(256).fill(255), model, skinHeight: height, skinRgba: Array(64 * height * 4).fill(0) };
}
function paint(a: HeadAvatar, sx: number, sy: number, w: number, h: number, color: number[]) {
  for (let y = sy; y < sy + h; y++) for (let x = sx; x < sx + w; x++) a.skinRgba.splice((y * 64 + x) * 4, 4, ...color);
}
function pixel(p: Uint8ClampedArray, x: number, y: number) { return [...p.slice((y * 20 + x) * 4, (y * 20 + x) * 4 + 4)]; }
it("modern body uses distinct head, torso, both arms and both legs", () => {
  const a = fixture();
  [[8,8,8,8],[20,20,8,12],[44,20,4,12],[36,52,4,12],[4,20,4,12],[20,52,4,12]].forEach(([x,y,w,h], i) => paint(a,x,y,w,h,[i+1,0,0,255]));
  const p = playerPixels(a)!;
  [[6,0],[6,8],[2,8],[14,8],[6,20],[10,20]].forEach(([x,y],i) => assert.deepEqual(pixel(p,x,y),[i+1,0,0,255]));
});
it("slim models have three-pixel arms and classic models have four", () => {
  for (const model of ["classic", "slim"] as const) {
    const a = fixture(64, model); paint(a,44,20,4,12,[1,2,3,255]); paint(a,36,52,4,12,[4,5,6,255]);
    const p = playerPixels(a)!;
    assert.equal(pixel(p,2,8)[3],model === "slim" ? 0 : 255);
    assert.equal(pixel(p,17,8)[3],model === "slim" ? 0 : 255);
    assert.deepEqual(pixel(p,3,8),[1,2,3,255]);
  }
});
it("legacy limbs mirror their original arm and leg without reading modern UVs", () => {
  const a=fixture(32); paint(a,44,20,1,12,[10,0,0,255]); paint(a,47,20,1,12,[20,0,0,255]);
  paint(a,4,20,1,12,[30,0,0,255]); paint(a,7,20,1,12,[40,0,0,255]);
  const p=playerPixels(a)!;
  assert.deepEqual(pixel(p,14,8),[20,0,0,255]); assert.deepEqual(pixel(p,17,8),[10,0,0,255]);
  assert.deepEqual(pixel(p,10,20),[40,0,0,255]); assert.deepEqual(pixel(p,13,20),[30,0,0,255]);
});
it("all modern outer layers composite alpha over their corresponding body faces", () => {
  const a=fixture();
  const faces=[[8,8,40,8,8,8,6,0],[20,20,20,36,8,12,6,8],[44,20,44,36,4,12,2,8],[36,52,52,52,4,12,14,8],[4,20,4,36,4,12,6,20],[20,52,4,52,4,12,10,20]];
  for (const [bx,by,ox,oy,w,h] of faces) { paint(a,bx,by,w,h,[100,0,0,255]); paint(a,ox,oy,w,h,[0,100,0,128]); }
  const p=playerPixels(a)!;
  for (const face of faces) assert.deepEqual(pixel(p,face[6],face[7]),[50,50,0,255]);
});
it("missing and malformed cosmetics never yield body pixels", () => {
  assert.equal(playerPixels(null),null); const a=fixture(); a.skinRgba.pop(); assert.equal(playerPixels(a),null);
  const b=fixture(); b.skinRgba[0]=NaN; assert.equal(playerPixels(b),null);
});
