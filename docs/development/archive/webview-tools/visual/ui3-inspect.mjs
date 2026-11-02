import {openContentReview} from './ui3-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.launch({headless:true});
try {
  const page=await browser.newPage({viewport:{width:1280,height:800},reducedMotion:'reduce'});
  await openContentReview(page,'installed-packs');
  console.log(await page.locator('.pack-description').evaluateAll(es=>es.map(e=>{const s=getComputedStyle(e);return {text:e.textContent?.length,height:e.getBoundingClientRect().height,font:s.fontSize,lineHeight:s.lineHeight,display:s.display,webkitLineClamp:s.webkitLineClamp,lineClamp:s.lineClamp,orient:s.webkitBoxOrient,overflow:s.overflow};})));
}finally{await browser.close();}
