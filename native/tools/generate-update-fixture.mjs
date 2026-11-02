// Offline fixtures only. The ephemeral test private key is never written.
import { generateKeyPairSync, createHash, sign } from 'node:crypto';
import { writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const { privateKey, publicKey } = generateKeyPairSync('ed25519');
const keyId = Buffer.from('native9!');
const raw = publicKey.export({format:'der',type:'spki'}).subarray(-32);
const key = Buffer.from(`untrusted comment: NATIVE-9 TEST ONLY\n${Buffer.concat([Buffer.from('Ed'),keyId,raw]).toString('base64')}\n`).toString('base64');
function signature(bytes, name) {
  const sig = sign(null,createHash('blake2b512').update(bytes).digest(),privateKey);
  const comment = `file:${name}`;
  const global = sign(null,Buffer.concat([sig,Buffer.from(comment)]),privateKey);
  return Buffer.from(`untrusted comment: NATIVE-9 TEST ONLY\n${Buffer.concat([Buffer.from('ED'),keyId,sig]).toString('base64')}\ntrusted comment: ${comment}\n${global.toString('base64')}\n`).toString('base64');
}
const installer = Buffer.from('NATIVE-9 harmless signed fixture. This is not an executable.\n');
const offer = {
  schemaVersion:1, product:'com.aurora.launcher.native-preview', channel:'native-preview', runtime:'slint',
  platform:'windows', architecture:'x86_64', packageType:'nsis-per-user', version:'1.4.3',
  minimumInstalledVersion:'1.4.2',sourceSha:'a'.repeat(40),
  url:'https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v1.4.3/Aurora.Native.Preview_1.4.3_x64-setup.exe',
  sizeBytes:installer.length,sha256:createHash('sha256').update(installer).digest('hex'),
  installerSignature:signature(installer,'Aurora.Native.Preview_1.4.3_x64-setup.exe')
};
function envelope(value) {
  const payload=Buffer.from(JSON.stringify(value));
  return {payload:payload.toString('base64'),signature:signature(payload,'native-update-payload.json')};
}
const invalid = [
  {schemaVersion:2}, {product:'com.aurora.launcher'}, {channel:'stable'}, {runtime:'webview'},
  {platform:'linux'},{architecture:'aarch64'}, {packageType:'msi'}, {minimumInstalledVersion:'1.4.4'},
  {version:'1.4.3+other'}, {sha256:'0'.repeat(63)}, {sizeBytes:0}, {sizeBytes:268435457},
  {url:'https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/v1.4.3/Aurora.Launcher_1.4.3_x64-setup.exe'},
  {url:offer.url+'?redirect=1'},{sourceSha:'z'.repeat(40)}, {unknown:'unsupported'}
].map(change=>envelope({...offer,...change}));
const fixture={testOnly:true,key,installer:installer.toString('base64'),valid:envelope(offer),invalid};
const directory=fileURLToPath(new URL('../../core/src/updates/fixtures/',import.meta.url));
mkdirSync(directory,{recursive:true});
writeFileSync(directory+'native-test.json',JSON.stringify(fixture,null,2)+'\n',{flag:'wx'});
