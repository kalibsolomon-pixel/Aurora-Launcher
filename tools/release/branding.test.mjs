import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {nativeName} from './native-update.mjs';
const text = path => readFile(new URL('../../'+path,import.meta.url),'utf8');

test('distribution entry points use Aurora Launcher and aurora-launcher.exe',async()=>{
  assert.match(await text('native/Cargo.toml'),/\[\[bin\]\]\s*name = "aurora-launcher"/);
  const resources=await text('native/build.rs');
  assert.match(resources,/resource.set\("ProductName", "Aurora Launcher"\)/);
  assert.match(resources,/resource.set\("OriginalFilename", "aurora-launcher.exe"\)/);
  const packaging=await text('native/tools/package-windows.ps1');
  assert.ok(packaging.includes('aurora-launcher.exe'));
  assert.ok(packaging.includes('Aurora.Launcher_${version}_x64-setup.exe'));
  const workflow=await text('.github/workflows/native-release.yml');
  assert.ok(workflow.includes('Aurora.Launcher_$($env:VERSION)_x64-setup.exe'));
  assert.ok(!workflow.includes('Aurora.Native.Preview_'));
  const installer=await text('native/installer/native-preview.nsi');
  assert.match(installer,/!define PRODUCT "Aurora Launcher"/);
  assert.match(installer,/!define EXECUTABLE "aurora-launcher.exe"/);
  assert.match(await text('core/src/shortcuts.rs'),/pub const PRODUCT_NAME: &str = "Aurora Launcher"/);
  assert.equal(nativeName('2.1.1'),'Aurora.Launcher_2.1.1_x64-setup.exe');
  const readme=await text('README.md');
  assert.match(readme,/^# Aurora Launcher\r?\n/);
  assert.doesNotMatch(readme,/Aurora Native|Aurora Launcher Native|Aurora\.Native\.Preview|aurora-native\.exe/);
});
