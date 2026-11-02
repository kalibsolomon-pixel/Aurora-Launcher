// Temporarily serve the byte-exact baseline component, preserving our pending
// UI-3 edits in memory and restoring them even when capture fails. No checkout,
// reset, owner file or unrelated worktree is involved.
import {readFile,writeFile,mkdtemp} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
const paths=['src/lib/instances/ModrinthBrowse.svelte','src/lib/instances/InstalledArtwork.svelte','src/lib/instances/InstanceModsPanel.svelte','src/lib/instances/InstancePacksPanel.svelte','src/lib/shell/phase-f.css'];
const pending=await Promise.all(paths.map(path=>readFile(path)));
const backup=await mkdtemp(join(tmpdir(),'aurora-ui3-source-'));
for(let i=0;i<paths.length;i++)await writeFile(join(backup,String(i)),pending[i]);
console.log('UI-3 source retained at',backup);
async function restore() {
  for(let i=0;i<paths.length;i++)for(let attempt=0;attempt<5;attempt++) {
    try {await writeFile(paths[i],pending[i]);break;} catch(e) {if(attempt===4)throw e;await new Promise(r=>setTimeout(r,200));}
  }
}
try {
  for(const path of paths)await writeFile(path,execFileSync('git',['show',`7c76626:${path}`]));
  await import('./ui3-capture.mjs');
} finally {await restore();console.log('Restart the review server before after-captures: Vite can retain baseline component styles across temporary source swaps.');}
