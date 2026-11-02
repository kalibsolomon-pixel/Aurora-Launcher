// Protected native publication adapter over the existing release/authority
// protocol. Never used by the application. No credential fallback for authority.
import {readFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {repository,sha256} from './native-signing.mjs';
import {authorityToken,blobSha,emptyTreeSha} from './native-git.mjs';
import {verifyNativeDirectory,nativeAdvancement,nativeAuthority} from './native-update.mjs';

export const nativeRef='refs/heads/launcher-native-update-authority';
const refPath='/git/ref/heads/launcher-native-update-authority';
const updatePath='/git/refs/heads/launcher-native-update-authority';
export const nativeInitialization='Initialize Aurora native preview update authority';
const requireSha=sha=>{if(!/^[a-f0-9]{40}$/.test(sha??'')) throw Error('Invalid exact Git identity'); return sha;};
export function releaseIdentity(release,version,sourceSha,requirePublic=false) {
  if (!release || release.tag_name!==`native-v${version}` || release.target_commitish!==requireSha(sourceSha) ||
      release.prerelease!==false || (requirePublic && (release.draft || release.immutable!==true))) throw Error('Native release identity/immutability mismatch');
}
async function verifyNativeTag(api,version,sourceSha) {
  const tag=await api(`/git/ref/tags/native-v${version}`);
  if(tag?.ref!==`refs/tags/native-v${version}` || tag.object?.type!=='commit' || tag.object.sha!==requireSha(sourceSha)) throw Error('Exact native tag commit differs');
}
export async function verifyPublicNative({api,download,metadata,directory}) {
  const release=await api(`/releases/tags/native-v${metadata.version}`);
  releaseIdentity(release,metadata.version,metadata.sourceSha,true);
  await verifyNativeTag(api,metadata.version,metadata.sourceSha);
  const bytes=await readFile(join(directory,'native-release-assets.json'));
  const expected=[...metadata.artifacts,{name:'native-release-assets.json',sizeBytes:bytes.length,sha256:sha256(bytes)}];
  if (!Array.isArray(release.assets) || release.assets.length!==expected.length) throw Error('Native public inventory differs');
  for(const entry of expected) {
    const matches=release.assets.filter(a=>a.name===entry.name);
    const url=`https://github.com/${repository}/releases/download/native-v${metadata.version}/${entry.name}`;
    if(matches.length!==1 || matches[0].size!==entry.sizeBytes || matches[0].browser_download_url!==url) throw Error('Native public asset identity differs');
    const publicBytes=await download(url,entry.sizeBytes);
    if(publicBytes.length!==entry.sizeBytes || sha256(publicBytes)!==entry.sha256) throw Error('Native public bytes differ');
  }
  return release;
}
export async function publishNativeRelease({api,upload,download,metadata,directory,releaseNotes}) {
  if(await api(`/releases/tags/native-v${metadata.version}`,{allowMissing:true})) throw Error('Native tag/release already exists; inspect and recover manually, never overwrite');
  if(await api(`/git/ref/tags/native-v${metadata.version}`,{allowMissing:true})) throw Error('Native tag already exists');
  // Atomic tag creation refuses a concurrent existing tag instead of attaching
  // the release to someone else's ref. Never move or force an existing tag.
  await api('/git/refs',{method:'POST',body:{ref:`refs/tags/native-v${metadata.version}`,sha:metadata.sourceSha}});
  await verifyNativeTag(api,metadata.version,metadata.sourceSha);
  const release=await api('/releases',{method:'POST',body:{tag_name:`native-v${metadata.version}`,target_commitish:metadata.sourceSha,name:`Aurora Launcher Native ${metadata.version}`,body:`${releaseNotes??'Native Rust/Slint release. Data and update authority are isolated from the WebView launcher.'}\n\nSource: ${metadata.sourceSha}.`,draft:true,prerelease:false,make_latest:'true'}});
  releaseIdentity(release,metadata.version,metadata.sourceSha);
  for(const name of [...metadata.artifacts.map(a=>a.name),'native-release-assets.json']) await upload(release.id,name,await readFile(join(directory,name)));
  // Verify uploaded draft bytes before the irreversible publication step.
  const draft=await api(`/releases/${release.id}`);
  const expected=[...metadata.artifacts];
  const inventory=await readFile(join(directory,'native-release-assets.json'));
  expected.push({name:'native-release-assets.json',sizeBytes:inventory.length,sha256:sha256(inventory)});
  if(draft.assets.length!==expected.length) throw Error('Native draft inventory differs');
  for(const entry of expected) {
    const assets=draft.assets.filter(a=>a.name===entry.name);
    if(assets.length!==1 || assets[0].size!==entry.sizeBytes) throw Error('Native draft asset differs');
    const bytes=await download(`/releases/assets/${assets[0].id}`,entry.sizeBytes,true);
    if(sha256(bytes)!==entry.sha256) throw Error('Native draft hash mismatch');
  }
  await verifyNativeTag(api,metadata.version,metadata.sourceSha);
  await api(`/releases/${release.id}`,{method:'PATCH',body:{draft:false,make_latest:'true'}});
  return verifyPublicNative({api,download,metadata,directory});
}
async function inspectAuthority(api,sha,key) {
  const commit=await api(`/git/commits/${requireSha(sha)}`);
  if(commit.sha!==sha || !Array.isArray(commit.parents) || commit.parents.length>1) throw Error('Unexpected native authority history');
  if(!commit.parents.length) {
    if(commit.tree.sha!==emptyTreeSha || commit.message!==nativeInitialization) throw Error('Owner must initialize the isolated native authority');
    return null;
  }
  const tree=await api(`/git/trees/${requireSha(commit.tree.sha)}`);
  const entry=tree.tree?.[0];
  if(tree.sha!==commit.tree.sha || tree.truncated || tree.tree.length!==1 || entry.path!=='native-update.json' || entry.type!=='blob' || entry.mode!=='100644') throw Error('Unexpected native authority files');
  const blob=await api(`/git/blobs/${requireSha(entry.sha)}`);
  const bytes=Buffer.from(blob.content??'','base64');
  if(blob.sha!==entry.sha || blob.encoding!=='base64' || bytes.length!==blob.size || bytes.length>65536 || blobSha(bytes)!==entry.sha) throw Error('Native authority blob differs');
  nativeAdvancement(bytes,bytes,key);
  return bytes;
}
export async function advanceNativeAuthority(options) {
  const {api,download,metadata,directory,key,expectedParent}=options;
  await verifyPublicNative(options);
  const current=await api(refPath);
  if(current.ref!==nativeRef || current.object?.type!=='commit' || requireSha(current.object.sha)!==requireSha(expectedParent)) throw Error('Native authority parent moved or is uninitialized');
  const previous=await inspectAuthority(api,expectedParent,key);
  const bytes=await readFile(join(directory,'native-update.json'));
  const unchanged=nativeAdvancement(previous,bytes,key)==='unchanged';
  const verifyRaw=async(sha)=>{
    for(let attempt=0;attempt<7;attempt++) {
      if((await api(refPath)).object.sha!==sha)throw Error('Native authority moved during public verification');
      let publicBytes;
      try {publicBytes=await download(nativeAuthority,bytes.length);} catch { /* bounded read-only CDN recovery */ }
      if(publicBytes?.equals(bytes)) {console.log(`VERIFIED ${sha}`);return;}
      if(attempt<6)await (options.wait??(ms=>new Promise(done=>setTimeout(done,ms))))(60000);
    }
    throw Error('Native authority PUBLISHED but raw bytes not VERIFIED; retain ref and inspect CDN');
  };
  if(unchanged){await verifyRaw(expectedParent);return 'unchanged';}
  const blob=await api('/git/blobs',{method:'POST',body:{content:bytes.toString('base64'),encoding:'base64'}});
  if(blob.sha!==blobSha(bytes)) throw Error('Native prepared blob differs');
  const tree=await api('/git/trees',{method:'POST',body:{tree:[{path:'native-update.json',mode:'100644',type:'blob',sha:blob.sha}]}});
  const commit=await api('/git/commits',{method:'POST',body:{message:`Publish Aurora native preview ${metadata.version}`,tree:requireSha(tree.sha),parents:[expectedParent]}});
  const prepared=await inspectAuthority(api,requireSha(commit.sha),key);
  if(!prepared?.equals(bytes) || (await api(refPath)).object.sha!==expectedParent) throw Error('Native prepared authority differs or parent moved');
  console.log(`PREPARED ${commit.sha}`);
  // Non-forced fast-forward only. Protected ref + dedicated App + workflow
  // concurrency exclude other writers. An uncertain write is inspected, never retried.
  try {await api(updatePath,{method:'PATCH',body:{sha:commit.sha,force:false}});}
  catch {if((await api(refPath)).object.sha!==commit.sha) throw Error('Native authority outcome requires manual inspection');}
  if((await api(refPath)).object.sha!==commit.sha) throw Error('Native authority moved');
  console.log(`PUBLISHED ${commit.sha}`);
  await verifyRaw(commit.sha);
  return commit.sha;
}
async function main() {
  const [mode,directoryArg,keyFile,version,sourceSha,expectedParent]=process.argv.slice(2);
  if(!['publish','advance'].includes(mode) || !directoryArg || process.env.GITHUB_REPOSITORY!==repository || process.env.GITHUB_REF!=='refs/heads/main' || process.env.GITHUB_EVENT_NAME!=='workflow_dispatch' || process.env.GITHUB_ACTIONS!=='true') throw Error('Protected manual native release workflow required');
  const directory=resolve(directoryArg),key=(await readFile(keyFile,'utf8')).trim();
  const metadata=await verifyNativeDirectory(directory,key,version,sourceSha);
  if(JSON.stringify(JSON.parse(await readFile(join(directory,'native-release-assets.json'),'utf8')))!==JSON.stringify(metadata)) throw Error('Native inventory differs');
  const token=mode==='advance'?authorityToken(process.env):process.env.GH_TOKEN;
  if(!token) throw Error('Protected credential missing');
  const api=async(path,{method='GET',body,allowMissing=false}={})=>{
    const r=await fetch(`https://api.github.com/repos/${repository}${path}`,{method,body:body?JSON.stringify(body):undefined,redirect:'error',signal:AbortSignal.timeout(30000),headers:{Authorization:`Bearer ${token}`,Accept:'application/vnd.github+json','Content-Type':'application/json','X-GitHub-Api-Version':'2022-11-28'}});
    if(r.status===404 && allowMissing) return null;
    if(!r.ok) throw Error(`Native API HTTP ${r.status}`);
    return r.json();
  };
  const bounded=async(r,size)=>{
    if(!r.ok) throw Error(`Native download HTTP ${r.status}`);
    const chunks=[];let count=0;
    for await(const chunk of r.body){count+=chunk.length;if(count>size)throw Error('Native download bound exceeded');chunks.push(chunk);}
    const bytes=Buffer.concat(chunks);if(bytes.length!==size)throw Error('Native download incomplete');return bytes;
  };
  const download=async(url,size,privateAsset=false)=>{
    if(privateAsset) return bounded(await fetch(`https://api.github.com/repos/${repository}${url}`,{redirect:'follow',signal:AbortSignal.timeout(300000),headers:{Authorization:`Bearer ${token}`,Accept:'application/octet-stream'}}),size);
    return bounded(await fetch(url,{redirect:url===nativeAuthority?'error':'follow',signal:AbortSignal.timeout(300000),headers:{'Cache-Control':'no-cache'}}),size);
  };
  const upload=async(id,name,bytes)=>{
    const r=await fetch(`https://uploads.github.com/repos/${repository}/releases/${id}/assets?name=${encodeURIComponent(name)}`,{method:'POST',redirect:'error',signal:AbortSignal.timeout(300000),headers:{Authorization:`Bearer ${token}`,'Content-Type':'application/octet-stream'},body:bytes});
    if(!r.ok)throw Error(`Native upload HTTP ${r.status}`);
  };
  const releaseNotes=mode==='publish'?await readFile(new URL(`../../docs/releases/NATIVE_${version.replaceAll('.','_')}_NOTES.md`,import.meta.url),'utf8'):undefined;
  const options={api,download,upload,metadata,directory,key,expectedParent,releaseNotes};
  if(mode==='publish')await publishNativeRelease(options);else await advanceNativeAuthority(options);
}
if(process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) main().catch(error=>{console.error(error.message);process.exitCode=1;});
