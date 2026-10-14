import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {openContentReview,contentGeometry} from './ui3-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({reducedMotion:'reduce'});
const out='docs/ui-3-1';await mkdir(`${out}/states`,{recursive:true});
const errors=[],cases=[];page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
const requests=command=>page.evaluate(c=>window.ui3Requests.filter(r=>r.command===c).map(r=>r.request),command);
try {
  const before=JSON.parse(await readFile(`${out}/before/geometry.json`,'utf8')).measurements;
  const after=JSON.parse(await readFile(`${out}/after/geometry.json`,'utf8')).measurements;
  for(const [key,value] of Object.entries(before))if(!key.startsWith('modpacks-'))assert.deepEqual(after[key],value,key);
  for(const surface of ['mods','packs'])for(const size of ['1440x900','1280x800','1024x768','720x520','706x512']) {
    const hash=async side=>createHash('sha256').update(await readFile(`${out}/${side}/${surface}-${size}.png`)).digest('hex');
    assert.equal(await hash('after'),await hash('before'),`${surface}-${size}: approved screenshot changed`);
  }
  cases.push('20 non-Modpack layouts preserve exact geometry; 10 approved Mods/Resource PNGs are byte-identical');
  for(const size of ['1440x900','1280x800','1024x768','720x520','706x512']) {
    const mods=after[`mods-${size}`],packs=after[`modpacks-${size}`];
    for(const key of ['padding','gap','border','radius','font','lineHeight'])assert.equal(packs.rowStyle[key],mods.rowStyle[key],`${size}: ${key}`);
    assert.deepEqual(packs.title,mods.title,`${size}: title typography`);
    assert.equal(packs.description.clamp,mods.description.clamp);
    assert.equal(packs.artwork.height,mods.artwork.height);
  }
  cases.push('Modpack spacing, typography, borders, artwork and truncation match approved Mods rows at all five sizes');
  for(const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520],[706,512]])for(const textScale of [100,200]) {
    await page.setViewportSize({width,height});await openContentReview(page,'modpacks');
    await page.evaluate(async()=>{
      const source=await(await fetch('/Review.svelte')).text();
      const {wrapReviewInvoke}=await import(source.match(/from "([^"\n]*review-core[^"\n]*)"/)[1]);
      wrapReviewInvoke(previous=>async(command,args)=>{
        const response=await previous(command,args);
        if(command==='browse_modrinth')response.hits[0]={...response.hits[0],title:'A long modpack title — 日本語の冒険と探索 '.repeat(5),summary:'Long multilingual description 中文 العربية 日本語 '.repeat(30),author:'A long authoritative fixture publisher '.repeat(4)};
        return response;
      });
    });
    await page.getByRole('searchbox').fill('Adventure');await page.getByRole('searchbox').press('Enter');
    await page.locator('.browse-row h4').first().filter({hasText:'A long modpack title'}).waitFor();
    if(textScale===200)await page.addStyleTag({content:'html {font-size:200% !important}'});
    const geometry=await contentGeometry(page);assert.equal(geometry.columns.split(' ').length,1);
    assert.equal(geometry.documentOverflow||geometry.contentOverflow,false);
    const metrics=await page.locator('.browse-row').first().evaluate(row=>{
      const box=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom};};
      const art=row.querySelector('.browse-glyph'),copy=row.querySelector('.browse-copy'),actions=row.querySelector('.browse-actions'),description=copy.querySelector('p');
      return {row:box(row),art:box(art),copy:box(copy),actions:box(actions),imageFit:getComputedStyle(art.querySelector('img')).objectFit,description:box(description),lineHeight:parseFloat(getComputedStyle(description).lineHeight),rowOverflow:row.scrollWidth>row.clientWidth+1};
    });
    assert.equal(metrics.rowOverflow,false);assert.equal(metrics.art.width,44);assert.equal(metrics.art.height,44);assert.equal(metrics.imageFit,'contain');
    assert.ok(metrics.description.height<=2*metrics.lineHeight+.5,'Description stays two lines');
    assert.ok(metrics.actions.y>=metrics.copy.bottom-.5 || metrics.actions.x>=metrics.copy.right-.5,'Actions neither collide with nor cover metadata');
    const action=page.locator('.browse-row').first().getByRole('button',{name:/^Review .+ pack$/});
    await action.focus();await action.scrollIntoViewIfNeeded();
    assert.equal(await action.evaluate(e=>e===document.activeElement&&getComputedStyle(e).outlineStyle==='solid'),true);
    const bounds=await action.boundingBox();assert.ok(bounds.height>=40);assert.ok(bounds.x>=0&&bounds.x+bounds.width<=width);
    await page.keyboard.press('Enter');await page.getByRole('combobox',{name:'Version',exact:true}).waitFor();
    await page.getByRole('button',{name:'Review pack installation',exact:true}).click();
    const preview=page.getByRole('group',{name:'Modpack installation preview',exact:true});await preview.waitFor();
    await preview.getByRole('checkbox').check();assert.equal(await page.getByRole('button',{name:'Install exact version',exact:true}).isDisabled(),true);
    await page.getByRole('button',{name:'Review selection',exact:true}).click();
    assert.equal(await page.getByRole('button',{name:'Install exact version',exact:true}).isEnabled(),true);
    assert.equal((await requests('install_modrinth_pack')).length,0);assert.equal((await requests('quick_install_modrinth')).length,0);
    const expanded=await contentGeometry(page);assert.equal(expanded.documentOverflow||expanded.contentOverflow,false);
    if(textScale===100){await page.getByRole('combobox',{name:'Version',exact:true}).scrollIntoViewIfNeeded();await page.screenshot({path:`${out}/states/review-${width}x${height}.png`});}
    if(width===706&&textScale===200){await preview.getByRole('checkbox').scrollIntoViewIfNeeded();await page.screenshot({path:`${out}/states/review-minimum-200.png`});}
    cases.push(`Long rows + keyboard + optional review: ${width}x${height} at ${textScale}%`);
  }
  await page.setViewportSize({width:1280,height:800});await openContentReview(page,'modpacks');
  await page.evaluate(async()=>{
    const source=await(await fetch('/Review.svelte')).text();const module=name=>source.match(new RegExp('from "([^"\\n]*'+name+'[^"\\n]*)"'))[1];
    const {wrapReviewInvoke}=await import(module('review-core'));const {launcher}=await import(module('store.svelte'));
    wrapReviewInvoke(previous=>async(command,args)=>{
      if(command==='install_modrinth_pack') {
        window.ui31Install=args.request;
        const created={...JSON.parse(JSON.stringify(launcher.launcherState.instances[0])),id:'e'.repeat(32),displayName:'Browser-only independent modpack instance'};
        launcher.launcherState.instances.push(created);return created;
      }
      const response=await previous(command,args);
      if(command==='get_modrinth_project')response.versions.push({...response.versions[0],id:'v0000002',name:'Exact second fixture version',versionNumber:'1.4.3'});
      if(command==='preview_modrinth_pack'){response.versionId=args.request.versionId;response.fingerprint='renewed-'+args.request.versionId+'-'+args.request.selectedOptional.length;}
      return response;
    });
  });
  await page.getByRole('button',{name:'Review Highland Adventures pack',exact:true}).click();
  await page.getByRole('combobox',{name:'Version',exact:true}).selectOption('v0000002');
  await page.getByRole('button',{name:'Review pack installation',exact:true}).click();
  await page.getByRole('group',{name:'Modpack installation preview',exact:true}).getByRole('checkbox').check();
  await page.getByRole('button',{name:'Review selection',exact:true}).click();
  await page.getByRole('button',{name:'Install exact version',exact:true}).click();
  await page.getByRole('status').filter({hasText:'installed as a new instance'}).waitFor();
  assert.deepEqual(await page.evaluate(()=>window.ui31Install),{projectId:'pU000000',versionId:'v0000002',selectedOptional:['resourcepacks/optional.zip'],fingerprint:'renewed-v0000002-1'});
  assert.equal((await requests('select_instance')).at(-1).instanceId,'e'.repeat(32));
  assert.equal((await requests('quick_install_modrinth')).length,0);
  cases.push('Exact version, renewed optional fingerprint and independent instance selection succeed through browser-only native-command fixtures');
  await openContentReview(page,'modpacks');
  const client=await page.context().newCDPSession(page);await client.send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-transparency',value:'reduce'},{name:'prefers-reduced-motion',value:'reduce'}]});
  assert.equal(await page.locator('.browse-row').first().evaluate(e=>getComputedStyle(e).backdropFilter),'none');
  assert.match(await page.locator('.browse-row').first().evaluate(e=>getComputedStyle(e).backgroundColor),/^rgb\(/);
  assert.deepEqual(errors,[]);
  await writeFile(`${out}/functional.json`,JSON.stringify({cases,pageErrors:errors,provenance:'Actual components; deterministic browser-only fixtures, no native install or owner data.'},null,2));
  console.log(`PASS ${cases.length} UI-3.1 groups; ten size/text combinations, exact workflow, unchanged approved surfaces; zero page errors.`);
} finally {await browser.close();}
