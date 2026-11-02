import {mkdir, writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {openReview, geometry} from './ui1-fixture.mjs';
const phase = process.argv[2];
assert.ok(['before', 'after'].includes(phase), 'specify before or after');
const {chromium} = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({headless: true});
const page = await browser.newPage({deviceScaleFactor: 1, reducedMotion: 'reduce'});
const errors = [], records = [];
page.on('pageerror', e => errors.push(e.message));
// Keep the fixture completely local, including artwork fallbacks.
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/, route => route.abort());
const out = `docs/ui-1/${phase}`;
await mkdir(out, {recursive: true});
try {
  for (const [width, height] of [[1440, 900], [1280, 800], [1024, 768], [720, 520]]) {
    await page.setViewportSize({width, height});
    for (const surface of ['home', 'mods']) {
      await openReview(page, surface);
      await page.screenshot({path: `${out}/${surface}-${width}x${height}.png`});
      records.push({surface, ...await geometry(page)});
    }
  }
  await page.setViewportSize({width: 1280, height: 800});
  for (const surface of ['settings', 'instances', 'cosmetics']) {
    await openReview(page, surface);
    await page.screenshot({path: `${out}/${surface}-1280x800.png`});
    records.push({surface, ...await geometry(page)});
  }
  await writeFile(`${out}/geometry.json`, JSON.stringify({provenance: 'Headless Chromium browser preview; actual Svelte components, synthetic DTOs; 100% CSS scaling, DPR 1; reduced motion, bundled Borealis still; browser outer chrome excluded', errors, records}, null, 2));
  assert.deepEqual(errors, []);
  console.log(`PASS ${phase}: 11 browser screenshots; 8 pilot layouts, 3 shared-shell regression views; zero page errors`);
} finally {await browser.close();}
