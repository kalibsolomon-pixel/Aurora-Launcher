import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {openContentReview,contentGeometry} from './ui3-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({reducedMotion:'reduce'});const errors=[],metrics=[];
page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
await mkdir('docs/ui-3/pilot',{recursive:true});
try {
  for(const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520]]) {
    await page.setViewportSize({width,height});await openContentReview(page);
    const g=await contentGeometry(page);assert.equal(g.contentOverflow||g.documentOverflow,false);metrics.push(g);
    assert.equal(g.columns.split(' ').length,1);
    await page.screenshot({path:`docs/ui-3/pilot/mods-${width}x${height}.png`});
  }
  const search=page.getByRole('searchbox',{name:'Find mods',exact:true});
  await search.fill('FerriteCore');await search.press('Enter');await page.waitForFunction(()=>document.querySelectorAll('.browse-row').length===3);
  assert.match(await page.locator('.result-count').textContent(),/3 of 3/);
  await page.getByRole('button',{name:'Clear search',exact:true}).click();await page.waitForFunction(()=>document.querySelectorAll('.browse-row').length===20);
  assert.equal(await search.evaluate(e=>e===document.activeElement),true);
  await page.getByRole('button',{name:/^Categories:/}).click();await page.getByRole('checkbox',{name:'Adventure',exact:true}).check();await page.keyboard.press('Escape');
  await page.getByRole('combobox',{name:'Sort results'}).selectOption('downloads');
  await page.waitForFunction(()=>window.ui3Requests.filter(r=>r.command==='browse_modrinth').at(-1).request.sort==='downloads');
  const last=await page.evaluate(()=>window.ui3Requests.filter(r=>r.command==='browse_modrinth').at(-1).request);assert.deepEqual(last.categories,['adventure']);
  assert.deepEqual(errors,[]);await writeFile('docs/ui-3/pilot/geometry.json',JSON.stringify(metrics,null,2));
  console.log('PASS Mods pilot: 4 responsive layouts, explicit search/clear/focus, provider facets/sort; zero page errors');
} finally {await browser.close();}
