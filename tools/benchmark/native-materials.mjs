// Prepare pinned B/C artifacts through existing native installers; never Play/login.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
const s=JSON.parse((await readFile('.git/performance-p3a-native-session.json','utf8')).replace(/^\uFEFF/,''));
const {chromium}=await import('file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9443');
const page=browser.contexts()[0].pages().find(p=>p.url().startsWith('http://tauri.localhost/'));assert.ok(page);
const invoke=(command,request)=>page.evaluate(({command,request})=>window.__TAURI_INTERNALS__.invoke(command,request?{request}:{}),{command,request});
const result={schemaVersion:1,evidenceScope:'actual isolated B/C native artifact preparation; runtime/game compatibility execution NOT TESTED',instances:[],accounts:0,authenticationAttempted:false,realMinecraftLaunch:false};
const save=()=>writeFile('docs/performance-p3a/evidence/native/materials.json',JSON.stringify(result,null,2));
try {
 assert.equal((await invoke('get_application_status')).managedDataRoot,s.managed);
 assert.equal((await invoke('get_accounts')).accounts.length,0);
 const initial=await invoke('get_launcher_state');assert.equal(initial.instances.length,1);
 for(const workload of ['B','C']){
  const instance=await invoke('create_instance',{displayName:`P3-A Workload ${workload} preparation`,minecraftVersion:'1.21.11',loader:{kind:'fabric',policy:{type:'pinned',version:'0.19.5'}},auroraEnabled:true});
  assert.equal(instance.configuration.auroraEnabled,true);
  const evidence={workload,id:instance.id,configuration:instance.configuration,mods:[]};result.instances.push(evidence);await save();
  for(const [projectId,versionId] of [['AANobbMI','rkdTcxoT'],['gvQqBUqZ','Ow7wA0kG']]){
   const request={instanceId:instance.id,contentType:'mod',projectId,versionId};
   const preview=await invoke('preview_modrinth_install',request);
   assert.equal(preview.conflicts.length,0,'unexpected reviewed-install conflict');
   const records=await invoke('install_modrinth',{...request,previewFingerprint:preview.previewFingerprint});
   evidence.mods.push({projectId,versionId,records});await save();
  }
  const validated=await invoke('validate_instance',{instanceId:instance.id});
  assert.equal(validated.status,'ready');assert.deepEqual(validated.problems,[]);
  await writeFile(`.git/performance-p3a-workload-${workload.toLowerCase()}-private.json`,JSON.stringify(validated,null,2));
  evidence.nativeValidationCompleted=true;await save();
 }
 assert.equal((await invoke('get_accounts')).accounts.length,0);
 console.log('PASS isolated pinned B/C artifacts installed; no authenticated execution');
}catch(error){
 result.blocker=typeof error?.code==='string'?error.code:'native-material-preparation-failed';await save();
 if(error?.code==='ERR_ASSERTION')throw error;
 console.log('BLOCKED material preparation:',result.blocker);
}finally{
 await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('plugin:window|close',{label:'main'})).catch(e=>{if(!/Target page, context or browser has been closed/.test(e.message))throw e;});
 await browser.close();
}
