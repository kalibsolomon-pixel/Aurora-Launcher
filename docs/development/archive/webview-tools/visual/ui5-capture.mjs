import {mkdir,writeFile} from 'node:fs/promises';
import {openSettings,settingsModules} from './ui5-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE||'file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const phase=process.argv[2];if(!['before','pilot','after'].includes(phase))throw Error('Expected before, pilot or after');
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({reducedMotion:'reduce'}),out=`docs/ui-5/${phase}`;
await mkdir(out,{recursive:true});await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
const metrics=[];
try {
 for(const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520]])for(const category of ['General','Appearance','Home','Updates','Discord & privacy']) {
  await page.setViewportSize({width,height});await openSettings(page,category);
  await page.locator('main.content').evaluate(e=>e.scrollTop=0);await page.waitForTimeout(80);
  metrics.push(await page.evaluate(category=>({category,viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,
   overflow:document.documentElement.scrollWidth>innerWidth||document.querySelector('main.content').scrollWidth>document.querySelector('main.content').clientWidth+1}),category));
  await page.screenshot({path:`${out}/${category.split(' ')[0].toLowerCase()}-${width}x${height}.png`});
 }
 await writeFile(`${out}/metrics.json`,JSON.stringify({provenance:'Browser fixtures, synthetic data',metrics},null,2));
 console.log(phase,metrics.length,'screenshots',metrics.filter(m=>m.overflow));
}finally{await browser.close();}
