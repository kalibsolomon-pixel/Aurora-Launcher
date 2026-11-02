import {mkdir, writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {homeFixture, homeGeometry} from './ui2-fixture.mjs';
const phase = process.argv[2];
assert.ok(['before', 'after'].includes(phase));
const {chromium} = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({headless:true});
const page = await browser.newPage({deviceScaleFactor:1, reducedMotion:'reduce'});
const errors = [], records = [];
page.on('pageerror', e => errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/, r => r.abort());
const out = `docs/ui-2/${phase}`;
await mkdir(out, {recursive:true});
try {
  for (const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520]]) {
    await page.setViewportSize({width,height});
    await homeFixture(page);
    await page.screenshot({path:`${out}/home-${width}x${height}.png`});
    records.push({state:'normal', ...await homeGeometry(page)});
  }
  await page.setViewportSize({width:1280,height:800});
  for (const state of ['sparse','dense','long','blocked','running','missing-skin','empty','error','loading']) {
    await homeFixture(page,state);
    await page.screenshot({path:`${out}/${state}-1280x800.png`});
    if (state === 'dense') {await page.locator('main.content').evaluate(e => e.scrollTop = e.scrollHeight); await page.screenshot({path:`${out}/dense-bottom-1280x800.png`});}
    records.push({state, ...await homeGeometry(page)});
  }
  await homeFixture(page,'normal',{background:'simple',theme:'oled'});
  await page.screenshot({path:`${out}/oled-simple-1280x800.png`});
  records.push({state:'oled-simple', ...await homeGeometry(page)});
  await writeFile(`${out}/geometry.json`,JSON.stringify({provenance:'Chromium browser fixture; real Svelte components, synthetic DTOs, DPR 1, 100% CSS scale, reduced motion, local-only requests; no native titlebar',errors,records},null,2));
  assert.deepEqual(errors,[]);
  console.log(`PASS ${phase}: 15 browser captures, zero page errors`);
} finally {await browser.close();}
