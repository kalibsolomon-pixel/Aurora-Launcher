import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
const s=JSON.parse((await readFile('.git/performance-p3a-native-session.json','utf8')).replace(/^\uFEFF/,''));
const runtimeUi=process.argv[2]==='--runtime';
const {chromium}=await import('file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9443');
const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));assert.ok(page);
const invoke=(command,request)=>page.evaluate(({command,request})=>window.__TAURI_INTERNALS__.invoke(command,request?{request}:{}),{command,request});
try {
 assert.equal((await invoke('get_application_status')).managedDataRoot,s.managed);
 const state=await invoke('get_launcher_state');assert.equal(state.instances.length,1);
 const instance=state.instances[0];assert.equal(instance.configuration.performanceMode,'balanced');assert.equal(instance.configuration.memoryMib,2048);assert.equal(instance.configuration.additionalJvmArguments,'');
 assert.equal((await invoke('get_accounts')).accounts.length,0);
 const runtime=await invoke('get_instance_runtime_status',{instanceId:instance.id});assert.equal(runtime.status,'ready');assert.equal(runtime.reportedMajorVersion,21);
 await page.locator('.rail-navigation').getByRole('button',{name:'Instances',exact:true}).click();await page.getByRole('button',{name:'Open',exact:true}).click();
 if(runtimeUi){
  await page.getByRole('tab',{name:'Overview',exact:true}).click();
  await page.getByRole('button',{name:'Check Java',exact:true}).click();
  await page.locator('.group-row').filter({has:page.getByText('Java runtime',{exact:true})}).getByText('Ready',{exact:true}).waitFor();
 }else{
  await page.getByRole('tab',{name:'Settings',exact:true}).click();await page.getByLabel('Configuration mode',{exact:true}).scrollIntoViewIfNeeded();
 }
 await page.screenshot({path:`docs/performance-p3a/evidence/native/${runtimeUi?'validated-runtime':'restart-settings'}.png`});
 await writeFile(`docs/performance-p3a/evidence/native/${runtimeUi?'runtime-ui':'restart'}.json`,JSON.stringify({schemaVersion:1,source:'actual isolated native restart',executableSha256:s.sha256,profile:'balanced',memoryMib:2048,additionalJvmArguments:'',accounts:0,runtime:{status:runtime.status,checkedFiles:runtime.checkedFiles,verifiedBytes:runtime.verifiedBytes,major:runtime.reportedMajorVersion},viewport:await page.evaluate(()=>[innerWidth,innerHeight]),dpr:await page.evaluate(()=>devicePixelRatio),realMinecraftLaunch:false},null,2));
 console.log('PASS isolated restart, deep runtime validation and P2 persistence; zero accounts');
} finally {
 await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('plugin:window|close',{label:'main'})).catch(e=>{if(!/Target page, context or browser has been closed/.test(e.message))throw e;});
 await browser.close();
}
