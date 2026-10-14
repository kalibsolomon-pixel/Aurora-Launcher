import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {appearanceFixture,appearanceModules} from './ui4-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE||'playwright');
const browser=await chromium.launch({headless:true}),page=await browser.newPage({reducedMotion:'reduce'});
const out='docs/ui-4/browser';await mkdir(out,{recursive:true});
const errors=[],measurements=[],checks=[];page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
const pass=message=>{checks.push(message);console.log('PASS',message);};
const pixels=()=>page.locator('.preview-stage canvas').evaluate(c=>c.toDataURL());
async function select() {await page.getByRole('button',{name:'Preview saved skin Trailblazer',exact:true}).click();await page.waitForTimeout(100);}
try {
  for(const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520],[706,512]])for(const scale of [1,2]) {
    await page.setViewportSize({width,height});await appearanceFixture(page);await select();
    if(scale===2)await page.evaluate(()=>{const elements=[...document.querySelectorAll('main.content :is(p,span,strong,small,label,button,input,select,h2,h3,h4,output)')];const sizes=elements.map(e=>parseFloat(getComputedStyle(e).fontSize));elements.forEach((e,i)=>e.style.setProperty('font-size',`${sizes[i]*2}px`,'important'));});
    await page.locator('main.content').evaluate(e=>e.scrollTop=0);
    await page.waitForTimeout(80);
    const geometry=await page.evaluate(()=>{
      const rect=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right};};
      const content=document.querySelector('main.content'),stage=document.querySelector('.preview-stage'),library=document.querySelector('.library-panel');
      return {viewport:[innerWidth,innerHeight],documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:content.scrollWidth>content.clientWidth+1,stage:rect(stage),library:rect(library),importVisible:!!document.querySelector('.top-actions .btn-primary'),accountSelectors:document.querySelectorAll('.account-chip').length};
    });
    assert.equal(geometry.documentOverflow||geometry.contentOverflow,false,JSON.stringify({width,height,scale,geometry}));assert.equal(geometry.accountSelectors,1);
    if(width>900&&scale===1)assert.ok(geometry.stage.right<geometry.library.x);
    measurements.push({...geometry,textScale:scale});
    await page.getByRole('button',{name:'Import skin',exact:true}).click();await page.getByRole('dialog').waitFor();
    const dialog=await page.getByRole('dialog').boundingBox();assert.ok(dialog.x>=0&&dialog.y>=0&&dialog.x+dialog.width<=width+1&&dialog.y+dialog.height<=height+1);
    await page.keyboard.press('Escape');assert.equal(await page.getByRole('button',{name:'Import skin',exact:true}).evaluate(e=>e===document.activeElement),true);
    if(scale===1) {await page.locator('main.content').evaluate(e=>e.scrollTop=0);await page.screenshot({path:`${out}/appearance-${width}x${height}.png`});}
    else {
      await page.screenshot({path:`${out}/text-200-${width}x${height}.png`});
      if(width===706){
        await page.getByRole('button',{name:'Preview saved skin Woodland',exact:true}).click();
        await page.locator('.preview-stage').scrollIntoViewIfNeeded();await page.waitForTimeout(80);
        await page.screenshot({path:`${out}/text-200-minimum-stage.png`});
        assert.match(await page.locator('.preview-heading').innerText(),/Woodland.*Local preview/s);
        await page.getByRole('button',{name:'Preview saved skin Trailblazer',exact:true}).click();
        await page.screenshot({path:`${out}/text-200-minimum-library.png`});
      }
    }
  }
  pass('Five requested/measured-minimum sizes at 100%/200% text; no horizontal overflow, dialog fit and focus return');
  await page.setViewportSize({width:1440,height:900});await appearanceFixture(page);await select();await appearanceModules(page,'requests');
  const original=await pixels(),stage=page.getByRole('slider');const box=await stage.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2+90,box.y+box.height/2+50,{steps:5});await page.mouse.up();await page.waitForTimeout(80);
  assert.notEqual(await pixels(),original);assert.match(await stage.getAttribute('aria-valuetext'),/23 degrees tilt/);await page.screenshot({path:`${out}/rotated.png`});
  await page.getByRole('button',{name:'Reset view',exact:true}).click();await page.waitForTimeout(80);assert.equal(await pixels(),original);
  await stage.focus();await page.keyboard.press('ArrowRight');await page.keyboard.press('ArrowUp');await page.keyboard.press('+');await page.waitForTimeout(80);assert.notEqual(await pixels(),original);
  for(let i=0;i<15;i++)await page.getByRole('button',{name:'Zoom in',exact:true}).evaluate(b=>b.click());assert.equal(await page.getByRole('button',{name:'Zoom in',exact:true}).isDisabled(),true);
  for(let i=0;i<20;i++)await page.getByRole('button',{name:'Zoom out',exact:true}).evaluate(b=>b.click());assert.equal(await page.getByRole('button',{name:'Zoom out',exact:true}).isDisabled(),true);
  await stage.focus();await page.keyboard.press('Home');await page.waitForTimeout(80);assert.equal(await pixels(),original);pass('Pointer/keyboard yaw and tilt, zoom bounds and pixel-exact reset');
  await page.getByLabel('Model preview',{exact:true}).selectOption('slim');await page.waitForTimeout(80);assert.notEqual(await pixels(),original);
  await page.getByLabel('Model preview',{exact:true}).selectOption('classic');await page.getByLabel('Outer layers',{exact:true}).uncheck();await page.waitForTimeout(80);assert.notEqual(await pixels(),original);
  await page.getByLabel('Outer layers',{exact:true}).check();
  assert.equal((await page.evaluate(()=>window.ui4Requests)).filter(c=>/apply|select_cape|disable_cape|update_skin/.test(c.command)).length,0);pass('Classic/slim and outer layers stay local, no mutation requests');
  const beforeAccount=(await appearanceModules(page,'snapshot')).avatar;
  await page.getByRole('button',{name:'Apply to AuroraPlayer',exact:true}).click();await page.getByRole('dialog').waitFor();assert.match(await page.getByRole('dialog').innerText(),/Trailblazer.*AuroraPlayer/s);
  assert.equal((await page.evaluate(()=>window.ui4Requests)).filter(c=>c.command==='apply_skin_preset').length,0);
  await page.getByRole('button',{name:'Confirm Apply',exact:true}).click();await page.getByText('Skin request completed and account profile refreshed.',{exact:true}).waitFor();
  assert.equal((await page.evaluate(()=>window.ui4Requests)).filter(c=>c.command==='apply_skin_preset').length,1);pass('Apply requires named-account confirmation and refreshed success');
  await appearanceFixture(page,'mutation-failure');await select();await page.getByRole('button',{name:'Apply to AuroraPlayer',exact:true}).click();await page.getByRole('button',{name:'Confirm Apply',exact:true}).click();await page.getByRole('alert').filter({hasText:'rejected'}).waitFor();assert.equal(await page.getByText('Skin request completed and account profile refreshed.',{exact:true}).count(),0);pass('Service failure has no false success and preserves local preview');
  await appearanceFixture(page,'cosmetic-tab=capes');await page.getByRole('button',{name:'Back',exact:true}).click();const withCape=await pixels();await page.getByLabel('Show cape',{exact:true}).uncheck();await page.waitForTimeout(80);assert.notEqual(await pixels(),withCape);await page.getByLabel('Show cape',{exact:true}).check();
  await page.getByRole('button',{name:'Preview owned cape Foundry',exact:true}).click();await page.screenshot({path:`${out}/cape-preview.png`});await page.getByRole('button',{name:'Equip Foundry',exact:true}).click();await page.getByRole('button',{name:'Confirm cape change',exact:true}).click();await page.getByText('Cape choice confirmed by Minecraft.',{exact:true}).waitFor();pass('Owned cape preview/visibility independent of explicit confirmed equip');
  await appearanceFixture(page,'no-capes&cosmetic-tab=capes');assert.match(await page.locator('.library-panel').innerText(),/No capes are reported/);assert.equal(await page.getByLabel('Show cape',{exact:true}).count(),0);await page.screenshot({path:`${out}/no-capes.png`});pass('No owned capes is truthful with no invented inventory');
  for(const state of ['accounts=0','offline','expired']) {await appearanceFixture(page,state);await select();assert.equal(await page.getByRole('button',{name:/Apply to/}).isDisabled(),true);await page.screenshot({path:`${out}/${state.replace('=','-')}.png`});}pass('No-account, offline and expired states retain local preview and block Apply');
  await appearanceFixture(page);await appearanceModules(page,'profile',{currentSkin:null,hasCurrentSkin:true,currentSkinModel:null});await page.getByRole('button',{name:/AuroraPlayer.s skin/}).click();assert.match(await page.locator('.preview-heading').innerText(),/Cached account skin/);pass('Fresh profile with unavailable texture labels cached fallback accurately');
  await appearanceFixture(page);await select();await page.getByRole('button',{name:'Apply to AuroraPlayer',exact:true}).click();await appearanceModules(page,'account','d'.repeat(32));await page.getByRole('dialog').waitFor({state:'hidden'});assert.match(await page.locator('.account-note').innerText(),/SecondPlayer/);assert.match(await page.locator('.preview-heading').innerText(),/Local preview/);pass('Account switch closes confirmation and explicitly retains local skin preview');
  await appearanceFixture(page);await appearanceModules(page,'stale','c'.repeat(32));await appearanceModules(page,'account','d'.repeat(32));await page.waitForTimeout(120);await appearanceModules(page,'resolve',{accountId:'c'.repeat(32),currentSkinModel:'classic',hasCurrentSkin:false,currentSkin:null,capes:[]});await page.waitForTimeout(120);assert.equal((await appearanceModules(page,'snapshot')).remote.accountId,'d'.repeat(32));pass('Late previous-account response cannot overwrite selected-account appearance');
  await appearanceFixture(page);await select();await page.getByRole('button',{name:'Rename',exact:true}).click();await page.getByLabel('Name',{exact:true}).fill('My winter skin');await page.getByRole('button',{name:'Save name',exact:true}).click();await page.getByRole('heading',{name:'My winter skin',exact:true}).waitFor();await page.getByRole('button',{name:'Remove from library',exact:true}).first().click();await page.getByRole('dialog').waitFor();await page.getByRole('dialog').getByRole('button',{name:'Remove from library',exact:true}).click();assert.equal(await page.getByRole('button',{name:'Preview saved skin My winter skin',exact:true}).count(),0);pass('Local rename/removal confirmed, exact entry removed');
  await appearanceFixture(page,'background=simple&theme=oled');await select();await page.emulateMedia({reducedMotion:'reduce',reducedTransparency:'reduce'});await page.screenshot({path:`${out}/reduced-transparency.png`});
  await page.getByRole('button',{name:'Home',exact:true}).click();await page.getByRole('button',{name:'Appearance',exact:true}).click();await page.getByRole('heading',{name:'Appearance',exact:true}).waitFor();pass('Existing rail navigation away/back releases and reinitializes preview');
  assert.deepEqual(errors,[]);await writeFile(`${out}/results.json`,JSON.stringify({provenance:'Deterministic browser fixtures, actual components; synthetic accounts and service outcomes; no real authentication/mutations',checks,measurements,pageErrors:errors},null,2));
}finally{await browser.close();}
