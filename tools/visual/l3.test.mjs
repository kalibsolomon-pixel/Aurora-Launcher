// Deterministic layout and interaction regression for the actual L3 components.
// Run the standalone review server first. All accounts here are synthetic.
import assert from 'node:assert/strict';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({reducedMotion:'reduce'}),errors=[];
page.on('pageerror',e=>errors.push(e.message));
try {
  for(const [width,height] of [[1600,1030],[1120,760],[860,800],[720,520]]) {
    await page.setViewportSize({width,height});
    await page.goto('http://127.0.0.1:1422/?l3&page=cosmetics');
    await page.getByRole('button',{name:'Preview saved skin Aurora Frost',exact:true}).click();
    await page.waitForFunction(()=>document.querySelector('.preview-stage canvas'));
    const geometry=await page.evaluate(()=>{
      const rect=s=>document.querySelector(s).getBoundingClientRect();
      const stage=rect('.preview-stage'),canvas=rect('.preview-stage canvas'),controls=rect('.view-controls'),actions=rect('.detail-actions'),library=rect('.library-panel'),grid=rect('.cosmetic-grid');
      const content=document.querySelector('main.content');
      return {contained:canvas.top>=stage.top-1&&canvas.bottom<=stage.bottom+1&&canvas.left>=stage.left-1&&canvas.right<=stage.right+1,controlsBelow:controls.top>=stage.bottom,actionsBelow:actions.top>=controls.bottom,gridContained:grid.width<=library.width,noHorizontalOverflow:content.scrollWidth<=content.clientWidth+1,onlyOneAccountSelector:document.querySelectorAll('.account-chip').length===1&&document.querySelectorAll('.context-account').length===0};
    });
    assert.ok(Object.values(geometry).every(Boolean),JSON.stringify({width,height,geometry}));
    await page.getByRole('button',{name:'Back',exact:true}).click();assert.equal(await page.getByRole('slider').getAttribute('aria-valuenow'),'180');
    await page.getByRole('button',{name:'Front',exact:true}).click();assert.equal(await page.getByRole('slider').getAttribute('aria-valuenow'),'0');
    await page.getByRole('slider').focus();await page.keyboard.press('ArrowRight');assert.notEqual(await page.getByRole('slider').getAttribute('aria-valuenow'),'0');
    await page.getByRole('button',{name:'Import skin',exact:true}).click();await page.getByRole('dialog').waitFor();await page.keyboard.press('Escape');assert.equal(await page.getByRole('dialog').isVisible(),false);assert.equal(await page.getByRole('button',{name:'Import skin',exact:true}).evaluate(e=>e===document.activeElement),true);
    console.log(`PASS layout ${width}x${height}: contained canvas, separate controls/actions, no horizontal overflow, one account selector, keyboard rotation and dialog focus return`);
  }
  await page.setViewportSize({width:1600,height:1030});await page.goto('http://127.0.0.1:1422/?l3&page=cosmetics&cosmetic-tab=capes');
  await page.getByRole('button',{name:'Preview owned cape Foundry',exact:true}).click();assert.ok(await page.getByRole('button',{name:'Equip Foundry',exact:true}).isEnabled());assert.equal(await page.locator('.cape-card .card-meta').filter({hasText:'Active'}).count(),1);
  await page.getByRole('button',{name:'Equip Foundry',exact:true}).click();await page.getByText('Cape choice confirmed by Minecraft.',{exact:true}).waitFor();assert.ok(await page.getByRole('button',{name:'Currently active',exact:true}).isDisabled());
  await page.getByRole('button',{name:'Accounts — signed in as AuroraPlayer',exact:true}).click();await page.getByRole('button',{name:'Use account',exact:true}).click();await page.getByRole('button',{name:'Accounts — signed in as SecondPlayer',exact:true}).waitFor({state:'attached'});await page.keyboard.press('Escape');await page.getByText('Owned by SecondPlayer',{exact:true}).waitFor();assert.equal(await page.locator('.cape-card').count(),0);
  await page.getByRole('button',{name:'Home',exact:true}).click();await page.getByRole('button',{name:/Open Cape Library/}).click();assert.equal(await page.getByRole('tab',{name:/Capes/}).getAttribute('aria-selected'),'true');
  await page.getByRole('button',{name:'Skins & Capes',exact:true}).click();assert.equal(await page.getByRole('tab',{name:/Capes/}).getAttribute('aria-selected'),'true');
  assert.deepEqual(errors,[]);console.log('PASS fixture cape confirmation, real account-selector interaction, account isolation, widget navigation, restored tab and zero page errors');
} finally {await browser.close();}
