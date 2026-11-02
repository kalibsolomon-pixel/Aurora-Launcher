import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {openSettings} from './ui5-fixture.mjs';
const {chromium}=await import('file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.launch({headless:true}),page=await browser.newPage({viewport:{width:1120,height:760},reducedMotion:'reduce'});
const out='docs/performance-p2/browser',checks=[],metrics=[],errors=[],ownershipWarnings=[];
page.on('console',message=>{if(message.type()==='warning'&&message.text().includes('ownership_invalid_mutation'))ownershipWarnings.push(message.text());});
await mkdir(out,{recursive:true});page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
const text='-Dlabel="two words" -Dlabel=last';
async function setup() {
 await openSettings(page,'Minecraft & Java');
 await page.evaluate(async text=>{
  const source=await(await fetch('/Review.svelte')).text();
  const module=name=>source.match(new RegExp('from "([^"\\n]*'+name+'[^"\\n]*)"'))[1];
  const {launcher}=await import(module('store.svelte'));
  const {wrapReviewInvoke}=await import(module('review-core'));
  const instance=launcher.selectedInstance;
  instance.configuration={...instance.configuration,memoryMib:3217,additionalJvmArguments:text};
   window.p2Requests=[];window.p2AdviceFailure=false;window.p2AdviceDelay=0;
  wrapReviewInvoke(previous=>async(command,args)=>{
   if(command==='get_launcher_state') return JSON.parse(JSON.stringify(launcher.launcherState));
   if(command==='get_instance_performance_advice') {
    if(window.p2AdviceDelay)await new Promise(resolve=>setTimeout(resolve,window.p2AdviceDelay));
    if(window.p2AdviceFailure)throw {code:'fixture_advice_failure',message:'Memory advice is unavailable. Saved settings are unchanged.'};
    return {facts:{totalMib:8192,availableMib:6144,architecture:'x86_64',source:'synthetic browser fixture',ageSeconds:0},recommendedMib:args.request.mode==='efficient'?1536:2048,explanation:'Conservative synthetic advice; no measured performance claim.',unknowns:['GPU and workload pressure unknown.'],argumentNotices:['Repeated system properties keep order; last value takes effect.']};
   }
   if(command==='update_instance_configuration') {
    const request=JSON.parse(JSON.stringify(args.request));
    window.p2Requests.push(request);
    if(args.request.configuration.additionalJvmArguments.includes('-Xmx'))throw {code:'instance_configuration_invalid',message:'Aurora owns the heap; edit Memory instead. Your draft has been kept.'};
    return {...instance,configuration:request.configuration};
   }
   return previous(command,args);
  });
 },text);
 await page.getByRole('button',{name:'Open instance Settings',exact:true}).click();
 await page.getByLabel('Configuration mode',{exact:true}).waitFor();
 await page.getByText('Recommended maximum: 2048 MiB',{exact:false}).waitFor();
}
const save=()=>page.getByRole('button',{name:'Save changes',exact:true});
const heap=()=>page.getByLabel('Memory (MiB)',{exact:true});
const raw=()=>page.getByLabel('Additional JVM arguments',{exact:true});
async function capture(name) {
 const m=await page.evaluate(()=>({viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,documentOverflow:document.documentElement.scrollWidth>innerWidth,mainOverflow:document.querySelector('main.content').scrollWidth>document.querySelector('main.content').clientWidth+1}));
 assert.equal(m.documentOverflow||m.mainOverflow,false,name);metrics.push({name,...m});await page.screenshot({path:`${out}/${name}.png`});
}
try {
 await setup();assert.equal(await heap().inputValue(),'3217');assert.equal(await raw().inputValue(),text);await capture('existing-default');
 for(const mode of ['balanced','efficient','performance','custom','balanced','custom','efficient']) {
  await page.getByLabel('Configuration mode',{exact:true}).selectOption(mode);
  assert.equal(await heap().inputValue(),'3217');assert.equal(await raw().inputValue(),text);
  await save().click();
  await page.getByText(new RegExp(`^Saved mode: ${mode[0].toUpperCase()+mode.slice(1)} ·`)).waitFor();
  assert.equal(await save().isDisabled(),true);
 }
 checks.push('all transitions persist mode without changing heap or custom text');
 await page.getByRole('button',{name:'Use 1536 MiB in draft',exact:true}).click();assert.equal(await heap().inputValue(),'1536');
 await page.getByRole('button',{name:'Undo memory recommendation',exact:true}).click();assert.equal(await heap().inputValue(),'3217');checks.push('recommendation is explicit, refreshed, undoable and unsaved');
 await page.evaluate(()=>window.p2AdviceDelay=200);
 await page.getByRole('button',{name:'Use 1536 MiB in draft',exact:true}).click();await heap().fill('4000');
 await page.getByText('Your draft changed. Review the recommendation before using it.',{exact:true}).waitFor();
 assert.equal(await heap().inputValue(),'4000');await heap().fill('3217');await page.evaluate(()=>window.p2AdviceDelay=0);
 checks.push('delayed memory advice never overwrites a newer manual heap');
 await page.getByLabel('Configuration mode',{exact:true}).selectOption('custom');await save().click();await page.getByText('Saved mode: Custom',{exact:false}).waitFor();
 await raw().fill('-Xmx4G');await save().click();await page.getByRole('alert').filter({hasText:'Aurora owns the heap'}).waitFor();
 assert.equal(await raw().inputValue(),'-Xmx4G');await page.getByText('Saved mode: Custom',{exact:false}).waitFor();await capture('rejected-save');checks.push('rejected save retains saved state and editable draft');
 await page.getByRole('button',{name:'Discard',exact:true}).click();assert.equal(await raw().inputValue(),text);
 for(const [width,height] of [[1120,760],[1024,768],[720,520],[712,512]])for(const scale of [1,2]) {
  await page.setViewportSize({width,height});await setup();await page.getByLabel('Configuration mode',{exact:true}).selectOption('custom');
  if(scale===2)await page.evaluate(()=>{const elements=[...document.querySelectorAll('main.content :is(p,span,strong,label,button,input,select,h2,h3,h4,summary)')];const sizes=elements.map(e=>parseFloat(getComputedStyle(e).fontSize));elements.forEach((e,i)=>e.style.fontSize=`${sizes[i]*2}px`);});
  await page.getByLabel('Configuration mode',{exact:true}).focus();assert.equal(await page.getByLabel('Configuration mode',{exact:true}).evaluate(e=>e===document.activeElement),true);
  await capture(`custom-${width}x${height}-text${scale}`);
  await raw().scrollIntoViewIfNeeded();await capture(`advanced-${width}x${height}-text${scale}`);
 }
 checks.push('keyboard focus, responsive and 200% text layouts');
 await setup();await page.evaluate(()=>window.p2AdviceFailure=true);await page.getByLabel('Configuration mode',{exact:true}).selectOption('efficient');
 await page.getByRole('alert').filter({hasText:'Memory advice is unavailable'}).waitFor();assert.equal(await heap().inputValue(),'3217');checks.push('missing hardware/advice failure does not change heap');
 await page.emulateMedia({forcedColors:'active',reducedMotion:'reduce'});await capture('forced-colors');
 assert.deepEqual(errors,[]);assert.deepEqual(ownershipWarnings,[]);await writeFile(`${out}/results.json`,JSON.stringify({provenance:'Synthetic browser fixtures, not native acceptance',checks,metrics,errors,ownershipWarnings},null,2));console.log('PASS',checks);
}finally{await browser.close();}
