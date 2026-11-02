import {spawnSync} from 'node:child_process';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
const [name,command,...args]=process.argv.slice(2);
if(!/^[a-z0-9-]+$/.test(name??'')||!command)throw Error('Expected log name and command');
const out='docs/ui-3-1/checks';await mkdir(out,{recursive:true});
const env={...process.env,AURORA_PLAYWRIGHT_MODULE:process.env.AURORA_PLAYWRIGHT_MODULE||'file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs',AURORA_UI2_OUTPUT:'docs/ui-3-1/regression',AURORA_UI3_OUTPUT:'docs/ui-3-1/regression/ui3'};
const result=spawnSync(command,args,{env,encoding:'utf8',windowsHide:true,maxBuffer:24*1024*1024});
await writeFile(`${out}/${name}.txt`,(result.stdout??'')+(result.stderr??'')+(result.error?String(result.error):''));
let checks={};try{checks=JSON.parse(await readFile(`${out}/exits.json`,'utf8'));}catch{}
checks[name]={command:[command,...args],exit:result.status,signal:result.signal,error:result.error?.message??null};
await writeFile(`${out}/exits.json`,JSON.stringify(checks,null,2));
console.log(name,'exit',result.status,((result.stdout??'')+(result.stderr??'')).slice(-700));process.exitCode=result.status??1;
