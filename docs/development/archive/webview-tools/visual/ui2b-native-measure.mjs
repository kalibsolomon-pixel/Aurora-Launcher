import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9432');
try {
  const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));
  assert.ok(page,'Compiled native WebView2 page required');
  const metrics=await page.evaluate(async()=>{
    const rect=s=>{const e=document.querySelector(s);if(!e)return null;const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom};};
    const c=document.querySelector('main.content');
    return {provenance:'Actual Tauri WebView2 client; compiled production frontend; real native commands against a fresh isolated Installing registry; CDP read-only measurement',url:location.href,viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,osScale:await window.__TAURI_INTERNALS__.invoke('plugin:window|scale_factor',{label:'main'}),theme:document.documentElement.dataset.theme,background:document.documentElement.dataset.background,accent:document.documentElement.dataset.accent,documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:c.scrollWidth>c.clientWidth+1,scrollExtent:c.scrollHeight-c.clientHeight,hero:rect('.home-composition'),instance:rect('.instance-card'),launch:rect('.play-hero'),player:rect('.player-preview'),canvas:rect('.player-preview canvas'),widgets:rect('.home-widgets'),cards:[...document.querySelectorAll('[data-home-widget]')].map(e=>({id:e.dataset.homeWidget,width:e.getBoundingClientRect().width})),rail:rect('.sidebar'),account:rect('.account-chip'),disabled:document.querySelector('.play-hero')?.disabled,blockers:document.querySelector('.launch-feedback')?.textContent?.trim()};
  });
  assert.equal(metrics.documentOverflow||metrics.contentOverflow,false);
  const name=process.argv[2]??'measurement';
  const out='docs/ui-2b/native-final';
  await writeFile(`${out}/${name}.json`,JSON.stringify(metrics,null,2));
  await page.screenshot({path:`${out}/${name}-client.png`});
  console.log(JSON.stringify(metrics));
} finally {await browser.close();}
