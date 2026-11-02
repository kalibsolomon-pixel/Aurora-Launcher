// Real native artwork acquisition, synthetic inventory. Requires a new disposable
// managed root passed to the isolated debug app. Never deletes/reset caches.
import assert from 'node:assert/strict';
import {readFile,readdir,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {seed} from './ui-corrections-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE||'playwright');
assert.ok(process.env.UI_MANAGED_ROOT&&process.env.UI_EVIDENCE_DIR);
const cache=path.join(process.env.UI_MANAGED_ROOT,'cache/artwork/modrinth');
const snapshot=async()=>Object.fromEntries(await Promise.all((await readdir(cache).catch(e=>{if(e.code==='ENOENT')return [];throw e;})).filter(n=>n.endsWith('.img')).map(async n=>[n,createHash('sha256').update(await readFile(path.join(cache,n))).digest('hex')])));
const audit=JSON.parse(await readFile('docs/l1/artwork-audit.json','utf8'));
const ids=[...new Set(audit.sample.map(p=>p.id))].slice(0,20);assert.equal(ids.length,20);
const browser=await chromium.connectOverCDP(process.env.UI_CDP_URL||'http://127.0.0.1:9237');
const page=browser.contexts()[0].pages()[0],results=[];
const q=(a,p)=>a.slice().sort((a,b)=>a-b)[Math.min(a.length-1,Math.floor(a.length*p))];
try{
  const review=await page.evaluate(async expected=>{
    const invoke=window.__TAURI_INTERNALS__.invoke;
    const identifier=await invoke('plugin:app|identifier'),status=await invoke('get_application_status');
    return {identifier,rootMatches:status.managedDataRoot.replaceAll('\\','/').toLowerCase()===expected.replaceAll('\\','/').toLowerCase()};
  },path.resolve(process.env.UI_MANAGED_ROOT));
  assert.match(review.identifier,/^com\.aurora\.launcher\..+-review$/,'requires an isolated review identifier');
  assert.equal(review.rootMatches,true,'UI_MANAGED_ROOT must match the actual native diagnostic root');
  assert.equal(Object.keys(await snapshot()).length,0,'cold run requires a fresh isolated cache; do not clear an existing root');
  for(const temperature of ['cold','warm']){
    await seed(page,'installed',undefined,{nativeArtwork:ids,settleMs:0});
    await page.evaluate(()=>{window.cacheFrames=[];window.cacheActive=true;window.cacheLong=[];window.cacheObserver=new PerformanceObserver(l=>window.cacheLong.push(...l.getEntries().map(e=>e.duration)));window.cacheObserver.observe({type:'longtask'});let last;requestAnimationFrame(function frame(t){if(last)window.cacheFrames.push(t-last);last=t;if(window.cacheActive)requestAnimationFrame(frame);});});
    const rect=await page.locator('main.content').boundingBox();await page.mouse.move(rect.x+rect.width*.7,rect.y+rect.height*.6);
    for(let i=0;i<60;i++){await page.mouse.wheel(0,i<30?100:-100);await page.waitForTimeout(16);}
    await page.waitForFunction(()=>window.uiArtworkRecords.length===100,{},{timeout:60000});
    await page.waitForFunction(()=>[...document.querySelectorAll('.mod-row img')].filter(i=>{const b=i.getBoundingClientRect();return b.bottom>40&&b.top<innerHeight;}).every(i=>i.complete&&i.naturalWidth>0));
    const data=await page.evaluate(()=>{window.cacheActive=false;window.cacheObserver.disconnect();return {frames:window.cacheFrames,longTasks:window.cacheLong,records:window.uiArtworkRecords,requests:window.uiRequests.resolve_project_artwork,visibleDecoded:true,decoded:[...document.querySelectorAll('.mod-row img')].filter(i=>i.complete&&i.naturalWidth>0).length};});
    assert.equal(data.records.filter(r=>r.status==='available').length,100);
    const timings=data.records.map(r=>r.ms);
    const result={temperature,publicProjects:ids,rows:100,requests:data.requests,decoded:data.decoded,visibleDecoded:data.visibleDecoded,acquisitionMedianMs:q(timings,.5),acquisitionP95Ms:q(timings,.95),frameMedianMs:q(data.frames,.5),frameP95Ms:q(data.frames,.95),frameP99Ms:q(data.frames,.99),frameSamples:data.frames.length,over25:data.frames.filter(t=>t>25).length,longTasks:data.longTasks};
    results.push(result);console.log(JSON.stringify(result));
    const disk=await snapshot();assert.equal(Object.keys(disk).length,20);
    if(temperature==='cold')await writeFile(process.env.UI_EVIDENCE_DIR+'/cache-hashes.json',JSON.stringify(disk));
    else assert.deepEqual(disk,JSON.parse(await readFile(process.env.UI_EVIDENCE_DIR+'/cache-hashes.json','utf8')));
  }
  await writeFile(process.env.UI_EVIDENCE_DIR+'/artwork-cache.json',JSON.stringify({results,warmCacheBytesUnchanged:true},null,2));
}finally{await browser.close();}
