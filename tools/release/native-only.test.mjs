import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,access,readdir} from 'node:fs/promises';

test('supported application has no legacy frontend or WebView host dependency', async()=>{
  for(const path of ['src','svelte.config.js','vite.config.js','core/src/main.rs','core/src/application_tauri.rs']) {
    await assert.rejects(access(new URL('../../'+path,import.meta.url)),{code:'ENOENT'});
  }
  for(const path of ['core/Cargo.toml','native/Cargo.toml','core/Cargo.lock','native/Cargo.lock']) {
    const text=await readFile(new URL('../../'+path,import.meta.url),'utf8');
    assert.doesNotMatch(text,/^tauri(?:-build|-plugin-\w+)?\s*=|^name = "(?:tauri|wry|webview2-com|webkit2gtk)"/m,path);
  }
  const files=await readdir(new URL('../../.github/workflows/',import.meta.url));
  assert.ok(files.includes('native-release.yml'));
  assert.ok(files.every(name=>!name.startsWith('launcher-')));
});
