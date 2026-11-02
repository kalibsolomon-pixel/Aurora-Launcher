import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE||'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9434');
try {
  const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));
  assert.ok(page,'Actual compiled native frontend required');
  const bytes=await readFile('docs/ui-3-1/native/start.json');
  const start=JSON.parse(bytes.toString(bytes[0]===255&&bytes[1]===254?'utf16le':'utf8').replace(/^\uFEFF/,''));
  assert.equal((await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_application_status'))).managedDataRoot,start.managed);
  const metrics=await page.evaluate(()=>{
    const main=document.querySelector('main.content'),list=document.querySelector('.browse-list'),row=document.querySelector('.browse-row');
    const rect=e=>{if(!e)return null;const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom};};
    return {provenance:'Actual compiled Tauri frontend; real Rust Modrinth results; new isolated data; read-only CDP measurement/capture',url:location.href,viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:main.scrollWidth>main.clientWidth+1,columns:list?getComputedStyle(list).gridTemplateColumns:null,results:document.querySelectorAll('.browse-row').length,row:rect(row),artwork:rect(row?.querySelector('.browse-glyph')),actions:rect(row?.querySelector('.browse-actions')),rowOverflow:row?row.scrollWidth>row.clientWidth+1:false,text:main.innerText};
  });
  assert.equal(metrics.documentOverflow||metrics.contentOverflow||metrics.rowOverflow,false);
  if(metrics.columns)assert.equal(metrics.columns.split(' ').length,1);
  const name=process.argv[2];assert.match(name,/^[a-z0-9-]+$/);
  await writeFile(`docs/ui-3-1/native/${name}.json`,JSON.stringify(metrics,null,2));
  await page.screenshot({path:`docs/ui-3-1/native/${name}.png`});
  console.log(JSON.stringify({...metrics,text:metrics.text.slice(0,170)}));
} finally {await browser.close();}
