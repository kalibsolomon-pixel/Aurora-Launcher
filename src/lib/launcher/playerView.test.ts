import assert from 'node:assert/strict';
import {it} from 'node:test';
import {DEFAULT_VIEW,playerView,MIN_ZOOM,MAX_ZOOM} from './playerView.ts';
import {playerModel,renderPlayer,PLAYER_RENDER_WIDTH as W,PLAYER_RENDER_HEIGHT as H} from './playerModel.ts';
it('camera clamps extreme and nonfinite values and normalizes repeated rotation',()=>{
  assert.deepEqual(playerView(NaN,Infinity,NaN),DEFAULT_VIEW);
  assert.equal(playerView(1000,100,100).pitch,.4);assert.equal(playerView(-1000,-100,-100).pitch,-.35);
  assert.equal(playerView(0,0,100).zoom,MAX_ZOOM);assert.equal(playerView(0,0,-100).zoom,MIN_ZOOM);
  assert.ok(Math.abs(playerView(10000,0,1).yaw)<=Math.PI);
});
it('outer layers are optional for both modern and legacy skins; capes remain independent',()=>{
  const avatar={model:'classic',skinHeight:64,skinRgba:Array(64*64*4).fill(255)} as any;
  const cape={width:64,height:32,rgba:Array(64*32*4).fill(255)};
  assert.equal(playerModel(avatar,null,false).faces.length,36);
  assert.equal(playerModel(avatar,cape,false).faces.length,42);
  assert.equal(playerModel({...avatar,skinHeight:32,skinRgba:Array(64*32*4).fill(255)},null,false).faces.length,36);
});
it('default view preserves Home pixels exactly and full body stays inside all bounded views',()=>{
  let pixels:Uint8ClampedArray;
  const context={createImageData:(w:number,h:number)=>({data:new Uint8ClampedArray(w*h*4)}),putImageData:(frame:any)=>pixels=frame.data} as any;
  const model=playerModel(null);
  renderPlayer(context,model); const original=pixels!.slice();
  renderPlayer(context,model,DEFAULT_VIEW.yaw,W,H,DEFAULT_VIEW);assert.deepEqual(pixels!,original);
  for(const yaw of [-Math.PI,-1,0,1,Math.PI])for(const pitch of [-.35,.1,.4]) {
    renderPlayer(context,model,yaw,W,H,playerView(yaw,pitch,MAX_ZOOM));
    const occupied:number[]=[];for(let i=3;i<pixels!.length;i+=4)if(pixels![i]>0)occupied.push((i-3)/4);
    assert.ok(occupied.length>1000);
    assert.ok(occupied.every(i=>i%W>0 && i%W<W-1 && Math.floor(i/W)>0 && Math.floor(i/W)<H-1),`bounded view ${yaw}/${pitch}`);
  }
});
