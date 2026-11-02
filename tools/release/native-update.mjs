// Local preparation only. Never contacts GitHub, publishes, signs, or accepts a
// private key. Both installer and payload signatures are supplied by the owner.
import { readFile, writeFile, readdir } from 'node:fs/promises';
import { basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyUpdaterSignature, versionParts, sha256, strictUtf8, publicKeyPacket } from './native-signing.mjs';

export const nativeAuthority = 'https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-native-update-authority/native-update.json';
export const nativeName = version => `Aurora.Native.Preview_${version}_x64-setup.exe`;
const compare = (a,b) => versionParts(a).map((n,i)=>n-versionParts(b)[i]).find(n=>n!==0) ?? 0;
function exactKeys(value, keys) {
  if (!value || Array.isArray(value) || Object.keys(value).sort().join() !== [...keys].sort().join()) throw Error('Unexpected native manifest fields');
}
export function validateNativePayload(value) {
  exactKeys(value, ['schemaVersion','product','channel','runtime','platform','architecture','packageType','version','minimumInstalledVersion','sourceSha','url','sizeBytes','sha256','installerSignature']);
  versionParts(value.version); versionParts(value.minimumInstalledVersion);
  if (value.schemaVersion!==1 || value.product!=='com.aurora.launcher.native-preview' || value.channel!=='native-preview' || value.runtime!=='slint' ||
      value.platform!=='windows' || value.architecture!=='x86_64' || value.packageType!=='nsis-per-user' ||
      !/^[a-f0-9]{40}$/.test(value.sourceSha) || !/^[a-f0-9]{64}$/.test(value.sha256) ||
      !Number.isSafeInteger(value.sizeBytes) || value.sizeBytes<1 || value.sizeBytes>256*1024*1024 ||
      compare(value.minimumInstalledVersion,value.version)>0 || typeof value.installerSignature!=='string' ||
      value.url!==`https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v${value.version}/${nativeName(value.version)}`) throw Error('Native package identity/compatibility mismatch');
  return value;
}
export function verifyNativeEnvelope(bytes, publicKey) {
  if (!Buffer.isBuffer(bytes) || !bytes.length || bytes.length>64*1024) throw Error('Invalid native envelope size');
  const envelope=JSON.parse(strictUtf8(bytes)); exactKeys(envelope,['payload','signature']);
  if (typeof envelope.payload!=='string') throw Error('Invalid native payload encoding');
  const payload=Buffer.from(envelope.payload,'base64');
  if (payload.toString('base64')!==envelope.payload || payload.length>32*1024) throw Error('Invalid native payload encoding');
  // The verifier authenticates the bytes before returning any interpreted field.
  // The version read here is only an expected authenticated-comment constraint.
  const value=JSON.parse(strictUtf8(payload));
  verifyUpdaterSignature(payload,envelope.signature,publicKey,value.version,'native-update-payload.json');
  return validateNativePayload(value);
}
export function nativeAdvancement(previousBytes, nextBytes, key) {
  const next=verifyNativeEnvelope(nextBytes,key);
  if (!previousBytes) return 'advance';
  const previous=verifyNativeEnvelope(previousBytes,key);
  if (previousBytes.equals(nextBytes)) return 'unchanged';
  if (compare(next.version,previous.version)<=0) throw Error('Native downgrade or same-version drift refused');
  return 'advance';
}
export async function verifyNativeDirectory(directory, key, version, sourceSha) {
  publicKeyPacket(key);
  const offer=verifyNativeEnvelope(await readFile(join(directory,'native-update.json')),key);
  if (offer.version!==version || offer.sourceSha!==sourceSha) throw Error('Native provenance mismatch');
  const installer=await readFile(join(directory,nativeName(version)));
  if (installer.length!==offer.sizeBytes || sha256(installer)!==offer.sha256) throw Error('Native hash/size mismatch');
  verifyUpdaterSignature(installer,offer.installerSignature,key,version,nativeName(version));
  const payload=await readFile(join(directory,'native-update-payload.json'));
  const sig=(await readFile(join(directory,'native-update-payload.json.sig'),'utf8')).trim();
  const installerSig=(await readFile(join(directory,nativeName(version)+'.sig'),'utf8')).trim();
  const expected=createNativeEnvelope(payload,sig,key);
  const actual=JSON.parse(await readFile(join(directory,'native-update.json'),'utf8'));
  if (JSON.stringify(actual)!==JSON.stringify(expected) || installerSig!==offer.installerSignature) throw Error('Native signed inventory mismatch');
  const files=[nativeName(version),nativeName(version)+'.sig','native-update-payload.json','native-update-payload.json.sig','native-update.json'];
  const inventory=[];
  for (const name of files) { const bytes=await readFile(join(directory,name)); inventory.push({name,sizeBytes:bytes.length,sha256:sha256(bytes)}); }
  const metadata={schemaVersion:1,product:offer.product,channel:offer.channel,version,sourceSha,publicKeySha256:sha256(Buffer.from(key)),artifacts:inventory};
  const existing=await readdir(directory);
  if (existing.some(name=>![...files,'native-release-assets.json'].includes(name))) throw Error('Unexpected native release files');
  return metadata;
}

export function createNativePayload(bytes, signature, publicKey, version, minimum, sourceSha) {
  versionParts(version); versionParts(minimum);
  if (compare(minimum,version)>0) throw Error('Minimum version exceeds candidate');
  if (!/^[a-f0-9]{40}$/.test(sourceSha)) throw Error('Exact source SHA required');
  const name=`Aurora.Native.Preview_${version}_x64-setup.exe`;
  verifyUpdaterSignature(bytes,signature,publicKey,version,name);
  if (!bytes.length || bytes.length > 256*1024*1024) throw Error('Native installer size is invalid');
  return validateNativePayload({ schemaVersion:1, product:'com.aurora.launcher.native-preview',channel:'native-preview',runtime:'slint',
    platform:'windows',architecture:'x86_64',packageType:'nsis-per-user',version,minimumInstalledVersion:minimum,sourceSha,
    url:`https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v${version}/${name}`,
    sizeBytes:bytes.length,sha256:sha256(bytes),installerSignature:signature });
}
export function createNativeEnvelope(payloadBytes, signature, publicKey) {
  if (payloadBytes.length > 32*1024) throw Error('Native payload exceeds bound');
  const payload=JSON.parse(strictUtf8(payloadBytes));
  // Authenticate the original bytes before interpreting fields.
  verifyUpdaterSignature(payloadBytes,signature,publicKey,payload.version,'native-update-payload.json');
  validateNativePayload(payload);
  return {payload:payloadBytes.toString('base64'),signature};
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [mode,...args]=process.argv.slice(2);
  if (mode==='payload' && args.length===7) {
    const [installer,sigFile,keyFile,version,minimum,sourceSha,output]=args;
    if (basename(installer)!==`Aurora.Native.Preview_${version}_x64-setup.exe`) throw Error('Native installer filename mismatch');
    const bytes=await readFile(installer); const sig=(await readFile(sigFile,'utf8')).trim();const key=(await readFile(keyFile,'utf8')).trim();
    await writeFile(output,JSON.stringify(createNativePayload(bytes,sig,key,version,minimum,sourceSha),null,2)+'\n',{flag:'wx'});
  } else if (mode==='envelope' && args.length===4) {
    const [payloadFile,sigFile,keyFile,output]=args;
    const bytes=await readFile(payloadFile);const sig=(await readFile(sigFile,'utf8')).trim();const key=(await readFile(keyFile,'utf8')).trim();
    await writeFile(output,JSON.stringify(createNativeEnvelope(bytes,sig,key))+'\n',{flag:'wx'});
  } else if ((mode==='inventory' || mode==='verify') && args.length===4) {
    const [directory,keyFile,version,sourceSha]=args;
    const metadata=await verifyNativeDirectory(directory,(await readFile(keyFile,'utf8')).trim(),version,sourceSha);
    const path=join(directory,'native-release-assets.json');
    if (mode==='inventory') await writeFile(path,JSON.stringify(metadata,null,2)+'\n',{flag:'wx'});
    else if (JSON.stringify(JSON.parse(await readFile(path,'utf8')))!==JSON.stringify(metadata)) throw Error('Native inventory differs');
  } else { throw Error('Usage: native-update.mjs payload installer sig public-key version minimum source-sha output | envelope payload sig public-key output | inventory/verify directory public-key version source-sha'); }
}
