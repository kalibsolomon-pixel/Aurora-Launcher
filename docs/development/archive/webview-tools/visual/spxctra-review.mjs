// Actual Svelte components with synthetic accounts/instances; no real game actions.
// Start tools/visual/serve.mjs first. Never points at personal launcher data.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
const { chromium } = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const stage = process.argv[2];
assert.ok(['before', 'after'].includes(stage), 'Expected before or after');
const output = `docs/spxctra-ui/${stage}`;
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const errors = [], captures = [], performance = [];
const screens = [
  ['home', 'l3&skin=1'],
  ['instances', 'page=instances&instances=4'],
  ['overview', 'tab=overview'],
  ['mods', 'tab=mods'],
  ['resource-packs', 'tab=resourcePacks'],
  ['shaders', 'tab=shaders'],
  ['instance-settings', 'tab=settings'],
  ['launcher', 'page=settings&category=General'],
  ['appearance', 'page=settings'],
  ['cosmetics', 'l3&page=cosmetics'],
];
try {
  for (const [width, height] of [[1120, 760], [720, 520]]) {
    for (const [name, query] of screens) {
      const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' });
      const page = await context.newPage();
      page.on('pageerror', e => errors.push(e.message));
      await page.goto(`http://127.0.0.1:1422/?${query}&background=simple`);
      await page.locator('main.content').waitFor();
      if (name === 'appearance') await page.locator('.settings-nav').getByRole('button', { name: 'Appearance', exact: true }).click();
      await page.evaluate(() => document.fonts.ready);
      await page.waitForTimeout(350);
      const geometry = await page.evaluate(() => {
        const main = document.querySelector('main.content');
        return { clientWidth: main.clientWidth, scrollWidth: main.scrollWidth,
          documentWidth: document.documentElement.scrollWidth,
          viewportWidth: innerWidth };
      });
      assert.ok(geometry.scrollWidth <= geometry.clientWidth + 1, `${name} ${width}: content overflow ${JSON.stringify(geometry)}`);
      assert.ok(geometry.documentWidth <= width + 1, `${name}: window overflow`);
      await page.screenshot({ path: `${output}/${name}-${width}.png` });
      captures.push({ name, width, height, geometry });
      await context.close();
    }
  }
  const page = await browser.newPage({ viewport: { width: 1120, height: 760 }, reducedMotion: 'reduce' });
  for (const theme of ['midnight', 'oled']) {
    await page.goto(`http://127.0.0.1:1422/?page=settings&background=simple&theme=${theme}`);
    await page.locator('.settings-nav').getByRole('button', { name: 'Appearance', exact: true }).click();
    await page.screenshot({ path: `${output}/appearance-${theme}.png` });
  }
  await page.goto('http://127.0.0.1:1422/?l3&background=borealis&reduced=1');
  await page.screenshot({ path: `${output}/home-borealis.png` });
  await page.getByRole('button', { name: /Accounts — signed in/ }).click();
  await page.getByRole('dialog').waitFor();
  await page.screenshot({ path: `${output}/accounts-dialog.png` });
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('.account-chip').evaluate(e => e === document.activeElement), true, 'Dialog returns focus');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  for (const category of ['Updates', 'Discord', 'Accessibility', 'About']) {
    await page.locator('.settings-nav').getByRole('button', { name: category, exact: true }).click();
    await page.screenshot({ path: `${output}/settings-${category.toLowerCase()}.png` });
  }
  if (stage === 'after') {
    await page.goto('http://127.0.0.1:1422/?background=simple');
    await page.keyboard.press('Tab');
    assert.equal(await page.locator('.skip-link').evaluate(e => e === document.activeElement), true);
    await page.screenshot({ path: `${output}/skip-link-focus.png` });
    await page.keyboard.press('Enter');
    assert.equal(await page.locator('main.content').evaluate(e => e === document.activeElement), true);
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.locator('.settings-nav button').first().focus();
    await page.keyboard.press('End');
    assert.equal(await page.locator('.settings-nav button[aria-current]').innerText(), 'About');
    await page.keyboard.press('Home');
    assert.equal(await page.locator('.settings-nav button[aria-current]').innerText(), 'Appearance');
    await page.emulateMedia({ forcedColors: 'active' });
    await page.screenshot({ path: `${output}/forced-colors.png` });
    await page.emulateMedia({ forcedColors: 'none' });
  }
  // Browser-only paired observations. These are not desktop startup, physical
  // visibility, process memory, GPU timing, or evidence about Slint/egui.
  for (const background of ['simple', 'borealis']) {
    for (let trial = 0; trial < 3; trial++) {
      const context = await browser.newContext({ viewport: { width: 1120, height: 760 } });
      const sample = await context.newPage();
      const cdp = await context.newCDPSession(sample);
      await cdp.send('Performance.enable');
      await sample.goto(`http://127.0.0.1:1422/?page=settings&background=${background}`);
      await sample.waitForTimeout(700);
      const metrics = async () => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
      const before = await metrics();
      await sample.waitForTimeout(3000);
      const after = await metrics();
      const details = await sample.evaluate(async () => {
        const all = [...document.querySelectorAll('*')];
        const filtered = all.filter(e => getComputedStyle(e).backdropFilter !== 'none').length;
        const frames = []; let last = performance.now();
        await new Promise(resolve => {
          function frame(now) { frames.push(now - last); last = now; frames.length < 90 ? requestAnimationFrame(frame) : resolve(); }
          requestAnimationFrame(frame);
        });
        return { filteredElements: filtered, rafIntervalsMs: frames.slice(1) };
      });
      performance.push({ background, trial, seconds: after.Timestamp - before.Timestamp,
        taskSeconds: after.TaskDuration - before.TaskDuration,
        layoutSeconds: after.LayoutDuration - before.LayoutDuration,
        jsHeapBytes: after.JSHeapUsedSize, ...details });
      await context.close();
    }
  }
  assert.deepEqual(errors, []);
  await writeFile(`${output}/results.json`, JSON.stringify({ stage, captures, errors, performance }, null, 2));
  console.log(`${stage}: ${captures.length} dimension checks, theme/dialog/settings captures, 6 browser performance observations; no page errors`);
} finally { await browser.close(); }
