import {readFile,writeFile,mkdir} from 'node:fs/promises';
import assert from 'node:assert/strict';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE||'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9436');
try{
 const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));assert.ok(page,'Compiled Tauri frontend required');
 await page.locator('.cosmetics-page .page-title').waitFor();
 const name=process.argv[2],folder=process.argv[3]??'native';assert.match(name,/^[a-z0-9-]+$/);assert.match(folder,/^native(-restart)?$/);
 const bytes=await readFile(`docs/ui-4/${folder}/start.json`),start=JSON.parse(bytes.toString(bytes[0]===255?'utf16le':'utf8').replace(/^\uFEFF/,''));
 assert.equal((await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_application_status'))).managedDataRoot,start.managed);
 await page.waitForTimeout(400); // Let native smooth scrolling/paint settle before measurement.
 const metrics=await page.evaluate(()=>{
  const main=document.querySelector('main.content'),stage=document.querySelector('.preview-stage'),canvas=stage?.querySelector('canvas');
  let bounds=null;if(canvas){const data=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;let left=canvas.width,top=canvas.height,right=0,bottom=0;for(let i=3;i<data.length;i+=4)if(data[i]>0){const x=((i-3)/4)%canvas.width,y=Math.floor((i-3)/4/canvas.width);left=Math.min(left,x);right=Math.max(right,x);top=Math.min(top,y);bottom=Math.max(bottom,y);}bounds={left,top,right,bottom,width:canvas.width,height:canvas.height};}
  return {provenance:'Actual compiled native frontend; read-only CDP screenshot/geometry; synthetic local imports, no account',url:location.href,viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,previewGeometry:document.querySelector('select[aria-label="Model preview"]')?.value,savedGeometry:document.querySelector('select[aria-label="Saved skin model"]')?.value,outerLayers:[...document.querySelectorAll('.model-controls input')].map(e=>e.checked),documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:main.scrollWidth>main.clientWidth+1,stage:stage?{width:stage.clientWidth,height:stage.clientHeight,orientation:stage.getAttribute('aria-valuetext')}:null,pixelBounds:bounds,text:main.innerText};
 });
 assert.equal(metrics.documentOverflow||metrics.contentOverflow,false);
 await mkdir(`docs/ui-4/${folder}`,{recursive:true});await writeFile(`docs/ui-4/${folder}/${name}.json`,JSON.stringify(metrics,null,2));await page.screenshot({path:`docs/ui-4/${folder}/${name}.png`});console.log(JSON.stringify({...metrics,text:metrics.text.slice(0,160)}));
}finally{await browser.close();}
