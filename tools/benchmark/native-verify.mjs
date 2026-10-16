// Real isolated Tauri commands. Never invokes login, Play or any account mutation.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
const json=async path=>JSON.parse((await readFile(path,'utf8')).replace(/^\uFEFF/,''));
const session=await json('.git/performance-p3a-native-session.json');
const {chromium}=await import('file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9443');
const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));assert.ok(page);
const out='docs/performance-p3a/evidence/native';await mkdir(out,{recursive:true});
const invoke=(command,request)=>page.evaluate(({command,request})=>window.__TAURI_INTERNALS__.invoke(command,request?{request}:{}),{command,request});
const result={schemaVersion:1,source:'actual isolated development host',executableSha256:session.sha256,checks:[],screenshots:[],realMinecraftLaunch:false,authenticationAttempted:false};
async function checkpoint(){await writeFile(`${out}/verification.json`,JSON.stringify(result,null,2));}
async function capture(name){await page.screenshot({path:`${out}/${name}.png`});result.screenshots.push({name,viewport:await page.evaluate(()=>[innerWidth,innerHeight]),dpr:await page.evaluate(()=>devicePixelRatio),provenance:'actual native WebView; no viewport override'});await checkpoint();}
try {
 const status=await invoke('get_application_status');assert.equal(status.managedDataRoot,session.managed);
 const accounts=await invoke('get_accounts');assert.equal(accounts.accounts.length,0);
 const initial=await invoke('get_launcher_state');assert.equal(initial.instances.length,0);
 result.checks.push('fresh separate managed root, empty registry, zero accounts');await capture('isolated-startup');
 try {
  const instance=await invoke('create_instance',{displayName:'P3-A Workload A preparation',minecraftVersion:'1.21.11',loader:{kind:'fabric',policy:{type:'pinned',version:'0.19.5'}},auroraEnabled:false});
  assert.equal(instance.configuration.performanceMode,null);assert.equal(instance.configuration.memoryMib,2048);
  result.instance={id:instance.id,minecraftVersion:instance.configuration.minecraftVersion,loader:instance.configuration.loader,memoryMib:2048};
  result.checks.push('Workload A base installed through normal verified native lifecycle; world absent');await checkpoint();
  const configuration=instance.configuration;
  const changed=await invoke('update_instance_configuration',{instanceId:instance.id,expectedConfiguration:configuration,configuration:{...configuration,performanceMode:'balanced'}});
  assert.equal(changed.configuration.memoryMib,2048);assert.equal(changed.configuration.additionalJvmArguments,'');
  const disk=await json(`${session.managed}/launcher/instances.json`);assert.equal(disk.schemaVersion,6);assert.equal(disk.instances[0].configuration.performanceMode,'balanced');
  result.checks.push('schema-6 Balanced persisted with unchanged heap/text');
  const runtime=await invoke('ensure_instance_runtime',{instanceId:instance.id});
  assert.equal(runtime.status,'ready');assert.equal(runtime.requiredMajorVersion,21);
  await writeFile('.git/performance-p3a-native-runtime-private.json',JSON.stringify(runtime,null,2));
  result.runtime={component:runtime.component,version:runtime.runtimeVersion,requiredMajor:runtime.requiredMajorVersion,checkedFiles:runtime.checkedFiles,verifiedBytes:runtime.verifiedBytes,status:runtime.status};
  result.checks.push('exact game-plan Java installed/validated by existing SHA-1 runtime path and bounded version diagnostic');await checkpoint();
  await page.reload();
  await page.locator('.rail-navigation').getByRole('button',{name:'Instances',exact:true}).click();
  await page.getByRole('button',{name:'Open',exact:true}).click();
  await capture('benchmark-instance');
  await page.getByRole('tab',{name:'Settings',exact:true}).click();
  await page.getByLabel('Configuration mode',{exact:true}).waitFor();
  assert.equal(await page.getByLabel('Configuration mode',{exact:true}).inputValue(),'balanced');
  await page.getByLabel('Configuration mode',{exact:true}).scrollIntoViewIfNeeded();await capture('performance-settings');
  await page.getByRole('tab',{name:'Overview',exact:true}).click();await capture('runtime-and-auth-required');
 } catch(error) {
  if(error?.code==='ERR_ASSERTION')throw error;
  // Native error codes are safe; never persist raw response bodies or live commands.
  result.preparationBlocker=typeof error?.code==='string'?error.code:'native-preparation-or-verification-failed';
  console.error('Preparation gate:',result.preparationBlocker);await checkpoint();
 }
 assert.equal((await invoke('get_accounts')).accounts.length,0);
 result.checks.push('no login/Play/credential operation; zero accounts at shutdown');await checkpoint();
 console.log(JSON.stringify(result));
} finally {
 await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('plugin:window|close',{label:'main'})).catch(error=>{
  if(!/Target page, context or browser has been closed/.test(error.message))throw error;
 });
 await browser.close();
}
