import {readFile,writeFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const git=(...args)=>execFileSync('git',args,{encoding:'utf8'}).trim();
const baseline=JSON.parse(await readFile('docs/ui-5/baseline.json','utf8'));
const scoped=['ARCHITECTURE.md','src/lib/pages/SettingsPage.svelte','src/lib/launcher/navigation.svelte.ts',
 'src/lib/launcher/HomeWidgetSettings.svelte','src/lib/launcher/DiscordSettings.svelte'];
for(const item of [...baseline.tracked,...baseline.protected])if(!scoped.includes(item.path))
 assert.equal(createHash('sha256').update(await readFile(item.path)).digest('hex'),item.sha256,item.path);
const otherWorktrees=s=>s.split(/\r?\n\r?\n/).slice(1).join('\n');
assert.equal(otherWorktrees(git('worktree','list','--porcelain')),otherWorktrees(baseline.worktrees));
assert.equal(git('branch','--show-current'),baseline.branch);
assert.equal(git('diff',baseline.head,'--name-only','--diff-filter=DR'),'');
assert.equal(git('diff','--cached','--name-only','--diff-filter=DR'),'');
execFileSync('git',['diff',baseline.head,'--check']);
const result={startingHead:baseline.head,checkedTracked:baseline.tracked.length,scopedExistingFiles:scoped,
 protectedFiles:baseline.protected.length,worktrees:7,otherWorktreesUnchanged:true,deletions:0,
 nativeAndReleaseFilesByteIdentical:true,homeRendererRegistryAndPersistenceByteIdentical:true};
await writeFile('docs/ui-5/preservation.json',JSON.stringify(result,null,2));console.log('PASS preservation',JSON.stringify(result));
