// The isolated debug package must already be running with its bundled production
// frontend, disposable AURORA_DIAGNOSTIC_DATA_ROOT, and CDP port 9441. Seed its
// appearance through the actual UI as described in docs/spxctra-ui/README.md.
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
const { chromium } = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.connectOverCDP('http://127.0.0.1:9441');
const page = browser.contexts()[0].pages()[0];
const errors = [];
page.on('pageerror', e => errors.push(e.message));
try {
  await page.waitForFunction(() => document.querySelector('.home-page'));
  assert.ok(page.url().startsWith('http://tauri.localhost/'), `Expected bundled app, got ${page.url()}`);
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'midnight');
  assert.equal(await page.getByRole('button', { name: 'Developer', exact: true }).count(), 0);
  assert.equal(await page.getByRole('button', { name: 'Accounts — not signed in', exact: true }).count(), 1);
  assert.equal(await page.getByRole('heading', { name: 'No instances yet' }).count(), 1);
  await page.screenshot({ path: 'docs/spxctra-ui/native/packaged-home.png' });
  await page.locator('.skip-link').focus();
  await page.keyboard.press('Enter');
  assert.equal(await page.locator('main.content').evaluate(e => e === document.activeElement), true);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.locator('.settings-nav').getByRole('button', { name: 'Appearance', exact: true }).click();
  assert.equal(await page.getByRole('radio', { name: 'Midnight', exact: true }).isChecked(), true);
  await page.screenshot({ path: 'docs/spxctra-ui/native/packaged-restored-midnight.png' });
  // Return only this disposable root to the original theme through Rust persistence.
  await page.getByRole('radio', { name: 'Aurora Dark', exact: true }).check();
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'aurora-dark');
  await page.screenshot({ path: 'docs/spxctra-ui/native/packaged-appearance.png' });
  assert.deepEqual(errors, []);
  await writeFile('docs/spxctra-ui/native/package-acceptance.json', JSON.stringify({
    url: page.url(), restoredNativeTheme: 'midnight', savedNativeTheme: 'aurora-dark',
    noDeveloperDestination: true, signedOut: true, noInstances: true,
    skipLinkFocus: true, errors,
    scope: 'Actual bundled production frontend in isolated debug native package; no accounts, games or provider writes',
  }, null, 2));
  console.log('PASS bundled production frontend, native restored theme, native save, navigation, skip-link focus and no page errors');
} finally { await browser.close(); }
