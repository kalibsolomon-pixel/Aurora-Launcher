import {mkdir, writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {openContentReview, contentGeometry} from './ui3-fixture.mjs';
const {chromium} = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({headless:true});
const page = await browser.newPage({reducedMotion:'reduce'});
const side = process.argv[2]; assert.ok(['before','after'].includes(side));
const out = `docs/ui-3-1/${side}`; await mkdir(out,{recursive:true});
const errors=[], measurements={}; page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
try {
  for(const surface of ['mods','modpacks','packs','installed-mods','installed-packs']) {
    for(const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520],[706,512]]) {
      await page.setViewportSize({width,height}); await openContentReview(page,surface);
      measurements[`${surface}-${width}x${height}`]={...await contentGeometry(page), ...await page.evaluate(()=>{
        const row=document.querySelector('.browse-row'); if(!row)return {};
        const styles=e=>{const s=getComputedStyle(e);return {height:e.getBoundingClientRect().height,columns:s.gridTemplateColumns,padding:s.padding,gap:s.gap,border:s.borderBottom,radius:s.borderRadius,font:s.fontSize,lineHeight:s.lineHeight,clamp:s.webkitLineClamp};};
        return {rowStyle:styles(row),title:styles(row.querySelector('h4')),description:styles(row.querySelector('.browse-copy p')),artwork:styles(row.querySelector('.browse-glyph')),imageSize:[row.querySelector('img').width,row.querySelector('img').height],actionSize:[...row.querySelectorAll('button')].map(b=>[b.getBoundingClientRect().width,b.getBoundingClientRect().height])};
      })};
      assert.equal(measurements[`${surface}-${width}x${height}`].documentOverflow||measurements[`${surface}-${width}x${height}`].contentOverflow,false);
      if(!surface.startsWith('installed'))await page.screenshot({path:`${out}/${surface}-${width}x${height}.png`});
      if(side==='after' && surface==='modpacks' && width<=1024) {
        await page.locator('.browse-row').first().scrollIntoViewIfNeeded();
        await page.screenshot({path:`${out}/modpacks-results-${width}x${height}.png`});
      }
    }
  }
  await writeFile(`${out}/geometry.json`,JSON.stringify({provenance:'Actual components, deterministic browser-only fixtures, reduced motion, DPR 1; no native forwarding or owner data.',measurements,pageErrors:errors},null,2));
  assert.deepEqual(errors,[]);console.log(`PASS ${side}: 15 matched screenshots${side==='after'?' + 3 scrolled result captures':''}, 25 measured layouts, no page errors.`);
} finally {await browser.close();}
