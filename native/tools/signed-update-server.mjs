// Local acceptance only. Ephemeral private key stays in this process's memory.
import {createServer} from 'node:http';
import {generateKeyPairSync,createHash,sign} from 'node:crypto';
import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {join,resolve,dirname,basename} from 'node:path';
import {tmpdir} from 'node:os';
const [directory,id]=process.argv.slice(2), root=resolve(directory??'');
if(dirname(root).toLowerCase()!==resolve(tmpdir()).toLowerCase() || !basename(root).startsWith('aurora-native9-') || !/^[a-f0-9]{32}$/.test(id??'') || !existsSync(join(root,'NATIVE-RELEASE-DISPOSABLE.txt'))) throw Error('Marked disposable acceptance root required');
const {privateKey,publicKey}=generateKeyPairSync('ed25519');
const keyId=Buffer.from('localE2E');
const key=Buffer.from(`untrusted comment: AURORA LOCAL UPDATE ACCEPTANCE ONLY\n${Buffer.concat([Buffer.from('Ed'),keyId,publicKey.export({type:'spki',format:'der'}).subarray(-32)]).toString('base64')}\n`).toString('base64');
const signature=(bytes,name)=>{
  const sig=sign(null,createHash('blake2b512').update(bytes).digest(),privateKey),comment=`file:${name}`;
  return Buffer.from(`untrusted comment: LOCAL ACCEPTANCE ONLY\n${Buffer.concat([Buffer.from('ED'),keyId,sig]).toString('base64')}\ntrusted comment: ${comment}\n${sign(null,Buffer.concat([sig,Buffer.from(comment)]),privateKey).toString('base64')}\n`).toString('base64');
};
let origin, signed;
const name='Aurora.Launcher_1.4.3_x64-setup.exe';
const server=createServer((req,res)=>{
  if(!signed && existsSync(join(root,'signed-installer.exe'))) {
    const bytes=readFileSync(join(root,'signed-installer.exe'));
    const payload=Buffer.from(JSON.stringify({schemaVersion:1,product:`com.aurora.launcher.native-preview.acceptance.${id}`,channel:'native-acceptance',runtime:'slint',platform:'windows',architecture:'x86_64',packageType:'nsis-per-user',version:'1.4.3',minimumInstalledVersion:'1.4.2',sourceSha:readFileSync(join(root,'source-sha.txt'),'utf8').trim(),url:`${origin}/native-v1.4.3/${name}`,sizeBytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex'),installerSignature:signature(bytes,name)}));
    signed={bytes,envelope:Buffer.from(JSON.stringify({payload:payload.toString('base64'),signature:signature(payload,'native-update-payload.json')}))};
    writeFileSync(join(root,'local-native-update.json'),signed.envelope,{flag:'wx'});
  }
  const bytes=req.url==='/native-update.json'?signed?.envelope:req.url===`/native-v1.4.3/${name}`?signed?.bytes:null;
  if(!bytes){res.writeHead(404);res.end();return;}
  res.writeHead(200,{'Content-Length':bytes.length,'Content-Type':'application/octet-stream'});
  res.end(bytes);
});
server.listen(0,'127.0.0.1',()=>{origin=`http://127.0.0.1:${server.address().port}`;writeFileSync(join(root,'local-signing-public.json'),JSON.stringify({origin,key,id}),{flag:'wx'});});
setTimeout(()=>server.close(),90*60*1000).unref();
setInterval(()=>{if(existsSync(join(root,'stop-server')))server.close(()=>process.exit(0));},500).unref();
