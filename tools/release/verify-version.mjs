import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {join,dirname} from 'node:path';
const root=join(dirname(fileURLToPath(import.meta.url)),'../..');
const json=p=>JSON.parse(readFileSync(join(root,p),'utf8'));
const text=p=>readFileSync(join(root,p),'utf8');
const pkg=json('package.json'),lock=json('package-lock.json');
const version=pkg.version;
if(!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)||version.split('.').some(n=>Number(n)>65535))throw Error('Invalid native version');
for(const path of ['core/Cargo.toml','native/Cargo.toml','native-gpui/Cargo.toml']){
 const actual=/^version\s*=\s*"([^"]+)"/m.exec(text(path))?.[1];
 if(actual!==version)throw Error(`${path} version differs`);
}
for(const path of ['core/Cargo.lock','native/Cargo.lock','native-gpui/Cargo.lock']){
 const packages=text(path).split('[[package]]');
 for(const name of path.startsWith('native-gpui')?['aurora-native-gpui','aurora-launcher']:path.startsWith('native')?['aurora-native','aurora-launcher']:['aurora-launcher']){
  const entry=packages.find(p=>p.includes(`name = "${name}"`));
  if(!entry||/^version\s*=\s*"([^"]+)"/m.exec(entry)?.[1]!==version)throw Error(`${path} ${name} differs`);
 }
}
if(lock.version!==version||lock.packages?.['']?.version!==version)throw Error('npm lock version differs');
if(process.argv.length>2 && (process.argv.length!==4||!['--release','--native-release'].includes(process.argv[2])||process.argv[3]!==version))throw Error('Requested release differs from native source');
console.log(`Aurora Launcher version contract: ${version}`);
