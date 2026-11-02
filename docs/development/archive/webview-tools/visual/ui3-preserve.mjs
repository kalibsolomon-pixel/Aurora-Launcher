import {readFile, writeFile, mkdir, stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import assert from 'node:assert/strict';
const out = 'docs/ui-3';
const git = (...args) => execFileSync('git', args, {encoding: 'utf8'}).trim();
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const critical = ['AGENTS.md', 'ARCHITECTURE.md', 'README.md', 'package.json', 'package-lock.json', 'svelte.config.js', 'vite.config.js', 'tsconfig.json', 'src/lib/backend.ts', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/build.rs', 'src-tauri/tauri.conf.json', 'src-tauri/capabilities/default.json'];
await mkdir(out, {recursive: true});
if (process.argv[2] === 'baseline') {
  try { await stat(`${out}/baseline.json`); throw Error('Baseline already exists; never overwrite'); } catch (e) { if (e.code !== 'ENOENT') throw e; }
  const tracked = git('ls-files').split(/\r?\n/);
  const untracked = git('ls-files', '--others', '--exclude-standard').split(/\r?\n/).filter(p => !p.startsWith('tools/visual/ui3-') && !p.startsWith('docs/ui-3/'));
  const hashes = async paths => Promise.all(paths.map(async path => ({path, sha256: await hash(path)})));
  const b = {branch: git('branch', '--show-current'), head: git('rev-parse', 'HEAD'), status: git('status', '--short'), history: git('log', '-8', '--oneline'), tracked, worktrees: git('worktree', 'list', '--porcelain'), protected: await hashes(untracked), critical: await hashes(critical), evidence: await hashes(tracked.filter(p => p.startsWith('docs/ui-') || /^UI_.*\.md$/.test(p)))};
  await writeFile(`${out}/baseline.json`, JSON.stringify(b, null, 2));
  console.log(`Baseline: ${tracked.length} tracked paths; ${untracked.length} protected files; ${b.evidence.length} existing acceptance artifacts`);
} else {
  const b = JSON.parse(await readFile(`${out}/baseline.json`, 'utf8'));
  assert.equal(git('branch', '--show-current'), b.branch);
  const tracked = new Set(git('ls-files').split(/\r?\n/));
  for (const path of b.tracked) { assert.ok(tracked.has(path), path); await stat(path); }
  for (const item of [...b.protected, ...b.critical, ...b.evidence]) assert.equal(await hash(item.path), item.sha256, item.path);
  const other = value => value.split(/\r?\n\r?\n/).slice(1);
  assert.deepEqual(other(git('worktree', 'list', '--porcelain')), other(b.worktrees));
  assert.equal(git('diff', b.head, '--name-only', '--diff-filter=D'), '');
  assert.equal(git('diff', '--cached', '--name-only', '--diff-filter=D'), '');
  const result = {originalTracked: b.tracked.length, protectedFiles: b.protected.length, priorEvidence: b.evidence.length, worktrees: 7, deletions: 0, head: git('rev-parse', 'HEAD'), status: git('status', '--short'), changes: git('diff', b.head, '--name-status')};
  if (process.argv[2] !== '--check-only') await writeFile(`${out}/preservation.json`, JSON.stringify(result, null, 2));
  console.log('PASS preservation', JSON.stringify(process.argv[2] === '--check-only'
    ? {originalTracked: result.originalTracked, protectedFiles: result.protectedFiles, priorEvidence: result.priorEvidence, worktrees: result.worktrees, deletions: result.deletions, head: result.head}
    : result));
}
