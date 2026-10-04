// Tests the real Svelte surfaces through the standalone, native-boundary review fixture.
// Start tools/visual/serve.mjs first. AURORA_PLAYWRIGHT_MODULE can point to a bundled runtime.
import assert from 'node:assert/strict';
const { chromium } = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.AURORA_BROWSER_CHANNEL ? { channel: process.env.AURORA_BROWSER_CHANNEL } : {}) });
const page = await browser.newPage({ viewport: { width: 1440, height: 960 }, reducedMotion: 'reduce' });
const origin = 'http://127.0.0.1:1422';
const errors = [];
page.on('pageerror', error => errors.push(error.message));
try {
  await page.goto(origin);
  const selector = page.getByRole('button', { name: 'Select Play instance' });
  await selector.focus(); await page.keyboard.press('ArrowDown');
  await page.getByRole('listbox').waitFor({ state: 'visible' });
  await page.keyboard.press('End'); await page.keyboard.press('Enter');
  await page.waitForFunction(() => document.querySelector('.picker-trigger')?.textContent.includes('Vanilla'));
  assert.equal(await selector.getAttribute('aria-expanded'), 'false');
  assert.equal(await selector.evaluate(el => el === document.activeElement), true);
  await selector.press('ArrowDown'); await page.keyboard.press('Escape');
  assert.equal(await selector.getAttribute('aria-expanded'), 'false');
  await selector.click(); await page.mouse.click(1000, 200);
  assert.equal(await selector.getAttribute('aria-expanded'), 'false');
  console.log('PASS selector: keyboard selection, focus return, Escape and outside close');

  const dock = page.getByRole('button', { name: /^Accounts —/ });
  await dock.click(); const drawer = page.getByRole('dialog');
  await drawer.waitFor({ state: 'visible' });
  const rect = await drawer.boundingBox(); assert.equal(rect.x, 0); assert.equal(rect.width, 420);
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press(i === 0 ? 'Shift+Tab' : 'Tab');
    // Browser chrome may take focus at the tab boundary; underlying app controls stay inert.
    assert.equal(await drawer.evaluate(el => el.contains(document.activeElement) || document.activeElement === document.body), true);
  }
  await page.keyboard.press('Escape'); await drawer.waitFor({ state: 'hidden' });
  await page.waitForFunction(() => document.activeElement?.classList.contains('account-chip'));
  await dock.click(); await page.mouse.click(1000, 450); await drawer.waitFor({ state: 'hidden' });
  await page.setViewportSize({ width: 520, height: 740 }); await dock.click();
  assert.equal((await drawer.boundingBox()).width, 520);
  assert.equal(await drawer.locator('.drawer-body').evaluate(el => getComputedStyle(el).overflowY), 'auto');
  await page.keyboard.press('Escape'); await page.setViewportSize({ width: 1440, height: 960 });
  console.log('PASS accounts: left geometry, native focus containment, return focus, Escape, backdrop and narrow width');

  await page.route('https://cdn.modrinth.com/**', route => route.fulfill({ status: 200, contentType: 'image/png', body: 'invalid image' }));
  await page.goto(`${origin}/?tab=mods&broken-artwork=1`);
  await page.getByRole('button', { name: 'Browse Modrinth', exact: true }).click();
  await page.getByRole('button', { name: /Sodium/ }).first().waitFor();
  await page.waitForFunction(() => document.querySelector('.browse-glyph .artwork img') === null);
  assert.equal(await page.locator('.browse-glyph .artwork svg').first().isVisible(), true);
  console.log('PASS artwork: malformed provider response removes image and keeps the local SVG fallback');
  await page.goto(`${origin}/?page=settings`);
  await page.getByRole('heading', { name: 'General', exact: true }).waitFor();
  await page.getByText('Desktop shortcut', { exact: true }).waitFor();
  assert.equal(await page.getByRole('button', { name: /Create shortcut|Remove shortcut/ }).count(), 0);
  await page.getByRole('button', { name: 'Discord & privacy', exact: true }).click();
  assert.equal(await page.getByRole('button', { name: /(?:Connect|Reconnect) to Discord/ }).count(), 0);
  console.log('PASS settings: real General status, native capability gate and no Discord maintenance control');
  if (process.env.AURORA_SCREENSHOT_DIR) {
    const { mkdir } = await import('node:fs/promises');
    const { join } = await import('node:path');
    const directory = process.env.AURORA_SCREENSHOT_DIR;
    await mkdir(directory, { recursive: true });
    const capture = async name => { await page.screenshot({ path: join(directory, `${name}.png`) }); };
    await page.goto(origin); await selector.waitFor(); await capture('01-home-closed-fixture');
    await selector.click(); await capture('02-home-selector-open-fixture');
    await page.keyboard.press('Escape'); await dock.click(); await drawer.waitFor({ state: 'visible' });
    await capture('03-accounts-drawer-fixture'); await page.keyboard.press('Escape');
    await page.goto(`${origin}/?servers=1`); await page.getByText('mcpvp.club', { exact: true }).waitFor();
    await capture('03b-recent-servers-fallback-fixture');
    await page.goto(`${origin}/?page=instances&instances=2`);
    await page.getByRole('heading', { name: 'Create instance', exact: true }).waitFor();
    await page.getByText(/Aurora Client 2.1.5 is available/).waitFor(); await capture('04-instances-fixture');
    for (const [tab, label] of [['mods', 'mods'], ['resourcePacks', 'resource-packs'], ['shaders', 'shaders']]) {
      await page.goto(`${origin}/?tab=${tab}&updates=1`);
      await page.getByRole('button', { name: 'Browse Modrinth', exact: true }).waitFor();
      await capture(`05-${label}-installed-fixture`);
      await page.getByRole('button', { name: 'Browse Modrinth', exact: true }).click();
      await page.locator('.browse-list').waitFor(); await capture(`06-${label}-browse-fixture`);
    }
    await page.goto(`${origin}/?page=settings`); await page.getByText('Desktop shortcut', { exact: true }).waitFor();
    await capture('07-settings-general-fixture');
    await page.getByRole('button', { name: 'Discord & privacy', exact: true }).click(); await capture('08-settings-discord-fixture');
    console.log(`Fixture screenshots (1440×960) saved to ${directory}; these do not claim native runtime acceptance.`);
  }
  assert.deepEqual(errors, [], 'The rendered surfaces must have no uncaught browser errors.');
} finally { await browser.close(); }
