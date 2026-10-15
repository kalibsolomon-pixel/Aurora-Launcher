import {readFile,writeFile,mkdir} from 'node:fs/promises';
import assert from 'node:assert/strict';
const {chromium}=await import('file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9437');
try {
 const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));assert.ok(page,'Actual compiled native frontend required');
 const name=process.argv[2],folder=process.argv[3]??'native';assert.match(name,/^[a-z0-9-]+$/);assert.match(folder,/^native(-[a-z0-9-]+)?$/);
 const start=JSON.parse((await readFile(`docs/ui-5/${folder}/start.json`,'utf8')).replace(/^\uFEFF/,''));
 const status=await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_application_status'));
 assert.equal(status.managedDataRoot,start.managed);
 await page.waitForTimeout(180);
 const metrics=await page.evaluate(()=>{
  const main=document.querySelector('main.content');
  return {provenance:'Actual compiled native frontend; no fixture substitution; read-only CDP screenshot and geometry',url:location.href,
   viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,category:document.querySelector('.settings-nav [aria-current]')?.textContent,
   documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:main.scrollWidth>main.clientWidth+1,text:main.innerText,
   controls:[...main.querySelectorAll('input,button,select')].map(e=>{const r=e.getBoundingClientRect();return {label:e.getAttribute('aria-label')??e.textContent,value:e.type==='checkbox'||e.type==='radio'?e.checked:e.value,disabled:e.disabled,x:r.x,y:r.y,width:r.width,height:r.height};})};
 });
 assert.equal(metrics.documentOverflow||metrics.contentOverflow,false);
 const preferences=await page.evaluate(async()=>({appearance:await window.__TAURI_INTERNALS__.invoke('get_appearance'),widgets:await window.__TAURI_INTERNALS__.invoke('get_home_widgets'),discord:await window.__TAURI_INTERNALS__.invoke('get_discord_state')}));
 assert.equal(preferences.discord.preferences.enabled,false,'Native acceptance must keep Discord disabled');
 await mkdir(`docs/ui-5/${folder}`,{recursive:true});await writeFile(`docs/ui-5/${folder}/${name}.json`,JSON.stringify({...metrics,preferences},null,2));await page.screenshot({path:`docs/ui-5/${folder}/${name}.png`});console.log(name,JSON.stringify({...metrics,text:undefined,controls:undefined}));
}finally{await browser.close();}
