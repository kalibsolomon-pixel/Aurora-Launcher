import assert from 'node:assert/strict';
import { it } from 'node:test';
import { motionRate, motionAllowed, mountBorealis } from './borealis.ts';

it('video speed is bounded independently from motion accessibility policy', () => {
  assert.equal(motionRate(-10), .5); assert.equal(motionRate(50), 1);
  assert.equal(motionRate(120), 1.5); assert.equal(motionRate(NaN), 1);
  assert.equal(motionAllowed(false,true,false),true);
  for (const state of [[true,true,false],[false,false,false],[false,true,true]]) {
    assert.equal(motionAllowed(...state as [boolean,boolean,boolean]),false);
  }
});
it('video pauses on hidden/blur/reduced motion, preserves rate and releases decoder on unmount', async () => {
  const names = ['document','window','matchMedia'] as const;
  const originals = new Map(names.map(name=>[name,Object.getOwnPropertyDescriptor(globalThis,name)]));
  let focused=true, plays=0, pauses=0, loads=0;
  const doc=Object.assign(new EventTarget(),{hidden:false,hasFocus:()=>focused});
  const win=new EventTarget(), media=Object.assign(new EventTarget(),{matches:true});
  const video=Object.assign(new EventTarget(),{dataset:{} as Record<string,string>,style:{opacity:''},src:'',playbackRate:1.5,
    play:async()=>{plays++;},pause:()=>{pauses++;},load:()=>{loads++;},removeAttribute:()=>{video.src='';}});
  try {
    Object.assign(globalThis,{document:doc,window:win,matchMedia:()=>media});
    const dispose=mountBorealis(video as unknown as HTMLVideoElement);
    assert.equal(plays,0); assert.equal(loads,0); assert.equal(video.dataset.motion,'reduced');
    media.matches=false; media.dispatchEvent(new Event('change')); await Promise.resolve();
    assert.equal(plays,1); assert.equal(loads,1);
    video.dispatchEvent(new Event('playing')); assert.equal(video.dataset.motion,'running');
    doc.hidden=true; doc.dispatchEvent(new Event('visibilitychange')); assert.equal(video.dataset.motion,'paused');
    const paused=pauses; doc.hidden=false; doc.dispatchEvent(new Event('visibilitychange'));
    focused=false; win.dispatchEvent(new Event('blur')); assert(pauses>paused);
    focused=true; win.dispatchEvent(new Event('focus'));
    media.matches=true; media.dispatchEvent(new Event('change')); assert.equal(video.style.opacity,'0');
    assert.equal(video.playbackRate,1.5);
    media.matches=false; media.dispatchEvent(new Event('change'));
    video.dispatchEvent(new Event('error')); assert.equal(video.dataset.motion,'fallback');
    const final=plays; win.dispatchEvent(new Event('focus')); assert.equal(plays,final);
    dispose(); assert.equal(video.src,''); assert.equal(loads,2);
    win.dispatchEvent(new Event('focus')); assert.equal(plays,final);
  } finally {
    for(const name of names){ const descriptor=originals.get(name); if(descriptor) Object.defineProperty(globalThis,name,descriptor); else delete (globalThis as any)[name]; }
  }
});
