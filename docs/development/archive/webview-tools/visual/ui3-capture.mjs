import {mkdir,writeFile} from 'node:fs/promises';
import {openContentReview,contentGeometry} from './ui3-fixture.mjs';
import assert from 'node:assert/strict';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({reducedMotion:'reduce'});
const errors=[],metrics={};page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,route=>route.abort());
const out=`docs/ui-3/${process.argv[2] ?? 'after'}`;
await mkdir(out,{recursive:true});
try {
  for (const surface of ['mods','modpacks','packs','installed-mods','installed-packs']) {
    for (const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520]]) {
      await page.setViewportSize({width,height});await openContentReview(page,surface);
      metrics[`${surface}-${width}x${height}`]=await contentGeometry(page);
      assert.equal(metrics[`${surface}-${width}x${height}`].contentOverflow,false);
      await page.screenshot({path:`${out}/${surface}-${width}x${height}.png`});
    }
  }
  for (const mode of ['empty','error','loading']) {
    await page.setViewportSize({width:1280,height:800});await openContentReview(page);
    await page.evaluate(mode=>window.ui3Mode=mode,mode);
    await page.getByRole('searchbox').fill('fixture');await page.locator('.browse-search').getByRole('button',{name:'Search',exact:true}).click();
    await page.waitForTimeout(200);await page.screenshot({path:`${out}/mods-${mode}.png`});
    if(mode==='loading')await page.evaluate(()=>{window.ui3Mode='normal';window.ui3Pending?.();});
  }
  await writeFile(`${out}/geometry.json`,JSON.stringify(metrics,null,2));
  assert.deepEqual(errors,[]);console.log(`PASS 23 captures (${out}); zero page errors`);
} finally {await browser.close();}
