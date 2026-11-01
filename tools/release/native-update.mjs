// Local preparation only. Never contacts GitHub, publishes, signs, or accepts a
// private key. Both installer and payload signatures are supplied by the owner.
import { readFile, writeFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyUpdaterSignature, versionParts, sha256, strictUtf8 } from './updater-release.mjs';

export function createNativePayload(bytes, signature, publicKey, version, minimum, sourceSha) {
  versionParts(version); versionParts(minimum);
  if (!/^[a-f0-9]{40}$/.test(sourceSha)) throw Error('Exact source SHA required');
  const name=`Aurora.Native.Preview_${version}_x64-setup.exe`;
  verifyUpdaterSignature(bytes,signature,publicKey,version,name);
  if (!bytes.length || bytes.length > 256*1024*1024) throw Error('Native installer size is invalid');
  return { schemaVersion:1, product:'com.aurora.launcher.native-preview',channel:'native-preview',runtime:'slint',
    platform:'windows',architecture:'x86_64',packageType:'nsis-per-user',version,minimumInstalledVersion:minimum,sourceSha,
    url:`https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v${version}/${name}`,
    sizeBytes:bytes.length,sha256:sha256(bytes),installerSignature:signature };
}
export function createNativeEnvelope(payloadBytes, signature, publicKey) {
  if (payloadBytes.length > 32*1024) throw Error('Native payload exceeds bound');
  const payload=JSON.parse(strictUtf8(payloadBytes));
  // Authenticate the original bytes before interpreting fields.
  verifyUpdaterSignature(payloadBytes,signature,publicKey,payload.version,'native-update-payload.json');
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
  } else { throw Error('Usage: native-update.mjs payload installer sig public-key version minimum source-sha output | envelope payload sig public-key output'); }
}
