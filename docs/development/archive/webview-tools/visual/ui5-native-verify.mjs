import {readFile,writeFile,stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const json=async path=>JSON.parse((await readFile(path,'utf8')).replace(/^\uFEFF/,''));
const before=await json('docs/ui-5/native-final/before-restart.json');
const after=await json('docs/ui-5/native-persistence/restored-home.json');
assert.deepEqual(after.preferences,before.preferences,'Native preferences must survive restart');
assert.equal(after.preferences.appearance.background,'borealis');
assert.deepEqual(after.preferences.appearance.accent,{type:'preset',id:'cyan'});
assert.equal(after.preferences.appearance.auroraMotionSpeed,70);
assert.equal(after.preferences.widgets.widgets.find(w=>w.id==='instance-details').enabled,false);
assert.ok(Object.values(after.preferences.discord.preferences).every(value=>value===false));
const start=await json('docs/ui-5/native-persistence/start.json');
assert.match(start.isolatedRoot,/\\Temp\\aurora-ui5-[a-f0-9-]+$/);
await readFile(`${start.isolatedRoot}/UI5-ISOLATED.txt`);
const config=await json(`${start.managed}/launcher/config.json`);
assert.equal(config.schemaVersion,7);assert.equal(config.selectedInstanceId,null);
assert.deepEqual(config.homeWidgets,after.preferences.widgets);
const owner=await json('docs/ui-5/owner-before-native.json');
const ownerObservations=[];
for(const file of owner.files)if(file.sha256){
 const currentHash=createHash('sha256').update(await readFile(file.path)).digest('hex');
 const unchanged=currentHash===file.sha256;
 ownerObservations.push({path:file.path,baselineHash:file.sha256,currentHash,unchanged});
 if(!unchanged){
  assert.ok(file.path.endsWith('/launcher/config.json'),'Unexpected owner file drift');
  const info=await stat(file.path),sessionInfo=await stat('docs/ui-5/native-final/start.json');
  assert.ok(info.mtimeMs<sessionInfo.mtimeMs,'Owner config drift must predate final native acceptance');
  const earlierExit=await stat('docs/ui-5/native/exit.json');
  assert.ok(info.mtimeMs>earlierExit.mtimeMs,'Observed owner write occurred after the earlier isolated app exited');
  const ownerConfig=await json(file.path);
  assert.notDeepEqual(ownerConfig.appearance,config.appearance,'Owner appearance must remain distinct from the isolated fixture');
  ownerObservations.at(-1).observation='Hash changed before final native session. Current bytes preserved; cause not observed. Native writes resolve only the separately verified isolated root. Blanket owner-config byte identity is not claimed.';
  ownerObservations.at(-1).lastWriteUtc=info.mtime.toISOString();
  ownerObservations.at(-1).earlierIsolatedExitUtc=earlierExit.mtime.toISOString();
  ownerObservations.at(-1).finalIsolatedStartUtc=sessionInfo.mtime.toISOString();
 }
}
await writeFile('docs/ui-5/owner-observations.json',JSON.stringify(ownerObservations,null,2));
const initialLayout=(await json('docs/ui-5/native-final/overview.json')).preferences.widgets;
const expected=structuredClone(initialLayout);expected.widgets.find(w=>w.id==='instance-details').enabled=false;
assert.deepEqual(after.preferences.widgets,expected,'Hiding changes only one visibility flag');
const medium=await json('docs/ui-5/native-final/medium-proof.json');
const minimum=await json('docs/ui-5/native-final/minimum-proof.json');
assert.deepEqual(medium.viewport,[1024,768]);assert.deepEqual(minimum.viewport,[712,512]);
await writeFile('docs/ui-5/native-verification.json',JSON.stringify({
 restartPreferencesIdentical:true,widgetOrderSizeAndOtherFlagsPreserved:true,configSchema:7,
 selectedInstance:null,discordAllPreferencesOff:true,ownerHashesUnchanged:ownerObservations.filter(f=>f.unchanged).length,ownerConfigDriftObserved:ownerObservations.some(f=>!f.unchanged),
 medium,minimum,firstExit:await json('docs/ui-5/native-final/exit.json'),restartExit:await json('docs/ui-5/native-persistence/exit.json'),
 ui4:{verdict:'PARTIAL',reason:'No full-body character or populated library scrolling verified in this isolated UI-5 session. UI-5 Settings sizing does not grant UI-4 acceptance.'},
 excludedCaptures:['native-final/medium-updates','native-final/medium-1024x768'],
 excludedReason:'Browser window-bounds API resized/repositioned the embedded WebView, not its native host; not valid native host-size acceptance.'
},null,2));
console.log('PASS native restart, isolated config, widget preservation and native sizes; owner hashes:',ownerObservations.filter(f=>f.unchanged).length,'unchanged; pre-session config drift preserved and reported');
