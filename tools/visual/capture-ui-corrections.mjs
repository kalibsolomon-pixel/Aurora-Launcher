// Capture the current source in the isolated native review window. Baseline
// captures belong to the starting revision; never synthesize old DOM from CSS.
import {mkdir,writeFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {seed} from './ui-corrections-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE||'playwright');
assert.ok(process.env.UI_CAPTURE_DIR,'use an explicit disposable capture directory');
const out=process.env.UI_CAPTURE_DIR,root=process.cwd().replaceAll('\\','/');
await mkdir(out,{recursive:true});
const browser=await chromium.connectOverCDP(process.env.UI_CDP_URL||'http://127.0.0.1:9237');
const page=browser.contexts()[0].pages()[0],records=[];
async function resize(width,height){
  await page.evaluate(async({width,height,root})=>{const {getCurrentWindow,LogicalSize}=await import('/@fs/'+root+'/node_modules/@tauri-apps/api/window.js');await getCurrentWindow().setSize(new LogicalSize(width,height));},{width,height,root});
  // Fractional Windows display scaling can round the native size by one pixel.
  await page.waitForFunction(({width,height})=>Math.abs(innerWidth-width)<=1&&Math.abs(innerHeight-height)<=1,{width,height});
}
async function shot(name){
  await page.waitForTimeout(150);await page.screenshot({path:path.join(out,name+'.png')});
  records.push({name,...await page.evaluate(()=>({viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,overflow:document.documentElement.scrollWidth>innerWidth,scrollTop:document.querySelector('main.content')?.scrollTop}))});
}
try{
  assert.match(await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('plugin:app|identifier')),/^com\.aurora\.launcher\..+-review$/,'requires an isolated review identifier');
  await page.goto('http://127.0.0.1:1422/?tab=mods');await resize(1600,1000);await seed(page,'installed');await shot('01-installed-top');
  await page.locator('main.content').evaluate(e=>e.scrollTop=2200);await shot('02-installed-middle');await page.locator('main.content').evaluate(e=>e.scrollTop=0);
  await page.getByRole('button',{name:'Metadata warnings for More Culling',exact:true}).click();await shot('03-warning');await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'Metadata warnings for Reese’s Sodium Options',exact:true}).click();await shot('04-multiple-warnings');await page.keyboard.press('Escape');
  for(const [kind,name] of [['mod','05-browse-mods'],['resourcePack','06-browse-resourcepacks'],['shaderPack','07-browse-shaders'],['modpack','08-browse-modpacks']]){await seed(page,kind);await shot(name);}
  await seed(page,'installed');
  for(const [time,name] of [[3,'09-navigation-bright'],[12,'10-navigation-dark']]){
    await page.evaluate(async time=>{const v=document.querySelector('video');v.pause();const seek=new Promise(r=>v.addEventListener('seeked',r,{once:true}));v.currentTime=time;await seek;},time);await shot(name);
  }
  await resize(900,930);await seed(page,'installed');await shot('11-installed-narrow');await seed(page,'mod');await shot('12-browse-narrow');
  await resize(1600,1000);await page.goto('http://127.0.0.1:1422/?tab=settings&background=borealis');await page.locator('.workspace-tab-active').waitFor();await shot('13-settings');
  await page.goto('http://127.0.0.1:1422/?l3&page=cosmetics&background=borealis');await page.locator('.cosmetics-page').waitFor();await page.waitForTimeout(800);await shot('14-cosmetics');
  await writeFile(path.join(out,'captures.json'),JSON.stringify(records,null,2));console.log('Captured 14 actual native WebView2 views');
}finally{await browser.close();}
