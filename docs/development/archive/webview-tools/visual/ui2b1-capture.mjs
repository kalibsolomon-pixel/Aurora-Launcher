import {mkdir, writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {homeFixture, homeGeometry} from './ui2-fixture.mjs';
const phase = process.argv[2];
assert.ok(['before', 'hero', 'background', 'after'].includes(phase));
const {chromium} = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({headless:true});
const page = await browser.newPage({deviceScaleFactor:1, reducedMotion:'reduce'});
const errors = [], records = [];
page.on('pageerror', e => errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/, r => r.abort());
const out = `docs/ui-2b-1/${phase}`;
await mkdir(out, {recursive:true});
async function capture(name, state = 'normal', options = {}) {
  await homeFixture(page,state,options);
  await page.screenshot({path:`${out}/${name}.png`});
  records.push({state,name,...await homeGeometry(page),...await page.evaluate(()=>{
    const c=document.querySelector('main.content'),s=getComputedStyle(c);
    return {scrollExtent:c.scrollHeight-c.clientHeight,contentMargins:[s.paddingLeft,s.paddingRight],firstWidgetY:document.querySelector('[data-home-widget]')?.getBoundingClientRect().y};
  })});
}
try {
  for (const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520]]) {
    await page.setViewportSize({width,height});await capture(`home-${width}x${height}`);
  }
  if(['before','after'].includes(phase)) {
    await page.setViewportSize({width:1280,height:800});
    for (const state of ['sparse','dense','long','blocked','running','missing-skin','empty','error','loading']) await capture(`${state}-1280x800`,state);
    await capture('oled-simple-1280x800','normal',{background:'simple',theme:'oled'});
    await homeFixture(page);await page.evaluate(()=>{document.documentElement.dataset.accent='cyan';document.documentElement.style.setProperty('--color-accent-strong','#5bd0e0');document.documentElement.style.setProperty('--color-accent-contrast','#101116');});
    await page.screenshot({path:`${out}/cyan-1280x800.png`});
  }
  await writeFile(`${out}/geometry.json`,JSON.stringify({provenance:'Chromium browser fixtures; real components, synthetic DTOs, DPR 1, 100% CSS scale, reduced motion, blocked external requests; no native titlebar',errors,records},null,2));
  assert.deepEqual(errors,[]);console.log(`PASS ${phase}: ${records.length} measured captures; zero page errors`);
} finally {await browser.close();}
