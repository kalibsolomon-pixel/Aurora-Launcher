const { chromium } = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
import { writeFile } from 'node:fs/promises';
const browser = await chromium.connectOverCDP(process.env.UI_CDP_URL || 'http://127.0.0.1:9237');
const page = browser.contexts()[0].pages()[0];
const cdp = await page.context().newCDPSession(page);
const out = process.env.UI_EVIDENCE_DIR + '/';
import { seed } from './ui-corrections-fixture.mjs';
const quantile=(a,p)=>a.slice().sort((a,b)=>a-b)[Math.min(a.length-1,Math.floor(a.length*p))]??null;
async function trial(surface,variant,run){
  await seed(page, surface, process.env.UI_BASE_URL);
  if(variant==='baseline')await page.addStyleTag({content:'.f-pilot .mod-list,.f-pilot .packs-list,.f-pilot .browse-row,.f-pilot .project,.f-pilot .preview{background:var(--f-panel)!important;backdrop-filter:var(--f-blur)!important}'});
  if(variant==='no-list-blur')await page.addStyleTag({content:'.f-pilot .mod-list,.f-pilot .packs-list,.f-pilot .browse-row,.f-pilot .project,.f-pilot .preview{backdrop-filter:none!important}'});
  if(variant==='previous-backing')await page.addStyleTag({content:'.f-pilot.workspace-page,.f-pilot .modpack-group{background:var(--f-content-panel)!important;backdrop-filter:none!important}.f-pilot.workspace-page{box-shadow:none!important}.f-pilot.workspace-page::before,.f-pilot .modpack-group::before{content:none!important}'});
  if(variant==='still')await page.evaluate(()=>document.querySelector('video')?.pause());
  await page.locator('main.content').evaluate(e=>e.scrollTop=600);
  await page.waitForTimeout(300);
  await cdp.send('Performance.enable');
  const before=await cdp.send('Performance.getMetrics');
  if(process.env.UI_TRACE)await cdp.send('Tracing.start',{categories:'benchmark,cc,viz,gpu,input,latencyInfo,devtools.timeline,disabled-by-default-devtools.timeline.frame',transferMode:'ReturnAsStream'});
  await page.evaluate(()=>{window.uiFrames=[];window.uiLong=[];window.uiChanges=0;window.uiObserving=true;window.uiObserver=new MutationObserver(r=>window.uiChanges+=r.length);window.uiObserver.observe(document.querySelector('main.content'),{subtree:true,attributes:true,childList:true});window.uiLongObserver=new PerformanceObserver(l=>window.uiLong.push(...l.getEntries().map(e=>e.duration)));window.uiLongObserver.observe({type:'longtask',buffered:false});let last;function frame(t){if(last)window.uiFrames.push(t-last);last=t;if(window.uiObserving)requestAnimationFrame(frame);}requestAnimationFrame(frame);});
  const rect=await page.locator('main.content').boundingBox();
  await page.mouse.move(rect.x+rect.width*.7,rect.y+rect.height*.6);
  for(let i=0;i<60;i++){await page.mouse.wheel(0,i<30?100:-100);await page.waitForTimeout(16);}
  const data=await page.evaluate(()=>{window.uiObserving=false;window.uiObserver.disconnect();window.uiLongObserver.disconnect();return {frames:window.uiFrames,longTasks:window.uiLong,mutations:window.uiChanges,requests:window.uiRequests,nodes:document.getElementsByTagName('*').length,images:[...document.images].filter(i=>i.complete&&i.naturalWidth>0).length,scrollTop:document.querySelector('main.content').scrollTop,video:document.querySelector('video')?.dataset.motion,dpr:devicePixelRatio,viewport:[innerWidth,innerHeight]};});
  const after=await cdp.send('Performance.getMetrics');const metrics=Object.fromEntries(after.metrics.map(m=>[m.name,m.value-(before.metrics.find(v=>v.name===m.name)?.value??0)]));
  if(process.env.UI_TRACE){const completed=new Promise(resolve=>cdp.once('Tracing.tracingComplete',resolve));await cdp.send('Tracing.end');const {stream}=await completed;let raw='';for(;;){const part=await cdp.send('IO.read',{handle:stream});raw+=part.data;if(part.eof)break;}await cdp.send('IO.close',{handle:stream});await writeFile(out+`trace-${surface}-${variant}-${run}.json`,raw);}
  const result={surface,variant,run,median:quantile(data.frames,.5),p95:quantile(data.frames,.95),p99:quantile(data.frames,.99),over25:data.frames.filter(v=>v>25).length,over50:data.frames.filter(v=>v>50).length,...data,metrics};
  console.log(JSON.stringify({surface,variant,run,median:result.median,p95:result.p95,p99:result.p99,over25:result.over25,frames:data.frames.length,mutations:data.mutations,video:data.video}));return result;
}
if(process.argv[2]==='seed'){await seed(page, process.argv[3]??'installed');}
else {const results=[];for(const surface of (process.env.UI_SURFACES??'installed,mod,resourcePack,shaderPack,modpack').split(','))for(let run=0;run<3;run++)for(const variant of (process.env.UI_VARIANTS??'baseline,no-list-blur').split(','))results.push(await trial(surface,variant,run));await writeFile(out+(process.argv[2]??'profile')+'.json',JSON.stringify(results,null,2));}
await browser.close();
