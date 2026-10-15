import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {openSettings} from './ui5-fixture.mjs';
const {chromium}=await import('file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.launch({headless:true}),page=await browser.newPage({viewport:{width:712,height:512},reducedMotion:'reduce'});
const metrics=[];await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());await mkdir('docs/ui-5/browser/measured-minimum',{recursive:true});
try {
 for(const category of ['Appearance','Home','General','Minecraft & Java','Updates','Discord & privacy','Accessibility','About'])for(const scale of [1,2]) {
  await openSettings(page,category);
  if(scale===2)await page.evaluate(()=>{const es=[...document.querySelectorAll('main.content :is(p,span,strong,small,label,button,input,select,h2,h3,h4,legend,output,li)')];const sizes=es.map(e=>parseFloat(getComputedStyle(e).fontSize));es.forEach((e,i)=>e.style.setProperty('font-size',`${sizes[i]*2}px`,'important'));});
  const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth||document.querySelector('main.content').scrollWidth>document.querySelector('main.content').clientWidth+1);
  assert.equal(overflow,false,`${category}, text ${scale}`);metrics.push({category,textScale:scale,viewport:[712,512],overflow});
  await page.screenshot({path:`docs/ui-5/browser/measured-minimum/${category.split(' ')[0].toLowerCase()}-text${scale}.png`});
 }
 await writeFile('docs/ui-5/browser/measured-minimum/results.json',JSON.stringify({provenance:'Browser fixtures at the separately measured native minimum; not native captures',metrics},null,2));console.log('PASS 16 layouts at measured 712x512 minimum, 100%/200% text');
}finally{await browser.close();}
