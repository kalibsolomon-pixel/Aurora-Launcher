import {readFile, stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import assert from 'node:assert/strict';
const baseline = JSON.parse(await readFile('docs/ui-2/baseline.json','utf8'));
const git = (...args) => execFileSync('git',args,{encoding:'utf8'}).trim();
assert.equal(git('branch','--show-current'),baseline.branch);
const current = new Set(git('ls-files').split(/\r?\n/));
for (const path of baseline.tracked) {assert.ok(current.has(path), `Tracked path removed: ${path}`); await stat(path);}
for (const file of baseline.critical) await stat(file);
for (const file of baseline.protected) assert.equal(createHash('sha256').update(await readFile(file.path)).digest('hex').toUpperCase(),file.hash);
const identities = text => text.split(/\r?\n/).filter(line => /^(worktree |branch |detached)/.test(line));
assert.deepEqual(identities(git('worktree','list','--porcelain')), identities(baseline.worktrees.join('\n')));
assert.equal(git('diff','--name-only','--diff-filter=D'), '');
assert.equal(git('diff','--cached','--name-only','--diff-filter=D'), '');
console.log(`PASS preservation: ${baseline.tracked.length} original tracked paths, critical files, four local hashes, seven worktree identities; no deletions`);
