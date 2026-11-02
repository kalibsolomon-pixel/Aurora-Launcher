import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdtemp} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {createHash,generateKeyPairSync,sign} from 'node:crypto';
import {createNativePayload,createNativeEnvelope,verifyNativeEnvelope,verifyNativeDirectory,nativeAdvancement,validateNativePayload,nativeName,nativeAuthority} from './native-update.mjs';
import {publishNativeRelease,advanceNativeAuthority,nativeRef,nativeInitialization,releaseIdentity} from './native-publish.mjs';
import {blobSha,emptyTreeSha,authorityToken} from './native-git.mjs';
import {sha256} from './native-signing.mjs';

function fixture(version='1.4.3', shared) {
  const pair=shared??generateKeyPairSync('ed25519'),id=Buffer.from('release!');
  const key=Buffer.from(`untrusted comment: TEST ONLY\n${Buffer.concat([Buffer.from('Ed'),id,pair.publicKey.export({format:'der',type:'spki'}).subarray(-32)]).toString('base64')}\n`).toString('base64');
  const signature=(bytes,name)=>{const sig=sign(null,createHash('blake2b512').update(bytes).digest(),pair.privateKey),comment=`file:${name}`;return Buffer.from(`untrusted comment: TEST ONLY\n${Buffer.concat([Buffer.from('ED'),id,sig]).toString('base64')}\ntrusted comment: ${comment}\n${sign(null,Buffer.concat([sig,Buffer.from(comment)]),pair.privateKey).toString('base64')}\n`).toString('base64');};
  const bytes=Buffer.from('Harmless signed fixture, not an executable.');
  const payload=createNativePayload(bytes,signature(bytes,nativeName(version)),key,version,'1.4.2','a'.repeat(40));
  const payloadBytes=Buffer.from(JSON.stringify(payload)),payloadSig=signature(payloadBytes,'native-update-payload.json');
  const envelope=Buffer.from(JSON.stringify(createNativeEnvelope(payloadBytes,payloadSig,key)));
  return {pair,key,bytes,payload,payloadBytes,payloadSig,envelope,signature,version};
}
async function directory(f) {
  const root=await mkdtemp(join(tmpdir(),'aurora-native-release-tools-'));
  for(const [name,bytes] of [[nativeName(f.version),f.bytes],[nativeName(f.version)+'.sig',f.payload.installerSignature],['native-update-payload.json',f.payloadBytes],['native-update-payload.json.sig',f.payloadSig],['native-update.json',f.envelope]]) await writeFile(join(root,name),bytes);
  const metadata=await verifyNativeDirectory(root,f.key,f.version,'a'.repeat(40));
  await writeFile(join(root,'native-release-assets.json'),JSON.stringify(metadata));
  return {directory:root,metadata};
}

test('native signed inventory rejects channel, unknown fields, malformed bounds, wrong key, hash and source',async()=>{
  const f=fixture(),d=await directory(f);
  assert.equal((await verifyNativeDirectory(d.directory,f.key,f.version,'a'.repeat(40))).version,f.version);
  assert.throws(()=>verifyNativeEnvelope(f.envelope,fixture().key));
  for(const patch of [{channel:'stable'},{runtime:'webview'},{architecture:'aarch64'},{product:'com.aurora.launcher'},{packageType:'msi'},{minimumInstalledVersion:'9.0.0'},{extra:true},{url:f.payload.url+'?x'},{sizeBytes:0},{sha256:'bad'}]) {
    const bytes=Buffer.from(JSON.stringify({...f.payload,...patch}));
    assert.throws(()=>verifyNativeEnvelope(Buffer.from(JSON.stringify({payload:bytes.toString('base64'),signature:f.signature(bytes,'native-update-payload.json')})),f.key));
  }
  await assert.rejects(verifyNativeDirectory(d.directory,f.key,f.version,'b'.repeat(40)));
  await writeFile(join(d.directory,nativeName(f.version)),Buffer.from('corrupt'));
  await assert.rejects(verifyNativeDirectory(d.directory,f.key,f.version,'a'.repeat(40)));
  assert.throws(()=>validateNativePayload({...f.payload,unknown:true}));
});
test('native authority never downgrades or accepts same-version drift',()=>{
  const a=fixture(),b=fixture('1.4.4',a.pair);
  assert.equal(nativeAdvancement(null,a.envelope,a.key),'advance');
  assert.equal(nativeAdvancement(a.envelope,a.envelope,a.key),'unchanged');
  assert.equal(nativeAdvancement(a.envelope,b.envelope,a.key),'advance');
  assert.throws(()=>nativeAdvancement(b.envelope,a.envelope,a.key));
  const payload=Buffer.from(JSON.stringify({...a.payload,sourceSha:'b'.repeat(40)}));
  const drift=Buffer.from(JSON.stringify(createNativeEnvelope(payload,a.signature(payload,'native-update-payload.json'),a.key)));
  assert.throws(()=>nativeAdvancement(a.envelope,drift,a.key));
  assert.throws(()=>authorityToken({GH_TOKEN:'ordinary',AURORA_AUTHORITY_TOKEN:'ordinary'}));
});
test('native publication uploads verified draft bytes and selects native as latest stable',async()=>{
  const f=fixture('2.0.0'),d=await directory(f),uploaded=new Map(),mutations=[];
  const release={id:42,tag_name:`native-v${f.version}`,target_commitish:d.metadata.sourceSha,prerelease:false,draft:true,immutable:false,assets:[]};
  const api=async(path,options={})=>{
    if(options.method==='POST' && path==='/git/refs'){assert.deepEqual(options.body,{ref:`refs/tags/native-v${f.version}`,sha:d.metadata.sourceSha});mutations.push(path);return {};}
    if(options.method==='POST'){assert.equal(options.body.name,'Aurora Launcher Native 2.0.0');assert.equal(options.body.make_latest,'true');assert.equal(options.body.draft,true);mutations.push(path);return {...release};}
    if(options.method==='PATCH'){assert.equal(options.body.make_latest,'true');release.draft=false;release.immutable=true;mutations.push(path);return {...release};}
    if(options.allowMissing)return null;
    if(path===`/git/ref/tags/native-v${f.version}`)return {ref:`refs/tags/native-v${f.version}`,object:{type:'commit',sha:d.metadata.sourceSha}};
    if(path==='/releases/42')return {...release};
    if(path===`/releases/tags/native-v${f.version}`)return {...release};
    throw Error(`Unexpected path ${path}`);
  };
  const upload=async(id,name,bytes)=>{assert.equal(id,42);uploaded.set(name,bytes);release.assets.push({id:release.assets.length+1,name,size:bytes.length,browser_download_url:`https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v${f.version}/${name}`});};
  const download=async(url,size,privateAsset)=>{const name=privateAsset?release.assets.find(a=>url.endsWith('/'+a.id)).name:url.split('/').at(-1);const bytes=uploaded.get(name);assert.equal(bytes.length,size);return bytes;};
  await publishNativeRelease({...d,api,upload,download});
  assert.deepEqual(mutations,['/git/refs','/releases','/releases/42']);
  assert.equal(uploaded.size,6);
  assert.throws(()=>releaseIdentity({...release,prerelease:true},f.version,d.metadata.sourceSha,true));
  assert.throws(()=>releaseIdentity({...release,immutable:false},f.version,d.metadata.sourceSha,true));
  const wrongDownload=async()=>Buffer.from('wrong');
  release.draft=true;release.immutable=false;release.assets=[];uploaded.clear();
  const before=mutations.length;
  await assert.rejects(publishNativeRelease({...d,api,upload,download:wrongDownload}));
  assert.equal(mutations.length,before+2,'hash failure must leave a draft, never publish');
  const wrongTagApi=async(path,options={})=>path===`/git/ref/tags/native-v${f.version}` && !options.allowMissing?{ref:`refs/tags/native-v${f.version}`,object:{type:'commit',sha:'f'.repeat(40)}}:api(path,options);
  const beforeWrongTag=mutations.length;
  await assert.rejects(publishNativeRelease({...d,api:wrongTagApi,upload,download}));
  assert.equal(mutations.length,beforeWrongTag+1,'tag mismatch must stop before draft creation');
});
test('native authority verifies public artifacts then advances only isolated ref, with stale-parent refusal',async()=>{
  const f=fixture(),d=await directory(f),parent='b'.repeat(40),commit='c'.repeat(40),tree='d'.repeat(40),blob=blobSha(f.envelope);
  let current=parent;const writes=[];
  const assets=[];const publicBytes=new Map();
  for(const name of [...d.metadata.artifacts.map(a=>a.name),'native-release-assets.json']){const bytes=await readFile(join(d.directory,name));publicBytes.set(name,bytes);assets.push({name,size:bytes.length,browser_download_url:`https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v${f.version}/${name}`});}
  const api=async(path,options={})=>{
    if(path.startsWith('/releases/tags/'))return {tag_name:`native-v${f.version}`,target_commitish:d.metadata.sourceSha,prerelease:false,draft:false,immutable:true,assets};
    if(path===`/git/ref/tags/native-v${f.version}`)return {ref:`refs/tags/native-v${f.version}`,object:{type:'commit',sha:d.metadata.sourceSha}};
    if(path==='/git/ref/heads/launcher-native-update-authority')return {ref:nativeRef,object:{type:'commit',sha:current}};
    if(options.method){writes.push(path);if(path==='/git/blobs')return {sha:blob};if(path==='/git/trees')return {sha:tree};if(path==='/git/commits')return {sha:commit};if(path==='/git/refs/heads/launcher-native-update-authority'){assert.equal(options.body.force,false);current=commit;return {};}}
    if(path===`/git/commits/${parent}`)return {sha:parent,tree:{sha:emptyTreeSha},parents:[],message:nativeInitialization};
    if(path===`/git/commits/${commit}`)return {sha:commit,tree:{sha:tree},parents:[{sha:parent}]};
    if(path===`/git/trees/${tree}`)return {sha:tree,truncated:false,tree:[{path:'native-update.json',type:'blob',mode:'100644',sha:blob}]};
    if(path===`/git/blobs/${blob}`)return {sha:blob,encoding:'base64',size:f.envelope.length,content:f.envelope.toString('base64')};
    throw Error(`Unexpected path ${path}`);
  };
  const download=async(url)=>url===nativeAuthority?f.envelope:publicBytes.get(url.split('/').at(-1));
  const options={...d,api,download,key:f.key,expectedParent:'e'.repeat(40)};
  await assert.rejects(advanceNativeAuthority(options));assert.equal(writes.length,0);
  await advanceNativeAuthority({...options,expectedParent:parent});
  assert.equal(current,commit);assert.equal(writes.at(-1),'/git/refs/heads/launcher-native-update-authority');
  assert.ok(writes.every(p=>!p.includes('/heads/launcher-update-authority')));
});
test('native protected workflow is manual, scoped and signing fails closed',async()=>{
  const text=await readFile(new URL('../../.github/workflows/native-release.yml',import.meta.url),'utf8');
  assert.match(text,/workflow_dispatch:/);assert.doesNotMatch(text,/pull_request:|push:|schedule:|workflow_run:/);
  for(const name of ['launcher-production','launcher-native-preview-publication','launcher-update-authority-production'])assert.ok(text.includes(name));
  assert.match(text,/if: inputs.publish && inputs.advance_authority/);
  const advance=text.split('\n  advance:')[1];
  assert.doesNotMatch(advance,/GH_TOKEN:|GITHUB_TOKEN:|TAURI_SIGNING_PRIVATE_KEY:/);
  assert.match(advance,/AURORA_AUTHORITY_TOKEN:/);
  assert.match(text,/native-update.mjs verify/);
});
