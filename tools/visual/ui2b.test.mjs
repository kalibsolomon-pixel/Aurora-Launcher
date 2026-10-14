import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {homeFixture,homeGeometry} from './ui2-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({viewport:{width:1280,height:800},reducedMotion:'reduce'});
const results=[],errors=[];
const out='docs/ui-2b';
page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
await mkdir(`${out}/states`,{recursive:true});
const check=async(name,action)=>{await action();results.push(name);console.log('PASS '+name);};
async function mutate(action) {
  await page.evaluate(async action=>{
    const source=await(await fetch('/Review.svelte')).text();
    const {launcher}=await import(source.match(/from "([^"\n]*store.svelte[^"\n]*)"/)[1]);
    if(action==='forge') launcher.launcherState.instances[0].platform={kind:'forge',version:'61.1.0'};
    if(action==='select-vanilla') launcher.launcherState.selectedInstanceId='b'.repeat(32);
    if(action==='signed-out') launcher.accountsState.selectedAccountId=null;
  },action);
}
try {
  await check('Instance identity, honest installed metadata and existing Settings route',async()=>{
    await homeFixture(page);
    assert.equal(await page.locator('.instance-heading h1').textContent(),'Aurora Client');
    assert.match(await page.locator('.instance-metadata').textContent(),/Minecraft 1.21.11.*Fabric 0.19.5.*Aurora 2.1.5 · active/s);
    await mutate('forge');assert.match(await page.locator('.instance-metadata').textContent(),/Forge 61.1.0/);
    await page.getByRole('button',{name:'Instance settings',exact:false}).click();
    await page.locator('.workspace-page').waitFor();assert.equal(await page.getByRole('tab',{name:'Settings',exact:true}).getAttribute('aria-selected'),'true');
  });
  await check('Player stays distinct from instance and hero contains both controls',async()=>{
    await homeFixture(page);
    assert.equal(await page.locator('.home-composition > .player-preview').count(),1);
    assert.equal(await page.locator('.home-launch-column .player-preview').count(),0);
    const g=await homeGeometry(page),hero=g.home;
    for(const rect of [g.instance,g.player]) assert.ok(rect.x>=hero.x&&rect.right<=hero.right&&rect.y>=hero.y&&rect.bottom<=hero.bottom);
    assert.ok(g.widgets.y>hero.bottom);
    const controls=await page.getByRole('group',{name:'Player view'}).boundingBox();assert.ok(controls.y>=g.canvas.bottom);
  });
  await check('Home contrast foundation is scoped and adds no backdrop-filter stack',async()=>{
    await homeFixture(page);
    const material=await page.locator('.home-composition').evaluate(e=>({image:getComputedStyle(e).backgroundImage,filter:getComputedStyle(e).backdropFilter,widgetFilter:getComputedStyle(document.querySelector('.widget')).backdropFilter}));
    assert.match(material.image,/gradient/);assert.equal(material.filter,'none');assert.equal(material.widgetFilter,'none');
    const overlay=await page.locator('.aurora-background').evaluate(e=>getComputedStyle(e,'::after').backgroundImage);
    await page.locator('.rail-link[aria-label="Settings"]').click();await page.getByRole('heading',{name:'Settings',exact:true}).waitFor();
    assert.notEqual(await page.locator('.aurora-background').evaluate(e=>getComputedStyle(e,'::after').backgroundImage),overlay,'Home overlay leaves other pages unchanged');
  });
  await check('Reduced transparency uses opaque theme surfaces and reduced motion keeps still image',async()=>{
    const cdp=await page.context().newCDPSession(page);
    await cdp.send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-transparency',value:'reduce'},{name:'prefers-reduced-motion',value:'reduce'}]});
    await homeFixture(page);
    assert.equal(await page.evaluate(()=>matchMedia('(prefers-reduced-transparency: reduce)').matches),true);
    const panel=await page.locator('.home-page').evaluate(e=>getComputedStyle(e).getPropertyValue('--f-panel').trim());assert.ok(!panel.includes('gradient'));
    assert.ok((await page.locator('.widget').evaluateAll(es=>es.map(e=>getComputedStyle(e).backdropFilter))).every(filter=>filter==='none'));
    assert.equal(await page.locator('.aurora-background video').evaluate(e=>e.paused),true);
    await page.screenshot({path:`${out}/states/reduced-transparency.png`});
    await cdp.send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
  });
  await check('200% text keeps heading, action, selector and error separated at minimum width',async()=>{
    await page.setViewportSize({width:720,height:520});await homeFixture(page,'long');await page.addStyleTag({content:'html{font-size:200% !important}'});
    const g=await homeGeometry(page);assert.equal(g.documentOverflow||g.contentOverflow,false);
    const heading=await page.locator('.instance-heading').boundingBox(),settings=await page.locator('.instance-settings').boundingBox();
    assert.ok(heading.y+heading.height<=g.instance.y&&g.instance.bottom<=settings.y);
    await page.locator('.play-hero').focus();assert.equal(await page.locator('.play-hero').evaluate(e=>getComputedStyle(e).outlineStyle),'solid');
    await page.screenshot({path:`${out}/states/text-200-minimum.png`});
    await homeFixture(page,'blocked');assert.match(await page.locator('.launch-feedback').textContent(),/Synthetic sign-in required/);
    const feedback=await page.locator('.launch-feedback').boundingBox(),hero=await page.locator('.home-composition').boundingBox();
    assert.ok(feedback.width>hero.width*.8,'blockers span the shared hero rather than a nested instance card');
    assert.equal(await page.locator('.launch-feedback').evaluate(e=>getComputedStyle(e).backgroundColor),'rgba(0, 0, 0, 0)');
  });
  await check('Missing skin and signed-out fallback explain the empty preview without a generic model',async()=>{
    await page.setViewportSize({width:1280,height:800});await homeFixture(page,'missing-skin');
    assert.match(await page.locator('.player-placeholder').textContent(),/when available/);
    await mutate('signed-out');assert.match(await page.locator('.player-placeholder').textContent(),/Sign in/);
    assert.equal(await page.locator('.player-preview canvas, .player-controls').count(),0);
  });
  assert.deepEqual(errors,[]);
  await writeFile(`${out}/functional.json`,JSON.stringify({cases:results.length,results,errors,provenance:'Synthetic local browser fixtures; no native or owner data'},null,2));
  console.log(`PASS ${results.length} UI-2B browser cases; zero page errors`);
} finally {await browser.close();}
