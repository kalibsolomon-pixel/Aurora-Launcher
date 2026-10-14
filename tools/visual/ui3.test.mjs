import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {openContentReview,contentGeometry} from './ui3-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({viewport:{width:1280,height:800},reducedMotion:'reduce'});
const errors=[],results=[];page.on('pageerror',e=>errors.push(e.message));
let externalRequests=0;await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>{externalRequests++;return r.abort();});
await mkdir('docs/ui-3/states',{recursive:true});
const check=async(name,fn)=>{await fn();results.push(name);console.log('PASS '+name);};
const requests=command=>page.evaluate(c=>window.ui3Requests.filter(r=>r.command===c).map(r=>r.request),command);
const search=()=>page.getByRole('searchbox');
const submit=async term=>{await search().fill(term);await search().press('Enter');};
const rows=async n=>page.waitForFunction(n=>document.querySelectorAll('.browse-row').length===n,n);
async function contained() {
  const g=await contentGeometry(page);assert.equal(g.documentOverflow||g.contentOverflow,false,JSON.stringify(g));
  assert.equal(await page.locator('.browse-row,.mod-row,.pack-row').evaluateAll(es=>es.some(e=>e.scrollWidth>e.clientWidth+1)),false,'row overflow');
}
try {
  await check('Search stays explicit; clear restores focus and respects categories/sort; pagination appends',async()=>{
    await openContentReview(page);
    const initial=(await requests('browse_modrinth')).length;
    await search().fill('FerriteCore');await page.waitForTimeout(150);
    assert.equal((await requests('browse_modrinth')).length,initial);
    assert.match(await page.locator('.result-count').textContent(),/^20 of 24 mods$/);
    await search().press('Enter');await rows(3);assert.match(await page.locator('.result-count').textContent(),/3 of 3/);
    await page.getByRole('button',{name:'Clear search',exact:true}).click();await rows(20);
    assert.equal(await search().evaluate(e=>e===document.activeElement),true);
    await page.getByRole('button',{name:'Load more',exact:true}).click();await rows(24);
    assert.equal((await requests('browse_modrinth')).at(-1).offset,20);
  });
  await check('Categories retain search/sort, native provider values and keyboard focus',async()=>{
    await submit('Sodium');await rows(3);
    await page.getByRole('combobox',{name:'Sort results'}).selectOption('downloads');
    const trigger=page.getByRole('button',{name:/^Categories:/});await trigger.click();
    await page.getByRole('checkbox',{name:'Adventure',exact:true}).check();await page.keyboard.press('Escape');
    await page.waitForFunction(()=>window.ui3Requests.filter(r=>r.command==='browse_modrinth').at(-1).request.categories.length===1);
    const r=(await requests('browse_modrinth')).at(-1);assert.deepEqual({kind:r.contentType,instance:r.instanceId,search:r.search,categories:r.categories,sort:r.sort},{kind:'mod',instance:'a'.repeat(32),search:'Sodium',categories:['adventure'],sort:'downloads'});
    assert.equal(await trigger.evaluate(e=>e===document.activeElement),true);
    await page.getByRole('button',{name:'Remove Adventure filter',exact:true}).click();
    assert.equal((await requests('browse_modrinth')).at(-1).search,'Sodium');
  });
  await check('Installed and dependency states; pending install excludes duplicate requests and announces completion',async()=>{
    await openContentReview(page);
    assert.equal(await page.getByRole('button',{name:'Sodium is installed',exact:true}).isDisabled(),true);
    await page.getByRole('button',{name:'Keep Lithium installed directly',exact:true}).waitFor();
    await page.evaluate(()=>window.ui3InstallMode='pending');
    const action=page.getByRole('button',{name:'Install newest eligible version of Mod Menu',exact:true});
    await action.evaluate(e=>{e.click();e.click();e.click();});
    await page.getByRole('status').filter({hasText:'Installing Mod Menu into Aurora Client'}).waitFor();
    assert.equal((await requests('quick_install_modrinth')).length,1);
    assert.equal(await page.getByRole('button',{name:'Installing Mod Menu',exact:true}).isDisabled(),true);
    await page.evaluate(()=>{window.ui3InstallMode='success';window.ui3Pending();});
    await page.getByRole('button',{name:'Mod Menu is installed',exact:true}).waitFor();
    await page.getByRole('status').filter({hasText:'Mod Menu installed.'}).waitFor();
    await page.screenshot({path:'docs/ui-3/states/installed-state.png'});
  });
  await check('Collision/verification failures preserve explicit blocked or retryable feedback',async()=>{
    await openContentReview(page);await page.evaluate(()=>window.ui3InstallMode='blocked');
    await page.getByRole('button',{name:'Install newest eligible version of Mod Menu',exact:true}).click();
    assert.equal(await page.getByRole('button',{name:'Mod Menu installation blocked; review Details',exact:true}).isDisabled(),true);
    await page.getByRole('button',{name:'Details for Mod Menu',exact:true}).click();
    await page.locator('.project').getByText('Fixture: an existing local file conflicts with this project.',{exact:true}).waitFor();
    await openContentReview(page);await page.evaluate(()=>window.ui3InstallMode='error');
    await page.getByRole('button',{name:'Install newest eligible version of Mod Menu',exact:true}).click();
    await page.getByRole('status').filter({hasText:'The download failed verification. Nothing was installed.'}).waitFor();
    assert.equal(await page.getByRole('button',{name:'Install newest eligible version of Mod Menu',exact:true}).isEnabled(),true);
  });
  await check('Modpack artwork grid and exact version/optional-file review remain explicit',async()=>{
    await openContentReview(page,'modpacks');
    assert.equal(await page.locator('.install-action').count(),0);
    const card=page.locator('.browse-row').first();
    const art=await card.locator('.browse-glyph').boundingBox();assert.ok(Math.abs(art.width/art.height-16/9)<.01);
    assert.equal(await card.locator('img').evaluate(e=>getComputedStyle(e).objectFit),'contain');
    await page.getByRole('button',{name:'Review Highland Adventures pack',exact:true}).click();
    await page.getByRole('button',{name:'Review pack installation',exact:true}).click();
    const preview=page.getByRole('group',{name:'Modpack installation preview',exact:true});await preview.waitFor();
    assert.match(await preview.textContent(),/Minecraft 1.21.11.*Fabric Loader 0.19.5.*New independent instance/s);
    await preview.getByRole('checkbox').check();assert.equal(await page.getByRole('button',{name:'Install exact version',exact:true}).isDisabled(),true);
    await page.getByRole('button',{name:'Review selection',exact:true}).click();
    await page.getByRole('button',{name:'Install exact version',exact:true}).click();
    await page.getByRole('alert').filter({hasText:'Fixture: pack installation failed safely.'}).waitFor();
    assert.equal((await requests('install_modrinth_pack')).length,1);assert.equal((await requests('quick_install_modrinth')).length,0);
    assert.deepEqual((await requests('install_modrinth_pack'))[0].selectedOptional,['resourcepacks/optional.zip']);
    await page.screenshot({path:'docs/ui-3/states/modpack-review.png'});
  });
  await check('Resource packs use their native kind and distinct medium artwork',async()=>{
    await openContentReview(page,'packs');
    assert.equal((await requests('browse_modrinth')).at(-1).contentType,'resourcePack');
    assert.equal(await page.locator('.browse-glyph .installed-artwork').first().evaluate(e=>e.clientWidth),64);
    await page.getByRole('button',{name:'Faithful 32x is installed',exact:true}).waitFor();
    assert.match(await page.locator('.filter-context').textContent(),/Minecraft 1.21.11/);
  });
  await check('Loading, empty and provider error/retry states stay distinct',async()=>{
    await openContentReview(page);await page.evaluate(()=>window.ui3Mode='loading');await submit('fixture');
    await page.getByRole('status').filter({hasText:'Loading Modrinth projects…'}).waitFor();
    await page.screenshot({path:'docs/ui-3/states/loading.png'});
    await page.evaluate(()=>{window.ui3Mode='empty';window.ui3Pending();});
    await page.getByRole('heading',{name:'No matching results',exact:true}).waitFor();
    await page.screenshot({path:'docs/ui-3/states/empty.png'});
    await page.evaluate(()=>window.ui3Mode='error');await submit('fixture');
    await page.getByRole('alert').filter({hasText:'Fixture: Modrinth is unavailable.'}).waitFor();
    await page.screenshot({path:'docs/ui-3/states/error.png'});
    await page.evaluate(()=>window.ui3Mode='normal');await search().fill('Sodium');await page.getByRole('button',{name:'Retry search',exact:true}).click();await rows(3);
  });
  await check('Installed Mods search/filter/sort, switch, details and cancel confirmation',async()=>{
    await openContentReview(page,'installed-mods');
    await search().fill('Sodium');assert.equal(await page.locator('.mod-row').count(),1);
    await page.getByRole('switch',{name:'Disable Sodium',exact:true}).click();
    await page.getByRole('switch',{name:'Enable Sodium',exact:true}).waitFor();
    await search().fill('');await page.getByRole('combobox',{name:'Filter mods'}).selectOption('disabled');
    assert.ok(await page.locator('.mod-row').count()>0);
    await page.getByRole('combobox',{name:'Filter mods'}).selectOption('all');
    await page.getByRole('combobox',{name:'Sort mods'}).selectOption('state');
    await search().fill('Mod Menu');await page.getByRole('button',{name:'Details for Mod Menu',exact:true}).click();
    await page.locator('.mod-details').waitFor();
    const remove=page.getByRole('button',{name:'Remove Mod Menu',exact:true});await remove.click();
    assert.equal(await page.getByRole('button',{name:'Remove permanently',exact:true}).evaluate(e=>e===document.activeElement),true);
    await page.keyboard.press('Escape');assert.equal(await remove.evaluate(e=>e===document.activeElement),true);
    assert.equal((await requests('remove_instance_mod')).length,0);
  });
  await check('Installed Packs toggle, full details, safe removal cancellation and empty library',async()=>{
    await openContentReview(page,'installed-packs');
    assert.equal(await page.locator('.pack-description').last().evaluate(e=>{const s=getComputedStyle(e);return e.getBoundingClientRect().height<=parseFloat(s.fontSize)*3.5 && e.scrollHeight>e.clientHeight;}),true,'long collapsed description stays compact; full text remains in Details');
    await page.getByRole('switch',{name:'Enable Fresh Animations',exact:true}).click();await page.getByRole('switch',{name:'Disable Fresh Animations',exact:true}).waitFor();
    await page.getByRole('button',{name:'Details for Fresh Animations',exact:true}).click();assert.match(await page.locator('.pack-details').textContent(),/Local pack description/);
    const remove=page.getByRole('button',{name:'Remove Fresh Animations',exact:true});await remove.click();await page.keyboard.press('Escape');
    assert.equal(await remove.evaluate(e=>e===document.activeElement),true);
    assert.equal((await requests('remove_instance_content')).length,0);
    await page.evaluate(()=>window.ui3SetLibrary('empty'));await page.getByRole('heading',{name:'No local resource packs found',exact:true}).waitFor();
  });
  await check('All content layouts and long text remain contained at four required sizes and 200% text',async()=>{
    for(const surface of ['mods','modpacks','packs','installed-mods','installed-packs'])for(const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520],[706,512]]) {
      await page.setViewportSize({width,height});await openContentReview(page,surface);await contained();
      await page.addStyleTag({content:'html {font-size:200% !important}'});await contained();
      if(surface==='packs'&&width===720)await page.screenshot({path:'docs/ui-3/states/resource-packs-text-200.png'});
    }
  });
  await check('Keyboard focus, alternate accents, reduced motion/transparency and cached artwork reuse',async()=>{
    await page.setViewportSize({width:1280,height:800});await openContentReview(page);
    await search().focus();assert.equal(await search().evaluate(e=>getComputedStyle(e).outlineStyle),'solid');
    await page.keyboard.press('Tab');assert.equal(await page.locator('.browse-search > .btn').evaluate(e=>e===document.activeElement),true);
    await page.evaluate(async()=>{const s=await(await fetch('/Review.svelte')).text();const {appearance}=await import(s.match(/from "([^"\n]*appearance.svelte[^"\n]*)"/)[1]);appearance.apply({...appearance.state,accent:{type:'preset',id:'cyan'}});});
    await page.screenshot({path:'docs/ui-3/states/cyan.png'});
    const before=(await requests('resolve_project_artwork')).length;
    await page.getByRole('button',{name:'Installed',exact:true}).click();await page.getByRole('button',{name:'Browse Modrinth',exact:true}).click();await rows(20);
    assert.equal((await requests('resolve_project_artwork')).length,before,'ordinary navigation reuses artwork');
    const client=await page.context().newCDPSession(page);await client.send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-transparency',value:'reduce'},{name:'prefers-reduced-motion',value:'reduce'}]});
    assert.equal(await page.locator('.browse-row').first().evaluate(e=>getComputedStyle(e).backdropFilter),'none');
    const backing=await page.locator('.browse-row').first().evaluate(e=>getComputedStyle(e).backgroundColor);
    assert.match(backing,/^rgb\(/,'reduced transparency uses an opaque backing');
    await page.screenshot({path:'docs/ui-3/states/reduced-transparency.png'});await contained();
  });
  assert.deepEqual(errors,[]);
  await writeFile('docs/ui-3/functional.json',JSON.stringify({cases:results.length,results,pageErrors:errors,externalRequests,provenance:'Actual Svelte components; deterministic browser-only provider fixtures. No native forwarding or owner data.'},null,2));
  console.log(`PASS ${results.length} UI-3 browser groups; zero page errors`);
} finally {await browser.close();}
