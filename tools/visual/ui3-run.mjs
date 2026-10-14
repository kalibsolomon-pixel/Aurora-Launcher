import {spawnSync} from 'node:child_process';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
const [name,command,...args]=process.argv.slice(2);
if(!/^[a-z0-9-]+$/.test(name ?? '') || !command)throw Error('Expected log name and command');
const out='docs/ui-3/checks';await mkdir(out,{recursive:true});
const env={...process.env,AURORA_PLAYWRIGHT_MODULE:process.env.AURORA_PLAYWRIGHT_MODULE || 'file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs',AURORA_UI2_OUTPUT:'docs/ui-3/regression',AURORA_UI3_OUTPUT:'docs/ui-3'};
const r=spawnSync(command,args,{env,encoding:'utf8',maxBuffer:24*1024*1024,windowsHide:true});
const output=(r.stdout ?? '')+(r.stderr ?? '')+(r.error ? String(r.error) : '');
await writeFile(`${out}/${name}.txt`,output);
let checks={};try{checks=JSON.parse(await readFile(`${out}/exits.json`,'utf8'));}catch{}
checks[name]={command:[command,...args],exit:r.status,signal:r.signal,error:r.error?.message ?? null};
await writeFile(`${out}/exits.json`,JSON.stringify(checks,null,2));
console.log(name, 'exit',r.status,output.slice(-2000));process.exitCode=r.status ?? 1;
