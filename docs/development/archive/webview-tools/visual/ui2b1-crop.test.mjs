import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {homeFixture,homeGeometry} from './ui2-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.launch({headless:true});const page=await browser.newPage({reducedMotion:'reduce'});const measurements=[];
try {
 for(const width of [1440,1280,1024,900,801,720]) {
  await page.setViewportSize({width,height:800});await homeFixture(page);const g=await homeGeometry(page);
  assert.equal(g.documentOverflow||g.contentOverflow,false);assert.equal(await page.locator('.player-controls, .reset-view').count(),0);
  if(width>800) {
   const pixels=await page.locator('.player-preview canvas').evaluate(c=>{const d=c.getContext('2d').getImageData(0,0,c.width,c.height).data;let lo=c.height,hi=0;for(let y=0;y<c.height;y++)for(let x=0;x<c.width;x++)if(d[(y*c.width+x)*4+3]){lo=Math.min(lo,y);hi=Math.max(hi,y)}return{lo,hi,height:c.height}});
   assert.ok(g.canvas.y+pixels.lo/pixels.height*g.canvas.height>g.home.y,'head stays inside hero');
   assert.ok(g.canvas.y+pixels.hi/pixels.height*g.canvas.height>g.home.bottom,'actual lower body extends beyond hero');
   assert.ok(g.canvas.x>g.instance.right,'character does not cover Launch');
  } else assert.equal(g.previewDisplay,'none');measurements.push(g);
 }
 await writeFile('docs/ui-2b-1/crop-tests.json',JSON.stringify({cases:measurements.length,measurements},null,2));console.log('PASS 6 responsive crop cases');
} finally {await browser.close();}
