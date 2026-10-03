import test from "node:test";
import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { mkdtemp, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { expectedAssetName } from "./release-state.mjs";
import { installerName, publicKeyPacket, verifyUpdaterSignature, createManifest, publicationDecision, sha256, verifyDirectory, repository } from "./updater-release.mjs";
import { publishManifest, authorityRef, authorityUrl, initializationMessage, blobSha } from "./publish-manifest.mjs";

// DIAGNOSTIC unit keys exist only in memory, never persisted or supplied to a production build.
const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const keyId = Buffer.from("0102030405060708", "hex");
const rawPublic = publicKey.export({ type: "spki", format: "der" }).subarray(-32);
const encodedPublic = Buffer.from(`untrusted comment: diagnostic unit key\n${Buffer.concat([Buffer.from("Ed"), keyId, rawPublic]).toString("base64")}\n`).toString("base64");
const version = "1.4.0", sourceSha = "a".repeat(40), name = installerName(version, "nsis");
const bytes = Buffer.from("DIAGNOSTIC installer bytes: not executable, not a release");
const notes = "# Aurora Launcher 1.4.0\n\nAurora Client 2.1.5 remains unchanged.";
function signature(comment = `timestamp:1\tfile:${name}\tversion:${version}`) {
  const signature = sign(null, createHash("blake2b512").update(bytes).digest(), privateKey);
  const packet = Buffer.concat([Buffer.from("ED"), keyId, signature]);
  const global = sign(null, Buffer.concat([signature, Buffer.from(comment)]), privateKey);
  return Buffer.from(`untrusted comment: diagnostic unit signature\n${packet.toString("base64")}\ntrusted comment: ${comment}\n${global.toString("base64")}\n`).toString("base64");
}
const encodedSignature = signature();
test("verifies exact prehashed installer and authenticated identity", () => verifyUpdaterSignature(bytes, encodedSignature, encodedPublic, version, name));
test("rejects modified installer bytes", () => assert.throws(() => verifyUpdaterSignature(Buffer.from("tampered"), encodedSignature, encodedPublic, version, name)));
test("rejects trusted-comment tampering", () => {
  const changed = Buffer.from(Buffer.from(encodedSignature, "base64").toString().replace("version:1.4.0", "version:9.9.9")).toString("base64");
  assert.throws(() => verifyUpdaterSignature(bytes, changed, encodedPublic, version, name));
});
test("rejects different public key and malformed signing inputs", () => {
  assert.throws(() => publicKeyPacket("C:/keys/public.pub"));
  assert.throws(() => verifyUpdaterSignature(bytes, "invalid", encodedPublic, version, name));
  const changed = Buffer.from(`untrusted comment: diagnostic\n${Buffer.concat([Buffer.from("Ed"), Buffer.alloc(8), rawPublic]).toString("base64")}\n`).toString("base64");
  assert.throws(() => verifyUpdaterSignature(bytes, encodedSignature, changed, version, name));
});
test("rejects mismatched signed filename/version", () => {
  assert.throws(() => verifyUpdaterSignature(bytes, signature(`file:foreign.exe\tversion:${version}`), encodedPublic, version, name));
  assert.throws(() => verifyUpdaterSignature(bytes, signature(`file:${name}\tversion:1.3.1`), encodedPublic, version, name));
});
test("supports current locked CLI filename-only authenticated comment", () => verifyUpdaterSignature(bytes, signature(`timestamp:1\tfile:${name}`), encodedPublic, version, name));
test("one static manifest derives exact NSIS public URL/signature", () => {
  const manifest = createManifest(version, notes, encodedSignature);
  assert.deepEqual(Object.keys(manifest.platforms), ["windows-x86_64"]);
  assert.equal(manifest.platforms["windows-x86_64"].url, `https://github.com/${repository}/releases/download/v1.4.0/Aurora.Launcher_1.4.0_x64-setup.exe`);
  assert.equal(manifest.platforms["windows-x86_64"].signature, encodedSignature);
});
test("rejects stale notes, development versions and unsafe Windows versions", () => {
  assert.throws(() => createManifest(version, "# Aurora Launcher 1.3.1\nAurora Client 2.1.5", encodedSignature));
  for (const value of ["1.4.0-diag.1", "01.4.0", "256.0.0", "1.0.65536"]) assert.throws(() => installerName(value, "nsis"));
});
test("identical manifest is a no-op; downgrade and same-version drift refuse", () => {
  const current = Buffer.from(JSON.stringify(createManifest(version, notes, encodedSignature)));
  assert.equal(publicationDecision(current, current), "unchanged");
  assert.throws(() => publicationDecision(current, Buffer.from(JSON.stringify({ ...JSON.parse(current), notes: notes + "changed" }))));
  const older = Buffer.from(JSON.stringify(createManifest("1.3.1", "# Aurora Launcher 1.3.1\nAurora Client 2.1.5", encodedSignature)));
  assert.throws(() => publicationDecision(current, older));
  assert.equal(publicationDecision(older, current), "replace");
});
function fixture(previous = null, candidateVersion = version) {
  const candidateName = installerName(candidateVersion, "nsis");
  const candidateSignature = signature(`timestamp:1\tfile:${candidateName}\tversion:${candidateVersion}`);
  const sigBytes = Buffer.from(candidateSignature + "\n");
  const artifact = { name: candidateName, format: "nsis", architecture: "x64", sizeBytes: bytes.length, sha256: sha256(bytes) };
  const sig = { name: candidateName + ".sig", sizeBytes: sigBytes.length, sha256: sha256(sigBytes) };
  const metadata = { version: candidateVersion, sourceSha, artifacts: [artifact], updater: { platform: "windows-x86_64", publicKeySha256: sha256(Buffer.from(encodedPublic)), signatures: [sig] } };
  const metadataBytes = Buffer.from(JSON.stringify(metadata));
  const manifestBytes = Buffer.from(JSON.stringify(createManifest(candidateVersion, `# Aurora Launcher ${candidateVersion}\nAurora Client 2.1.5`, candidateSignature)));
  const publicBytes = new Map([[candidateName, bytes], [sig.name, sigBytes], ["release-assets.json", metadataBytes]]);
  const release = { id: 10, tag_name: `v${candidateVersion}`, target_commitish: sourceSha, draft: false, prerelease: false, immutable: true,
    assets: [...publicBytes].map(([file, content], index) => ({ id: index + 1, name: file.replaceAll(" ", "."), size: content.length, browser_download_url: `https://github.com/${repository}/releases/download/v${candidateVersion}/${file.replaceAll(" ", ".")}` })) };
  const objects = new Map(), calls = [], states = [], waits = [];
  let counter = 10, head = "1".repeat(40), rawQueue = [];
  const identity = () => (++counter).toString(16).padStart(40, "0");
  function addBlob(content) { const sha = blobSha(content); objects.set('/git/blobs/'+sha, {sha, encoding:'base64', size:content.length, content:content.toString('base64')}); return sha; }
  function addCommit(content, parent, message) {
    const treeSha = identity(); objects.set('/git/trees/'+treeSha, {sha:treeSha, truncated:false, tree:content ? [{path:'launcher-update.json',mode:'100644',type:'blob',sha:addBlob(content)}] : []});
    const sha = identity(); objects.set('/git/commits/'+sha, {sha, message, tree:{sha:treeSha}, parents:parent ? [{sha:parent}] : []});return sha;
  }
  head = addCommit(null, null, initializationMessage);
  const root = head;
  if (previous) head = addCommit(previous, root, 'Prior accepted publication');
  const initialHead = head;
  const contentAtHead = () => {
    const commit = objects.get('/git/commits/'+head); const tree = objects.get('/git/trees/'+commit.tree.sha); const entry=tree.tree[0];
    return entry ? Buffer.from(objects.get('/git/blobs/'+entry.sha).content,'base64') : null;
  };
  const api = async (path, options = {}) => {
    calls.push([options.method ?? 'GET', path, options.body]);
    if (path === `/releases/tags/v${candidateVersion}` || path === '/releases/latest') return release;
    if (path === '/git/ref/heads/launcher-update-authority') return {ref:authorityRef,object:{type:'commit',sha:head}};
    if (options.method === 'POST') {
      if (path === '/git/blobs') return {sha:addBlob(Buffer.from(options.body.content,'base64'))};
      if (path === '/git/trees') { const sha=identity();objects.set('/git/trees/'+sha,{sha,truncated:false,tree:options.body.tree});return {sha}; }
      if (path === '/git/commits') { const sha=identity();objects.set('/git/commits/'+sha,{sha,message:options.body.message,tree:{sha:options.body.tree},parents:options.body.parents.map(sha=>({sha}))});return {sha}; }
      throw Error('Unexpected mutation');
    }
    if (options.method === 'PATCH') {
      assert.equal(path,'/git/refs/heads/launcher-update-authority');assert.equal(options.body.force,false);
      if (objects.get('/git/commits/'+options.body.sha).parents[0].sha !== head) throw Error('Non-fast-forward');
      head=options.body.sha;return {ref:authorityRef,object:{type:'commit',sha:head}};
    }
    if (objects.has(path)) return structuredClone(objects.get(path));
    throw Error('Unexpected/missing diagnostic API path '+path);
  };
  const download = async (url) => {
    calls.push(['download',url]);
    if (url === authorityUrl) { if(rawQueue.length){const next=rawQueue.shift();if(next instanceof Error)throw next;return next;}return contentAtHead(); }
    return publicBytes.get(url.split('/').at(-1).replace('Aurora.Launcher','Aurora Launcher'));
  };
  return {version:candidateVersion,sourceSha,metadata,metadataBytes,manifestBytes,publicKey:encodedPublic,api,download,calls,objects,release,publicBytes,states,waits,
    publicAttempts:3,wait:async(ms)=>waits.push(ms),report:state=>states.push(state),initialHead,root,
    head:()=>head,setHead:sha=>{head=sha;},raw:queue=>{rawQueue=queue;},contentAtHead,addCommit};
}
const mutateCalls = v => v.calls.filter(([method])=>['POST','PATCH','DELETE'].includes(method));
const patchCalls = v => v.calls.filter(([method])=>method==='PATCH');
const previousManifest = (v) => fixture(null,v).manifestBytes;

test('explicit empty root supports first publication; PREPARED PUBLISHED VERIFIED; discovery mutation LAST',async()=>{
  const v=fixture();assert.equal(await publishManifest(v),'published');assert.deepEqual(v.states.map(s=>s.state),['PREPARED','PUBLISHED','VERIFIED']);
  assert.ok(v.contentAtHead().equals(v.manifestBytes));assert.equal(mutateCalls(v).at(-1)[0],'PATCH');
  const firstMutation=v.calls.findIndex(([m])=>m==='POST');assert.equal(v.calls.slice(0,firstMutation).filter(([m])=>m==='download').length,3);
  assert.equal(patchCalls(v).length,1);assert.equal(v.calls.some(([,p])=>p.includes('launcher-updates')),false);
});
test('future 1.4.1 and 1.4.2 advance from observed parent; immutable release untouched',async()=>{
  for(const [old,next] of [['1.4.0','1.4.1'],['1.4.1','1.4.2']]){const v=fixture(previousManifest(old),next);await publishManifest(v);assert.equal(v.states[0].parent,v.initialHead);assert.ok(mutateCalls(v).every(([,p])=>p.startsWith('/git/')));}
});
test('exact successful rerun is idempotent and still verifies public bytes',async()=>{
  const v=fixture();await publishManifest(v);v.calls.length=0;assert.equal(await publishManifest(v),'unchanged');assert.equal(mutateCalls(v).length,0);
});
for(const [name,prior] of [['same version different content',Buffer.from(previousManifest('1.4.0').toString().replace('Aurora Client 2.1.5','Aurora Client 2.1.5 changed'))],['downgrade',previousManifest('1.4.1')],['malformed current manifest',Buffer.from('{')]]){
 test(name+' fails before constructing objects',async()=>{const v=fixture(prior);await assert.rejects(publishManifest(v));assert.equal(mutateCalls(v).length,0);assert.equal(v.head(),v.initialHead);});
}
for(const problem of ['repository','ref','parent','candidate','channel','url','signature','publicHash','immutable','latest','missingAuthority','extraFile','truncatedTree','initialMessage','blobIdentity']){
 test(problem+' prerequisite fails closed',async()=>{
  const v=fixture();const original=v.api;
  if(problem==='repository')v.repository='foreign/repo';
  if(problem==='ref')v.ref='refs/heads/main';
  if(problem==='parent')v.expectedParent='e'.repeat(40);
  if(problem==='candidate')v.manifestBytes=Buffer.from('{');
  if(['channel','url','signature'].includes(problem)){const m=JSON.parse(v.manifestBytes);if(problem==='channel')m.channel='stable';if(problem==='url')m.platforms['windows-x86_64'].url='https://evil.example/installer.exe';if(problem==='signature')m.platforms['windows-x86_64'].signature=Buffer.from('invalid').toString('base64');v.manifestBytes=Buffer.from(JSON.stringify(m));}
  if(problem==='publicHash')v.publicBytes.set(installerName(version,'nsis'),Buffer.from('tampered'));
  if(problem==='immutable')v.release.immutable=false;
  v.api=async(path,options)=>{
   if(problem==='missingAuthority'&&path.includes('/git/ref/'))throw Error('404');
   if(problem==='latest'&&path==='/releases/latest')return {id:99};
   const r=await original(path,options);
   if(problem==='ref'&&path.includes('/git/ref/'))r.ref='refs/heads/main';
   if(problem==='extraFile'&&path.startsWith('/git/trees/'))r.tree.push({path:'README.md'});
   if(problem==='truncatedTree'&&path.startsWith('/git/trees/'))r.truncated=true;
   if(problem==='initialMessage'&&path.startsWith('/git/commits/'))r.message='Unrelated initial state';
   if(problem==='blobIdentity'&&path.startsWith('/git/commits/'))r.sha='f'.repeat(40);
   return r;
  };
  await assert.rejects(publishManifest(v));assert.equal(mutateCalls(v).length,0);assert.equal(v.head(),v.initialHead);
 });
}
for(const failPath of ['/git/blobs','/git/trees','/git/commits']){
 test(failPath+' construction failure leaves original ref active',async()=>{const v=fixture();const original=v.api;v.api=(p,o)=>{if(p===failPath)throw Error('Diagnostic interruption');return original(p,o);};await assert.rejects(publishManifest(v));assert.equal(v.head(),v.initialHead);assert.equal(patchCalls(v).length,0);});
}
test('prepared wrong parent is rejected before ref change',async()=>{const v=fixture();const original=v.api;v.api=async(p,o)=>{const r=await original(p,o);if(o?.method==='POST'&&p==='/git/commits'){v.objects.get('/git/commits/'+r.sha).parents=[{sha:'f'.repeat(40)}];}return r;};await assert.rejects(publishManifest(v));assert.equal(patchCalls(v).length,0);});
test('concurrent movement before final reread prevents advancement',async()=>{const v=fixture();v.report=s=>{if(s.state==='PREPARED')v.setHead(v.addCommit(previousManifest('1.4.1'),v.initialHead,'Concurrent writer'));};await assert.rejects(publishManifest(v));assert.equal(patchCalls(v).length,0);});
test('concurrent sibling after reread is rejected by non-forced API',async()=>{const v=fixture();const original=v.api;v.api=(p,o)=>{if(o?.method==='PATCH')v.setHead(v.addCommit(previousManifest('1.4.1'),v.initialHead,'Concurrent writer'));return original(p,o);};await assert.rejects(publishManifest(v),/Concurrent/);assert.equal(patchCalls(v).length,1);});
test('ref rejection leaves authority unchanged and never repeats mutation',async()=>{const v=fixture();const original=v.api;let writes=0;v.api=(p,o)=>{if(o?.method==='PATCH'){writes++;throw Error('403');}return original(p,o);};await assert.rejects(publishManifest(v),/unchanged/);assert.equal(v.head(),v.initialHead);assert.equal(writes,1);});
test('ambiguous accepted ref response is recovered through GET and public read',async()=>{const v=fixture();const original=v.api;v.api=async(p,o)=>{const r=await original(p,o);if(o?.method==='PATCH')throw Error('Timeout after acceptance');return r;};assert.equal(await publishManifest(v),'published');assert.equal(patchCalls(v).length,1);});
test('stale cached/temporarily missing response waits then verifies; no repeated mutation',async()=>{const v=fixture(previousManifest('1.3.1'));v.raw([previousManifest('1.3.1'),new Error('503'),v.manifestBytes]);await publishManifest(v);assert.deepEqual(v.waits,[60000,60000]);assert.equal(patchCalls(v).length,1);});
test('post-update verification failure retains new ref; rerun verifies without mutation',async()=>{const v=fixture();v.raw([Buffer.from('stale'),new Error('503'),Buffer.from('stale')]);await assert.rejects(publishManifest(v),/PUBLISHED but/);assert.notEqual(v.head(),v.initialHead);v.calls.length=0;assert.equal(await publishManifest(v),'unchanged');assert.equal(mutateCalls(v).length,0);});
test('public ref movement cannot yield VERIFIED',async()=>{const v=fixture();const original=v.download;v.download=async(u,n)=>{const b=await original(u,n);if(u===authorityUrl)v.setHead(v.addCommit(previousManifest('1.4.1'),v.head(),'Concurrent writer'));return b;};await assert.rejects(publishManifest(v));assert.equal(v.states.some(s=>s.state==='VERIFIED'),false);});
test('transferred signature verification rejects tampering without mutation',async()=>{
 const directory=await mkdtemp(join(tmpdir(),'aurora-release-diagnostic-unit-'));const v=fixture();for(const [f,b]of v.publicBytes)await writeFile(join(directory,f),b);await writeFile(join(directory,'launcher-update.json'),v.manifestBytes);
 await verifyDirectory(directory,version,sourceSha,encodedPublic);await writeFile(join(directory,name),'tampered');await assert.rejects(verifyDirectory(directory,version,sourceSha,encodedPublic));
});
test('workflow boundaries are manual, individually protected and publication precedes authority',async()=>{
 const text=await readFile('.github/workflows/launcher-release.yml','utf8');assert.match(text,/workflow_dispatch:/);assert.equal((text.match(/environment: launcher-production/g)||[]).length,3);assert.match(text,/advance_update_authority:[\s\S]*needs: \[resolve, build, publish\]/);assert.match(text,/cancel-in-progress: false/);
 const frontend=await readFile('src/lib/backend.ts','utf8');assert.doesNotMatch(frontend,/launcherUpdate(?:Download|Install)[\s\S]{0,150}(?:endpoint|pubkey)/);
 const runtime=await readFile('src-tauri/src/updates/launcher.rs','utf8');assert.ok(runtime.includes(authorityUrl));assert.doesNotMatch(runtime,/option_env!\("AURORA_LAUNCHER_MANIFEST_URL"\)/);
});
test('CLI refuses ordinary local invocation without publication environment',()=>{const r=spawnSync(process.execPath,['tools/release/publish-manifest.mjs','1.4.0',sourceSha,'unused'],{env:{...process.env,GITHUB_ACTIONS:'false'},encoding:'utf8'});assert.equal(r.status,1);assert.doesNotMatch(r.stderr,/Bearer/);});

test('cryptographically invalid signature with consistent candidate/hash inventory fails before mutation',async()=>{
 const v=fixture();const lines=Buffer.from(signature(),'base64').toString().trim().split('\n');
 const packet=Buffer.from(lines[1],'base64');packet[10]^=1;lines[1]=packet.toString('base64');
 const wrong=Buffer.from(lines.join('\n')+'\n').toString('base64');
 const sigBytes=Buffer.from(wrong+'\n');const sig=v.metadata.updater.signatures[0];sig.sizeBytes=sigBytes.length;sig.sha256=sha256(sigBytes);
 v.publicBytes.set(sig.name,sigBytes);v.metadataBytes=Buffer.from(JSON.stringify(v.metadata));v.publicBytes.set('release-assets.json',v.metadataBytes);
 for(const a of v.release.assets){if(a.name===expectedAssetName(sig.name))a.size=sigBytes.length;if(a.name==='release-assets.json')a.size=v.metadataBytes.length;}
 const m=JSON.parse(v.manifestBytes);m.platforms['windows-x86_64'].signature=wrong;v.manifestBytes=Buffer.from(JSON.stringify(m));
 await assert.rejects(publishManifest(v),/signature verification failed/);assert.equal(mutateCalls(v).length,0);
});
test('prepared tree containing unexpected files fails before ref advancement',async()=>{const v=fixture();const original=v.api;v.api=async(p,o)=>{const r=await original(p,o);if(p==='/git/trees'&&o?.method==='POST')v.objects.get('/git/trees/'+r.sha).tree.push({path:'extra'});return r;};await assert.rejects(publishManifest(v));assert.equal(patchCalls(v).length,0);});
test('public reads contain no credential header and raw redirects fail closed',async()=>{const source=await readFile('tools/release/publish-manifest.mjs','utf8');const downloader=source.slice(source.indexOf('  const download = async'));assert.doesNotMatch(downloader,/Authorization:|GH_TOKEN/);assert.match(downloader,/redirect: raw \? "error" : "follow"/);assert.match(source,/force: false/);assert.doesNotMatch(source,/force: true|method: "DELETE"|uploads\.github/);});
test('missing owner signing configuration fails without creating output or leaking material',()=>{const r=spawnSync(process.execPath,['tools/release/updater-release.mjs','config','unused-output.json'],{env:{...process.env,AURORA_UPDATER_PUBKEY:'',TAURI_SIGNING_PRIVATE_KEY:'',TAURI_SIGNING_PRIVATE_KEY_PASSWORD:''},encoding:'utf8'});assert.equal(r.status,1);assert.match(r.stderr,/validation failed/);});

test('wrong returned ref identity cannot be silently accepted',async()=>{const v=fixture();const original=v.api;v.api=async(p,o)=>{const r=await original(p,o);if(p.includes('/git/ref/'))r.ref='refs/heads/main';return r;};await assert.rejects(publishManifest(v));assert.equal(mutateCalls(v).length,0);});
test('malformed previous blob bytes are rejected before construction',async()=>{const v=fixture(previousManifest('1.3.1'));const original=v.api;v.api=async(p,o)=>{const r=await original(p,o);if(p.startsWith('/git/blobs/'))r.content=Buffer.from('corrupt').toString('base64');return r;};await assert.rejects(publishManifest(v));assert.equal(mutateCalls(v).length,0);});
test('failed outcome reread remains ambiguous and mutation is never repeated',async()=>{const v=fixture();const original=v.api;let uncertain=false,writes=0;v.api=async(p,o)=>{if(o?.method==='PATCH'){writes++;uncertain=true;throw Error('connection lost');}if(uncertain&&p.includes('/git/ref/'))throw Error('read unavailable');return original(p,o);};await assert.rejects(publishManifest(v));assert.equal(writes,1);assert.equal(v.head(),v.initialHead);});
