import assert from "node:assert/strict";
import { it } from "node:test";
import { playerModel, renderPlayer } from "./playerModel.ts";
const avatar = (model="classic",skinHeight:32|64=64) => ({ model, skinHeight, rgba:Array(256).fill(255), skinRgba:Array(64*skinHeight*4).fill(255) });
it("static 3D model contains six cuboid body parts and six expanded overlay parts", () => {
  const model=playerModel(avatar());
  assert.equal(model.faces.length,72); assert.equal(model.fallback,false);
  assert.deepEqual(model.faces[0].uv,[8,8,8,8]);
  assert.deepEqual(model.faces[12].uv,[20,20,8,12]);
  assert.notEqual(model.faces[0].points[0][2],model.faces[0].points[1][2]);
});
it("authoritative slim geometry changes arm widths without changing torso or legs", () => {
  assert.equal(playerModel(avatar("slim")).faces[18].uv[2],3);
  assert.equal(playerModel(avatar()).faces[18].uv[2],4);
  assert.equal(playerModel(avatar("slim")).faces[30].uv[2],4);
});
it("legacy models mirror limbs and omit nonexistent outer body layers", () => {
  const model=playerModel(avatar("slim",32));
  assert.equal(model.faces.length,42); assert.equal(model.faces[24].mirror,true);
  assert.equal(model.faces[24].uv[2],4);
  assert.ok(model.faces.every(face=>face.uv[1]+face.uv[3]<=32));
});
it("missing or malformed pixels render a bounded default 3D player", () => {
  for(const value of [null,{...avatar(),skinRgba:[0]}, {...avatar(),model:"unknown"}]) {
    const model=playerModel(value); assert.equal(model.fallback,true); assert.equal(model.pixels.length,16384);
  }
});
it("software render uses actual texture pixels and transparent overlays without animation", () => {
  const image={data:new Uint8ClampedArray(420*600*4)};
  const context={createImageData:()=>image,putImageData:(value:unknown)=>assert.equal(value,image)} as unknown as CanvasRenderingContext2D;
  const model=playerModel(avatar()); model.pixels.fill(0);
  renderPlayer(context,model); assert.ok(image.data.every(value=>value===0));
  for(let i=0;i<model.pixels.length;i+=4) { model.pixels[i]=200; model.pixels[i+3]=255; }
  renderPlayer(context,model); assert.ok(image.data.some(value=>value===200));
  assert.ok(image.data.filter((_,i)=>i%4===3 && image.data[i]===255).length>10000);
});
