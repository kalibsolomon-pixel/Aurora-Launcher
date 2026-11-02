// Integration test of the existing native installer against only this run's
// fresh disposable Fabric instance. No synthetic Ready state, account or launch.
import {readFile,writeFile} from 'node:fs/promises';
import {join,resolve,dirname,basename} from 'node:path';
import {tmpdir} from 'node:os';
import assert from 'node:assert/strict';
const startBytes=await readFile('docs/ui-3/native/start.json');
const start=JSON.parse(startBytes.toString(startBytes[0]===255 && startBytes[1]===254 ? 'utf16le' : 'utf8').replace(/^\uFEFF/,''));
assert.equal(resolve(dirname(start.isolatedRoot)),resolve(tmpdir()));
assert.ok(basename(start.isolatedRoot).startsWith('aurora-ui3-'));
assert.equal((await readFile(join(start.isolatedRoot,'UI3-ISOLATED.txt'),'utf8')).trim(),'UI3 isolated acceptance; no owner data; retain this root');
const registry=JSON.parse(await readFile(join(start.managed,'launcher/instances.json'),'utf8'));
assert.equal(registry.instances.length,1);assert.equal(registry.instances[0].state,'installing');
assert.equal(registry.instances[0].id,'1234567890abcdef1234567890abcdef');
assert.equal(registry.instances[0].configuration.auroraEnabled,false);
const {chromium}=await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser=await chromium.connectOverCDP('http://127.0.0.1:9433');
try {
  const page=browser.contexts()[0].pages().find(p=>p.url()==='http://tauri.localhost/');assert.ok(page);
  const t=Date.now();
  const result=await page.evaluate(async()=>{
    try {return {ok:true,instance:await window.__TAURI_INTERNALS__.invoke('retry_instance_install',{request:{instanceId:'1234567890abcdef1234567890abcdef'}})};}
    catch(error){return {ok:false,error};}
  });
  await writeFile('docs/ui-3/native/install-result.json',JSON.stringify({...result,elapsedMs:Date.now()-t,provenance:'Actual compiled native retry_instance_install in new marked isolated root'},null,2));
  console.log(JSON.stringify(result));process.exitCode=result.ok?0:1;
} finally {await browser.close();}
