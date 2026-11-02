// Two historical helpers hard-code output paths. Retain this run separately and
// restore ONLY those generated artifacts to their recorded pre-task bytes.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const baseline=JSON.parse(await readFile('docs/ui-4/baseline.json','utf8'));
const paths=['docs/ui-2b-1/crop-tests.json','docs/ui-3-1/states/review-1280x800.png'];
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
await mkdir('docs/ui-4/regression/legacy-named',{recursive:true});
for(const path of paths){
 const expected=baseline.tracked.find(p=>p.path===path);assert.ok(expected);
 const current=await readFile(path);await writeFile(`docs/ui-4/regression/legacy-named/${path.split('/').at(-1)}`,current);
 const blob=execFileSync('git',['show',`${baseline.head}:${path}`],{maxBuffer:20*1024*1024});
 const candidates=[blob,Buffer.from(blob.toString('utf8').replace(/(?<!\r)\n/g,'\r\n'))];
 const original=candidates.find(b=>hash(b)===expected.sha256);assert.ok(original,`Cannot prove baseline bytes for ${path}`);
 await writeFile(path,original);assert.equal(hash(await readFile(path)),expected.sha256);console.log('Restored exact baseline',path);
}
