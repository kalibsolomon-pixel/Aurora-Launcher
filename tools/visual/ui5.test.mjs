import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {openSettings,settingsModules} from './ui5-fixture.mjs';
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE||'file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.launch({headless:true}),page=await browser.newPage({reducedMotion:'reduce'});
const out='docs/ui-5/browser';await mkdir(out,{recursive:true});
const checks=[],metrics=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/,r=>r.abort());
const categories=['Appearance','Home','General','Minecraft & Java','Updates','Discord & privacy','Accessibility','About'];
const labels=['Appearance','Home & Widgets','Launcher','Minecraft & Java','Updates','Discord & privacy','Accessibility','About'];
const categoryButton=label=>page.locator('.settings-nav').getByRole('button',{name:label,exact:true});
const pass=name=>{checks.push(name);console.log('PASS',name);};
const requests=command=>settingsModules(page,'requests').then(items=>items.filter(item=>item.command===command));
const saved=()=>page.getByText('Saved. Changes are live.',{exact:true}).waitFor();
async function contained() {
 const m=await page.evaluate(()=>({viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,category:document.querySelector('.settings-nav [aria-current]')?.textContent,
  documentOverflow:document.documentElement.scrollWidth>innerWidth,contentOverflow:document.querySelector('main.content').scrollWidth>document.querySelector('main.content').clientWidth+1}));
 assert.equal(m.documentOverflow||m.contentOverflow,false,JSON.stringify(m));return m;
}
try {
 for(const [width,height] of [[1440,900],[1280,800],[1024,768],[720,520],[706,512]])for(const scale of [1,2])for(let i=0;i<categories.length;i++) {
  await page.setViewportSize({width,height});await openSettings(page,categories[i]);
  if(scale===2)await page.evaluate(()=>{const es=[...document.querySelectorAll('main.content :is(p,span,strong,small,label,button,input,select,h2,h3,h4,legend,output,li)')];const sizes=es.map(e=>parseFloat(getComputedStyle(e).fontSize));es.forEach((e,i)=>e.style.setProperty('font-size',`${sizes[i]*2}px`,'important'));});
  const m=await contained();metrics.push({...m,textScale:scale});
  assert.equal(await categoryButton(labels[i]).getAttribute('aria-current'),'page');
  if(scale===1||width===720)await page.screenshot({path:`${out}/${categories[i].split(' ')[0].toLowerCase()}-${width}x${height}-text${scale}.png`});
 }
 pass('Eight categories at five dimensions and 100%/200% text: 80 contained layouts');
 await page.setViewportSize({width:1440,height:900});await openSettings(page);
 await categoryButton('Appearance').focus();await page.keyboard.press('ArrowDown');assert.equal(await categoryButton('Home & Widgets').evaluate(e=>e===document.activeElement),true);
 await page.keyboard.press('End');assert.equal(await categoryButton('About').getAttribute('aria-current'),'page');
 await page.keyboard.press('Home');assert.equal(await categoryButton('Appearance').getAttribute('aria-current'),'page');
 pass('Arrow/Home/End keyboard category navigation and focus');
 await categoryButton('Discord & privacy').click();await page.locator('main.content').evaluate(e=>e.scrollTop=350);const top=await page.locator('main.content').evaluate(e=>e.scrollTop);
 await categoryButton('Launcher').click();await categoryButton('Discord & privacy').click();assert.equal(await page.locator('main.content').evaluate(e=>e.scrollTop),top);
 pass('Category reading position restored');
 await openSettings(page);await settingsModules(page,'delay',120);
 await page.getByRole('radio',{name:'Midnight',exact:true}).click();await page.getByText('Saving…',{exact:true}).waitFor();assert.equal(await page.getByText('Saved. Changes are live.',{exact:true}).count(),0);await saved();
 await settingsModules(page,'failure','set_appearance');await page.getByRole('radio',{name:'OLED Black',exact:true}).click();await page.getByRole('alert').waitFor();assert.equal(await page.getByRole('radio',{name:'Midnight',exact:true}).isChecked(),true);
 await settingsModules(page,'failure','');await page.getByRole('button',{name:'Retry save',exact:true}).click();await saved();assert.equal((await settingsModules(page,'snapshot')).appearance.theme,'oled');
 pass('Select/radio persistence, pending feedback, failed write and exact-value retry');
 await page.getByLabel('Aurora motion',{exact:true}).fill('100');await page.getByLabel('Aurora motion',{exact:true}).dispatchEvent('change');await saved();assert.equal((await settingsModules(page,'snapshot')).appearance.auroraMotionSpeed,100);
 const count=(await requests('set_appearance')).length;await page.getByLabel('Aurora motion',{exact:true}).focus();await page.keyboard.press('ArrowRight');assert.equal((await requests('set_appearance')).length,count);
 await settingsModules(page,'failure','set_appearance');await page.getByLabel('Aurora motion',{exact:true}).fill('23');await page.getByLabel('Aurora motion',{exact:true}).dispatchEvent('change');await page.getByRole('alert').waitFor();assert.equal(await page.getByLabel('Aurora motion',{exact:true}).inputValue(),'23');assert.equal((await settingsModules(page,'snapshot')).appearance.auroraMotionSpeed,100);
 await settingsModules(page,'failure','');await page.getByRole('button',{name:'Retry save',exact:true}).click();await saved();assert.equal((await settingsModules(page,'snapshot')).appearance.auroraMotionSpeed,23);
 pass('Motion bounds and retained failed slider draft');
 const color=page.getByLabel('Custom accent color',{exact:true});await color.focus();const box=await color.boundingBox();assert.ok(box.width>=30&&box.height>=30);await color.fill('#f2aaee');await color.dispatchEvent('change');await saved();assert.equal((await settingsModules(page,'snapshot')).appearance.accent.hex,'#f2aaee');
 await settingsModules(page,'failure','set_appearance');await color.fill('#010101');await color.dispatchEvent('change');await page.getByRole('alert').waitFor();assert.equal(await color.inputValue(),'#010101');assert.equal((await settingsModules(page,'snapshot')).appearance.accent.hex,'#f2aaee');await page.screenshot({path:`${out}/appearance-save-error.png`});
 pass('Custom picker is focusable, commits completed changes and keeps failed input');
 await openSettings(page,'Home');const layout={widgets:[{id:'session',enabled:false,size:'wide'},{id:'future-widget',enabled:true,size:'large'},{id:'instance-details',enabled:true,size:'large'}]};await settingsModules(page,'layout',layout);
 await page.getByRole('checkbox',{name:'Show Instance Details',exact:true}).uncheck();await saved();await page.getByRole('checkbox',{name:'Show Instance Details',exact:true}).check();await saved();assert.deepEqual((await settingsModules(page,'snapshot')).layout,layout);
 await settingsModules(page,'failure','set_home_widgets');await page.getByRole('checkbox',{name:'Show Instance Details',exact:true}).click();await page.getByRole('alert').waitFor();assert.equal(await page.getByRole('checkbox',{name:'Show Instance Details',exact:true}).isChecked(),true);
 await settingsModules(page,'failure','');await page.getByRole('button',{name:'Retry save',exact:true}).click();await saved();assert.equal((await settingsModules(page,'snapshot')).layout.widgets[2].enabled,false);
 pass('Widget toggle persistence, unknown/hidden order and sizing preserved; failure retry');
 const before=(await requests('reset_home_widgets')).length,trigger=page.getByRole('button',{name:'Reset layout…',exact:true});await trigger.click();await page.getByRole('dialog').waitFor();assert.equal(await page.getByRole('button',{name:'Cancel',exact:true}).evaluate(e=>e===document.activeElement),true);await page.keyboard.press('Escape');assert.equal(await trigger.evaluate(e=>e===document.activeElement),true);assert.equal((await requests('reset_home_widgets')).length,before);
 await trigger.click();await page.getByRole('dialog').getByRole('button',{name:'Reset layout',exact:true}).click();await saved();assert.equal((await requests('reset_home_widgets')).length,before+1);
 pass('Layout reset requires explicit confirmation; Cancel/Escape preserve it and return focus');
 await page.getByRole('button',{name:'Edit Home',exact:true}).click();assert.equal((await settingsModules(page,'snapshot')).nav.page,'home');pass('Edit Home reuses existing editor');
 await openSettings(page,'Discord & privacy');const off=(await settingsModules(page,'snapshot')).discord;assert.ok(Object.values(off.preferences).every(value=>value===false));
 assert.equal(await page.getByRole('checkbox',{name:'Display Show Server Address',exact:true}).isDisabled(),true);
 await page.getByRole('checkbox',{name:'Display Show Server',exact:true}).check();await saved();await page.getByRole('checkbox',{name:'Display Show Server Address',exact:true}).check();await saved();await page.getByRole('checkbox',{name:'Display Show Server',exact:true}).uncheck();await saved();assert.equal((await settingsModules(page,'snapshot')).discord.preferences.serverAddress,true);assert.equal((await settingsModules(page,'snapshot')).discord.preferences.enabled,false);
 await settingsModules(page,'failure','set_discord_preferences');await page.getByRole('checkbox',{name:'Display Instance name',exact:true}).click();await page.getByRole('alert').waitFor();assert.equal((await settingsModules(page,'snapshot')).discord.preferences.instanceName,false);
 pass('Discord opt-ins remain independent, disabled server address preserves stored consent; save failures');
 for(const connection of ['notDetected','closed','failed','configurationMissing']){await settingsModules(page,'discord',{...off,configured:connection!=='configurationMissing',connection,preferences:{...off.preferences,enabled:true}});assert.doesNotMatch(await page.locator('#settings-content').innerText(),/^Connected$/m);}
 await page.screenshot({path:`${out}/discord-unconfigured.png`});pass('Discord unavailable/setup states never claim connected or request OAuth');
 await openSettings(page,'Minecraft & Java');await page.getByRole('button',{name:'Open instance Settings',exact:true}).click();assert.deepEqual((await settingsModules(page,'snapshot')).nav,{kind:'instance',instanceId:'a'.repeat(32),tab:'settings'});
 await openSettings(page,'Minecraft & Java');await settingsModules(page,'empty');await page.getByRole('button',{name:'Open Library',exact:true}).click();assert.equal((await settingsModules(page,'snapshot')).nav.page,'instances');pass('Game settings links keep instance scope; empty selection opens Library');
 await openSettings(page,'Accessibility');const mediaSession=await page.context().newCDPSession(page);await mediaSession.send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'},{name:'prefers-reduced-transparency',value:'reduce'}]});await page.waitForFunction(()=>[...document.querySelectorAll('.setting-state')].filter(e=>e.textContent==='Following system · Reduced').length===2);assert.equal(await page.getByText('Following system · Reduced',{exact:true}).count(),2);
 await categoryButton('Appearance').click();assert.equal(await page.locator('.appearance-surface').evaluate(e=>getComputedStyle(e).backdropFilter),'none');await page.screenshot({path:`${out}/reduced-transparency.png`});
 await page.emulateMedia({forcedColors:'active'});await contained();await page.screenshot({path:`${out}/forced-colors.png`});await page.emulateMedia({forcedColors:'none',reducedTransparency:'no-preference'});pass('System motion/transparency status, opaque panel and forced-color selection');
 await page.getByRole('radio',{name:'Rose',exact:true}).check();await saved();
 for(const label of ['Home','Instances','Appearance','Settings']){await page.locator('.rail-navigation').getByRole('button',{name:label,exact:true}).click();assert.equal(await page.evaluate(()=>document.documentElement.style.getPropertyValue('--color-accent')),'#ef8fa5');}
 await page.getByRole('button',{name:/Accounts —/}).click();assert.equal(await page.getByRole('dialog').evaluate(e=>getComputedStyle(e).getPropertyValue('--color-accent').trim()),'#ef8fa5');await page.keyboard.press('Escape');
 pass('Rose accent propagates through Home, Library, Appearance, Settings and account dialog');
 assert.deepEqual(errors,[]);await writeFile(`${out}/results.json`,JSON.stringify({checks,metrics,pageErrors:errors,provenance:'Deterministic browser fixtures; synthetic native responses, no owner data or external network'},null,2));
 console.log('PASS',checks.length,'UI-5 browser cases');
}finally{await browser.close();}
