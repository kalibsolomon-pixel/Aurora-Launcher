import { playerModel, renderPlayer } from '../../src/lib/launcher/playerModel.ts';
import { performance } from 'node:perf_hooks';
const context = {createImageData:(w,h)=>({data:new Uint8ClampedArray(w*h*4)}),putImageData(){}};
const model=playerModel(null), samples=[];
for(let i=0;i<35;i++){ const start=performance.now(); renderPlayer(context,model,-.42+i*.02); if(i>=5)samples.push(performance.now()-start); }
samples.sort((a,b)=>a-b);
console.log(JSON.stringify({renders:samples.length,meanMs:samples.reduce((a,b)=>a+b,0)/samples.length,p95Ms:samples[Math.ceil(samples.length*.95)-1],canvasBytes:420*600*4,method:'Node software rasterization; allocation included, native canvas upload excluded'}));
