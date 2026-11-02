// Read-only capture of the compiled app, never the browser review entry.
import {writeFile,readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9433');
try {
  const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));
  assert.ok(page,'Actual compiled native WebView required');
  const startBytes=await readFile('docs/ui-3/native/start.json');
  const start=JSON.parse(startBytes.toString(startBytes[0]===255 && startBytes[1]===254 ? 'utf16le' : 'utf8').replace(/^\uFEFF/,''));
  const nativeStatus=await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_application_status'));
  assert.equal(nativeStatus.managedDataRoot,start.managed,'Native app must still use this run isolated root');
  const metrics=await page.evaluate(async()=>{
    const main=document.querySelector('main.content');
    const rect=selector=>{const e=document.querySelector(selector);if(!e)return null;const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom};};
    return {provenance:'Compiled Tauri frontend, actual Rust commands, isolated native data; read-only CDP screenshot/measurement',url:location.href,viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,scale:await window.__TAURI_INTERNALS__.invoke('plugin:window|scale_factor',{label:'main'}),documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:main.scrollWidth>main.clientWidth+1,background:document.documentElement.dataset.background,accent:document.documentElement.dataset.accent,browse:rect('.browse'),firstResult:rect('.browse-row'),results:document.querySelectorAll('.browse-row').length,libraryRows:document.querySelectorAll('.mod-row,.pack-row').length,search:rect('.browse-search input'),filters:rect('.browse-filters'),account:rect('.account-chip'),text:main.innerText};
  });
  assert.equal(metrics.documentOverflow||metrics.contentOverflow,false);
  const name=process.argv[2];assert.match(name,/^[a-z0-9-]+$/);
  await writeFile(`docs/ui-3/native/${name}.json`,JSON.stringify(metrics,null,2));
  await page.screenshot({path:`docs/ui-3/native/${name}-client.png`});
  console.log(JSON.stringify({...metrics,text:metrics.text.slice(0,300)}));
} finally {await browser.close();}
