import {writeFile} from 'node:fs/promises';
const {chromium} = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.connectOverCDP('http://127.0.0.1:9432');
try {
  const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));
  if(!page)throw new Error('Isolated native WebView missing');
  const metrics=await page.evaluate(async()=>{
    const rect=s=>{const e=document.querySelector(s);if(!e)return null;const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom};};
    const content=document.querySelector('main.content');
    return {provenance:'Actual native Tauri WebView2 client, embedded production frontend, isolated synthetic native registry; CDP read-only geometry',viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,scaleFactor:await window.__TAURI_INTERNALS__.invoke('plugin:window|scale_factor',{label:'main'}),documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:content?content.scrollWidth>content.clientWidth+1:null,home:rect('.home-composition'),instance:rect('.instance-card'),play:rect('.play-hero'),preview:rect('.player-preview'),widgets:rect('.home-widgets'),heading:document.querySelector('.home-widgets h3')?.textContent};
  });
  const name=process.argv[2]??'measurement';
  await writeFile(`docs/ui-2/native/${name}.json`,JSON.stringify(metrics,null,2));
  // This is the actual native WebView, not the synthetic browser fixture.
  await page.screenshot({path:`docs/ui-2/native/${name}-client.png`});
  console.log(JSON.stringify(metrics));
} finally {await browser.close();}
