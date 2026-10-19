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
  assert.notEqual(candidate.productName, production.productName);
  assert.match(candidate.version, /^1\.4\.2-private\./);
  assert.equal(candidate.bundle.createUpdaterArtifacts, false);
  assert.deepEqual(candidate.bundle.targets, ['nsis']);
  assert.equal(candidate.bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(typeof production.plugins.updater.pubkey, 'string');
});
test('all launch-facing Java checks use execution eligibility', () => {
  const app = read('src-tauri/src/application.rs');
  const readiness = app.slice(app.indexOf('async fn calculate_play_readiness'), app.indexOf('pub async fn get_play_readiness'));
  assert.match(readiness, /validate_runtime_eligibility\(managed, &plan\)/);
  assert.match(app, /validate_runtime_eligibility\(&managed, &runtime_plan\)/);
  assert.match(read('src-tauri/src/instances/lifecycle.rs'), /if execute_diagnostic \{\s*crate::runtime::install::validate_runtime_eligibility/);
});
