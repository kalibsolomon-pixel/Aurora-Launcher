import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
const phase=process.argv[2]??'first';assert.ok(['first','restart'].includes(phase));
const json=async path=>JSON.parse((await readFile(path,'utf8')).replace(/^\uFEFF/,''));
const start=await json(`.git/performance-p2-native-${phase}.json`);
const {chromium}=await import('file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9442');
const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));assert.ok(page);
const out='docs/performance-p2/native';await mkdir(out,{recursive:true});
const invoke=(command,request)=>page.evaluate(({command,request})=>window.__TAURI_INTERNALS__.invoke(command,request?{request}:{}),{command,request});
const status=await invoke('get_application_status');assert.equal(status.managedDataRoot,start.managed);
const records=[],checks=[],text='-Dlabel="two words" -Dlabel=last';
const pwsh='C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe';
function window(mode,width=1120,height=760) {
 return JSON.parse(execFileSync(pwsh,['-NoProfile','-File','tools/visual/performance-p2-window.ps1','-Phase',phase,'-Mode',mode,'-Width',String(width),'-Height',String(height)],{encoding:'utf8',windowsHide:true,timeout:15000}));
}
async function settings() {
 await page.locator('.rail-navigation').getByRole('button',{name:'Instances',exact:true}).click();
 await page.getByRole('button',{name:'Open',exact:true}).click();
 await page.getByRole('tab',{name:'Settings',exact:true}).click();
 await page.getByLabel('Configuration mode',{exact:true}).waitFor();
}
async function capture(name,geometry) {
 const target=name.includes('advanced')?page.getByLabel('Additional JVM arguments',{exact:true}):name.includes('save-error')?page.getByRole('alert').filter({hasText:'Aurora owns the heap'}):name.includes('heap-text200')?page.getByLabel('Memory (MiB)',{exact:true}):page.getByLabel('Configuration mode',{exact:true});
 await target.scrollIntoViewIfNeeded();
 await page.waitForTimeout(180);
 const m=await page.evaluate(()=>({viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,documentOverflow:document.documentElement.scrollWidth>innerWidth,mainOverflow:document.querySelector('main.content').scrollWidth>document.querySelector('main.content').clientWidth+1}));
 assert.equal(m.documentOverflow||m.mainOverflow,false,name);
 records.push({name,...m,geometry,provenance:'Compiled frontend in actual Windows Tauri/WebView2 host; no fixture substitution or browser viewport resizing'});
 await page.screenshot({path:`${out}/${name}.png`});
}
const saved=async()=>{await page.waitForFunction(()=>[...document.querySelectorAll('button')].some(b=>b.textContent.trim()==='Save changes'&&b.disabled));};
const cfg=async()=>{const state=await invoke('get_launcher_state');return state.instances[0].configuration;};
try {
 const before=await cfg();assert.equal(before.memoryMib,3217);assert.equal(before.additionalJvmArguments,text);
 if(phase==='first')assert.equal(before.performanceMode,null);else assert.equal(before.performanceMode,'custom');
 await settings();
 let geometry=window('size');await page.locator('main.content').evaluate(e=>e.scrollTop=0);await capture(`${phase}-default`,geometry);
 if(phase==='first') {
  const advice=await invoke('get_instance_performance_advice',{instanceId:'a'.repeat(32),mode:'efficient'});
  assert.ok(advice.facts.source.includes('Windows'));if(advice.recommendedMib!==null)assert.ok(advice.recommendedMib<=1536);
  checks.push('real Windows memory advice and untouched legacy heap/text');
  for(const mode of ['balanced','efficient','performance','custom']) {
   await page.getByLabel('Configuration mode',{exact:true}).selectOption(mode);
   assert.equal(await page.getByLabel('Memory (MiB)',{exact:true}).inputValue(),'3217');
   assert.equal(await page.getByLabel('Additional JVM arguments',{exact:true}).inputValue(),text);
   if(mode==='efficient')await capture('unsaved-efficient',geometry);
   await page.getByRole('button',{name:'Save changes',exact:true}).click();await saved();
   const current=await cfg();assert.equal(current.performanceMode,mode);assert.equal(current.memoryMib,3217);assert.equal(current.additionalJvmArguments,text);
  }
  checks.push('all four modes persist through actual native commands without replacing heap/text');
  await capture('custom-mode',geometry);
  await page.getByLabel('Additional JVM arguments',{exact:true}).scrollIntoViewIfNeeded();await capture('advanced-jvm',geometry);
  await page.getByLabel('Additional JVM arguments',{exact:true}).fill('-Xmx4G');
  await page.getByRole('button',{name:'Save changes',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'Aurora owns the heap'}).waitFor();
  const rejected=await cfg();assert.equal(rejected.additionalJvmArguments,text);assert.equal(rejected.memoryMib,3217);
  await capture('native-save-error',geometry);checks.push('native rejected save keeps authoritative settings and editable error draft');
  await page.getByRole('button',{name:'Discard',exact:true}).click();
  geometry=window('size',1024,768);await page.locator('main.content').evaluate(e=>e.scrollTop=0);await capture('native-1024x768',geometry);
  geometry=window('minimum');await capture('native-minimum',geometry);assert.ok(geometry.nativeMinimum.X>0);
  await page.evaluate(()=>{const elements=[...document.querySelectorAll('main.content :is(p,span,strong,label,button,input,select,h2,h3,h4,summary)')];const sizes=elements.map(e=>parseFloat(getComputedStyle(e).fontSize));elements.forEach((e,i)=>{e.style.fontSize=`${sizes[i]*2}px`;});});
  await capture('native-minimum-text200',geometry);checks.push('measured native minimum and 200% text remain horizontally contained');
  await capture('native-minimum-heap-text200',geometry);
  await capture('native-minimum-advanced-text200',geometry);
  await page.reload();await page.getByRole('button',{name:'Settings',exact:true}).waitFor();
  await page.locator('.rail-navigation').getByRole('button',{name:'Settings',exact:true}).click();
  await page.locator('.settings-nav').getByRole('button',{name:'Appearance',exact:true}).click();
  await page.getByRole('radio',{name:'Rose',exact:true}).check();await page.getByText('Saved. Changes are live.',{exact:true}).waitFor();
  await settings();geometry=window('size');await capture('native-rose-accent',geometry);
  checks.push('actual native Rose preference applied without production state');
 } else {
  checks.push('restart restores Custom, exact 3217 MiB and byte-exact custom text');
  assert.equal(await page.getByLabel('Configuration mode',{exact:true}).inputValue(),'custom');
  const state=await invoke('get_launcher_state');assert.equal(state.config.schemaVersion,7);
  await capture('restart-custom',geometry);
 }
 const disk=await json(`${start.managed}/launcher/instances.json`);assert.equal(disk.schemaVersion,6);
 assert.equal(disk.instances[0].configuration.performanceMode,'custom');
 const accounts=await invoke('get_accounts');assert.equal(accounts.accounts.length,0);
 const final=await cfg();
 await writeFile(`${out}/${phase}-verification.json`,JSON.stringify({phase,executableSha256:start.sha256,provenance:'Actual isolated native host; schema-5 synthetic record, no installed Minecraft or accounts, no command substitution',checks,records,finalConfiguration:final,registrySchema:disk.schemaVersion,accounts:0,realMinecraftLaunch:false},null,2));
 console.log('PASS native',phase,checks);window('close');
}finally{await browser.close();}
