import assert from 'node:assert/strict';
import { seed } from './ui-corrections-fixture.mjs';
const { chromium } = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = process.env.UI_CDP_URL ? await chromium.connectOverCDP(process.env.UI_CDP_URL) : await chromium.launch({headless:true});
const page = process.env.UI_CDP_URL ? browser.contexts()[0].pages()[0] : await browser.newPage({viewport:{width:1600,height:1000}});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
try {
  await seed(page,'installed');
  assert.equal(await page.locator('.mod-row').count(),100);
  assert.equal(await page.locator('.mod-warning').count(),0);
  const single=page.getByRole('button',{name:'Metadata warnings for More Culling',exact:true});
  const row=page.locator('.mod-row').filter({has:single});
  const placement=await row.evaluate(e=>{const v=e.querySelector('.mod-version').getBoundingClientRect(),b=e.querySelector('.tooltip-trigger').getBoundingClientRect();return {gap:b.left-v.right,dy:Math.abs((b.top+b.bottom-v.top-v.bottom)/2)};});
  assert.ok(placement.gap>=0&&placement.gap<=4&&placement.dy<2);
  const heights=await page.locator('.mod-row').evaluateAll(es=>es.slice(0,8).map(e=>e.getBoundingClientRect().height));
  assert.ok(Math.max(...heights)-Math.min(...heights)<=1);
  await single.focus();
  const tooltip=page.getByRole('tooltip');await tooltip.waitFor({state:'visible'});
  assert.equal(await tooltip.locator('li').count(),1);
  assert.match(await tooltip.innerText(),/Requires Sodium/);
  assert.equal(await single.getAttribute('aria-describedby'),await tooltip.getAttribute('id'));
  await page.keyboard.press('Escape');await tooltip.waitFor({state:'hidden'});assert.ok(await single.evaluate(e=>document.activeElement===e));
  await single.click();await tooltip.waitFor({state:'visible'});await single.click();await tooltip.waitFor({state:'hidden'});
  const multiple=page.getByRole('button',{name:'Metadata warnings for Reese’s Sodium Options',exact:true});
  await multiple.hover();await tooltip.waitFor({state:'visible'});assert.equal(await tooltip.locator('li').count(),2);assert.match(await tooltip.innerText(),/long metadata explanation/);
  let box=await tooltip.boundingBox();const viewport=await page.evaluate(()=>({width:innerWidth,height:innerHeight}));assert.ok(box.x>=0&&box.y>=0&&box.x+box.width<=viewport.width&&box.y+box.height<=viewport.height);
  await page.keyboard.press('Escape');
  await page.getByRole('searchbox',{name:'Search mods'}).fill('Reese');assert.equal(await page.locator('.mod-row').count(),1);
  await multiple.focus();await tooltip.waitFor({state:'visible'});assert.equal(await tooltip.locator('li').count(),2);await page.keyboard.press('Escape');
  await page.getByRole('combobox',{name:'Sort mods'}).selectOption('warnings');
  assert.equal(await page.getByRole('button',{name:'Metadata warnings for Reese’s Sodium Options',exact:true}).count(),1);
  await page.getByRole('searchbox',{name:'Search mods'}).fill('');
  const before=await page.evaluate(()=>window.uiRequests.resolve_project_artwork??0);
  await page.locator('main.content').evaluate(e=>e.scrollTop=2200);await page.waitForTimeout(200);
  const scrollBefore=await page.locator('main.content').evaluate(e=>e.scrollTop);
  await page.evaluate(()=>document.documentElement.style.setProperty('--color-accent','#5bd0e0'));
  assert.equal(await page.locator('main.content').evaluate(e=>e.scrollTop),scrollBefore);
  await page.locator('main.content').evaluate(e=>e.scrollTop=0);
  assert.equal(await page.evaluate(()=>window.uiRequests.resolve_project_artwork??0),before);
  await page.getByRole('switch',{name:'Disable More Culling',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'required by another enabled mod'}).waitFor();
  assert.equal(await page.getByRole('switch',{name:'Disable More Culling',exact:true}).getAttribute('aria-checked'),'true');
  assert.equal(await page.getByRole('tooltip').count(),0);
  console.log('PASS 100 installed rows, single/grouped/long warnings, placement/density, keyboard/click/hover/Escape, filtering/sorting and artwork/scroll preservation');
  for(const kind of ['mod','resourcePack','shaderPack','modpack']){
    await seed(page,kind);assert.equal(await page.locator('.browse-row').count(),100);
    assert.ok(await page.locator('.browse-row').evaluateAll(es=>es.every(e=>getComputedStyle(e).backdropFilter==='none')));
    const requests=await page.evaluate(()=>window.uiRequests.resolve_project_artwork??0);
    await page.locator('main.content').evaluate(e=>e.scrollTop=3000);await page.waitForTimeout(100);await page.locator('main.content').evaluate(e=>e.scrollTop=0);
    assert.equal(await page.evaluate(()=>window.uiRequests.resolve_project_artwork??0),requests);
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  }
  assert.deepEqual(errors,[]);console.log('PASS four 100-result browser views, no list blur, artwork reuse, no overflow or page errors');
  await seed(page,'installed');
  const backing=await page.locator('.workspace-page').evaluate(e=>({background:getComputedStyle(e).background,blur:getComputedStyle(e).backdropFilter}));
  assert.match(backing.background,/0\.82/);assert.equal(backing.blur,'none');
  // Conservative white-underlay bound: 82% dark fill plus the lightest 5%
  // gradient endpoint. The video and shell scrim can only darken this bound.
  const background=[8,17,27].map((v,i)=>(v*.82+255*.18)*.95+[184,224,235][i]*.05);
  const luminance=rgb=>rgb.map(v=>v/255).reduce((sum,v,i)=>sum+[.2126,.7152,.0722][i]*(v<=.04045?v/12.92:((v+.055)/1.055)**2.4),0);
  const palette=await page.locator('.workspace-page').evaluate(e=>['--color-text','--color-text-secondary','--color-text-muted'].map(name=>getComputedStyle(e).getPropertyValue(name).trim()));
  for(const hex of palette){const rgb=hex.match(/[a-f\d]{2}/gi).map(v=>parseInt(v,16));assert.ok((luminance(rgb)+.05)/(luminance(background)+.05)>=4.5,hex);}
  const selected=await page.locator('.workspace-tab-active').evaluate(e=>({shadow:getComputedStyle(e).boxShadow,background:getComputedStyle(e).backgroundColor}));assert.match(selected.shadow,/91, 208, 224/);assert.notEqual(selected.background,'rgba(0, 0, 0, 0)');
  for(const [width,height] of [[900,930],[720,520],[1600,1000]]){
    if(process.env.UI_CDP_URL)await page.evaluate(async({width,height,root})=>{const {getCurrentWindow,LogicalSize}=await import('/@fs/'+root+'/node_modules/@tauri-apps/api/window.js');await getCurrentWindow().setSize(new LogicalSize(width,height));},{width,height,root:process.cwd().replaceAll('\\','/')});
    else await page.setViewportSize({width,height});
    await page.waitForTimeout(100);
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
    await page.locator('main.content').evaluate(e=>e.scrollTop=0);
    await page.locator('.workspace-tab-active').focus();
    await multiple.focus();await tooltip.waitFor({state:'visible'});
    box=await tooltip.boundingBox();const size=await page.evaluate(()=>({width:innerWidth,height:innerHeight}));assert.ok(box.x>=0&&box.y>=0&&box.x+box.width<=size.width&&box.y+box.height<=size.height);
    await page.keyboard.press('Escape');
  }
  console.log('PASS worst-case bright backing contrast, turquoise selection, 720/900/1600px layouts and tooltip edges');
  const media=await page.context().newCDPSession(page);
  await media.send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-transparency',value:'reduce'}]});
  assert.ok(await page.locator('.workspace-page').evaluate(e=>{const s=getComputedStyle(e);return s.backgroundImage==='none'&&s.backdropFilter==='none'&&s.backgroundColor.startsWith('rgb(');}));
  await media.send('Emulation.setEmulatedMedia',{features:[]});await media.detach();
  console.log('PASS reduced transparency uses an opaque unfiltered workspace backing');
  await multiple.click();await tooltip.waitFor({state:'visible'});
  await page.getByRole('tab',{name:'Settings',exact:true}).click();
  await tooltip.waitFor({state:'hidden'});await page.waitForTimeout(100);
  assert.deepEqual(errors,[]);
  console.log('PASS navigating with a pinned warning unmounts without a delayed toggle error');
  await page.goto('http://127.0.0.1:1422/?page=instances&background=borealis');
  await page.getByRole('button',{name:'Browse Modpacks',exact:true}).click();
  await page.locator('.browse-row').first().waitFor();
  assert.ok(await page.locator('.browse-row').first().evaluate(e=>{for(let p=e;p;p=p.parentElement)if(getComputedStyle(p).backdropFilter!=='none')return false;return true;}));
  assert.deepEqual(errors,[]);
  console.log('PASS standalone modpack browser also has no filtered scrolling ancestor');
} finally {await browser.close();}
