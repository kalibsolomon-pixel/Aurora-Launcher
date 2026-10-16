// Retain historical assertions; redirect only their newly written evidence.
import {registerHooks} from 'node:module';
import {readFileSync} from 'node:fs';
import {mkdir} from 'node:fs/promises';
const suite=process.argv[2];
if(!['ui1','ui2','ui2b','ui2b1-crop','ui3','ui31','ui4','ui5'].includes(suite))throw Error('Unknown regression suite');
const entry=new URL(`./${suite}.test.mjs`,import.meta.url),out=`docs/performance-p2/regressions/${suite}`;
await mkdir(out,{recursive:true});
process.env.AURORA_UI2_OUTPUT=out;process.env.AURORA_UI3_OUTPUT=out;
process.env.AURORA_PLAYWRIGHT_MODULE??='file:///C:/Users/kalib/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright/index.mjs';
registerHooks({load(url,context,nextLoad){
 if(url!==entry.href)return nextLoad(url,context);
 let source=readFileSync(entry,'utf8');
 if(suite==='ui1')source=source.replaceAll('docs/ui-1/',`${out}/`);
 if(suite==='ui2b1-crop')source=source.replace('docs/ui-2b-1/crop-tests.json',`${out}/crop-tests.json`);
 if(suite==='ui31')source=source.replace("const out='docs/ui-3-1'",`const reference='docs/ui-3-1';const out='${out}'`).replaceAll('readFile(`${out}/','readFile(`${reference}/');
 if(suite==='ui4')source=source.replace("const out='docs/ui-4/browser'",`const out='${out}'`);
 if(suite==='ui5')source=source.replaceAll('docs/ui-5/browser',out);
 return {format:'module',source,shortCircuit:true};
}});
await import(entry.href);
