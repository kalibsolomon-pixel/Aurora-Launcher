import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read = p => readFileSync(new URL(`../../${p}`, import.meta.url), 'utf8');
test('private overlay separates installer identity and keeps updater startup configuration', () => {
  const production = JSON.parse(read('src-tauri/tauri.conf.json'));
  const candidate = JSON.parse(read('src-tauri/tauri.private.conf.json'));
  assert.equal(production.identifier, 'com.aurora.launcher');
  assert.equal(production.version, '../package.json');
  assert.equal(candidate.identifier, 'com.aurora.launcher.private-candidate');
  assert.equal(candidate.mainBinaryName, 'aurora-launcher-private-candidate');
  assert.notEqual(candidate.mainBinaryName, JSON.parse(read('package.json')).name);
  assert.notEqual(candidate.productName, production.productName);
  assert.match(candidate.version, /^1\.4\.2-private\./);
  assert.equal(candidate.bundle.createUpdaterArtifacts, false);
  assert.deepEqual(candidate.bundle.targets, ['nsis']);
  assert.equal(candidate.bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(typeof production.plugins.updater.pubkey, 'string');
});
