// Real-component smoke checks with isolated browser DTOs, never owner data.
import assert from 'node:assert/strict';
import {mkdir, writeFile} from 'node:fs/promises';
import {openReview, geometry} from './ui1-fixture.mjs';
const {chromium} = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({headless: true});
const page = await browser.newPage({viewport: {width: 1024, height: 768}, reducedMotion: 'reduce'});
const errors = [], results = [];
page.on('pageerror', e => errors.push(e.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/, route => route.abort());
await mkdir('docs/ui-1/states', {recursive: true});
async function check(name, action) {
  await action(); results.push(name); console.log('PASS ' + name);
}
async function contained() {
  const g = await geometry(page);
  assert.equal(g.documentOverflow || g.contentOverflow, false, JSON.stringify(g));
  assert.ok(g.account.bottom <= g.viewport[1] && g.account.x >= 0);
  assert.ok(await page.locator('.account-chip').evaluate(e => e.scrollWidth <= e.clientWidth + 1), 'account text stays inside dock');
}
async function fixtureState(code) {
  await page.evaluate(async code => {
    const source = await (await fetch('/Review.svelte')).text();
    const url = source.match(/from "([^"\n]*store.svelte[^"\n]*)"/)[1];
    const {launcher} = await import(url);
    // Fixed test instructions only; no code or state comes from remote content.
    if (code === 'long-name') launcher.launcherState.instances[0].displayName = 'Aurora — A Very Long Instance Name With 日本語 and Adventure Worlds '.repeat(3);
    if (code === 'blocked') launcher.playReadiness = {...launcher.playReadiness, ready: false, blockers: [{code: 'auth_required', message: 'Synthetic sign-in required.'}]};
  }, code);
}
try {
  for (const [width, height] of [[1440, 900], [1280, 800], [1024, 768], [720, 520]]) {
    await check(`Home/Mods containment and account dialog at ${width}x${height}`, async () => {
      await page.setViewportSize({width, height});
      await openReview(page); await contained();
      assert.equal(await page.locator('.rail-link').count(), 5);
      assert.equal(await page.getByRole('button', {name: 'Play', exact: true}).isEnabled(), true);
      await page.locator('.account-chip').click();
      const dialog = page.getByRole('dialog'); await dialog.waitFor();
      const box = await dialog.boundingBox();
      assert.ok(box.x >= 0 && box.y >= 0 && box.x + box.width <= width + 1 && box.y + box.height <= height + 1);
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('.account-chip').evaluate(e => e === document.activeElement), true);
      await openReview(page, 'mods'); await contained();
      const controls = await page.locator('.browse-mods :is(input[type=search], .filter-input)').evaluateAll(es => es.map(e => e.getBoundingClientRect().height));
      assert.ok(controls.every(h => h === 40));
      const rows = await page.locator('.browse-row').evaluateAll(es => es.map(e => ({width: e.clientWidth, scroll: e.scrollWidth})));
      assert.ok(rows.every(r => r.scroll <= r.width + 1));
    });
  }
  await page.setViewportSize({width: 1024, height: 768});
  await check('Sidebar focus/hover, active marker and Settings navigation', async () => {
    await openReview(page);
    const settings = page.getByRole('button', {name: 'Settings', exact: true});
    await settings.focus();
    assert.equal(await settings.evaluate(e => getComputedStyle(e).outlineStyle), 'solid');
    assert.equal(await settings.locator('.f-tooltip').evaluate(e => getComputedStyle(e).opacity), '1');
    await settings.hover(); await settings.click();
    await page.locator('.launcher-settings').waitFor();
    assert.equal(await settings.getAttribute('aria-current'), 'page');
    assert.equal(await page.locator('.rail-link[aria-current=page]').count(), 1);
    await contained();
    await page.getByRole('navigation', {name: 'Aurora Launcher'}).getByRole('button', {name: 'Home', exact: true}).click();
    await page.locator('.home-page').waitFor();
  });
  await check('Instance picker keyboard selection, long names and readiness', async () => {
    const picker = page.getByRole('button', {name: 'Select Play instance'});
    await picker.focus(); await page.keyboard.press('ArrowDown');
    await page.getByRole('listbox').waitFor(); await page.keyboard.press('End'); await page.keyboard.press('Enter');
    await page.waitForFunction(() => document.querySelector('.picker-trigger').textContent.includes('Vanilla'));
    await picker.click(); await page.keyboard.press('Home'); await page.keyboard.press('Enter');
    await page.waitForFunction(() => document.querySelector('.picker-trigger').textContent.includes('Aurora Client'));
    await fixtureState('long-name'); await contained();
    await picker.click(); const menu = await page.getByRole('listbox').boundingBox();
    assert.ok(menu.x >= 0 && menu.x + menu.width <= 1024);
    await page.keyboard.press('Escape');
    await fixtureState('blocked');
    assert.equal(await page.locator('.play-hero').isDisabled(), true);
    await page.getByText('Synthetic sign-in required.', {exact: true}).waitFor();
    await page.screenshot({path: 'docs/ui-1/states/home-long-name-blocked.png'});
  });
  await check('Widget sizing/order persist through reload; history widgets still load', async () => {
    await openReview(page);
    await page.getByRole('button', {name: 'Edit Home widgets', exact: true}).click();
    await page.getByRole('combobox', {name: 'Skin Library size', exact: true}).selectOption('wide');
    await page.waitForFunction(() => JSON.parse(sessionStorage.getItem('phase-f-review')).layout.widgets.find(w => w.id === 'skin-manager').size === 'wide');
    await page.getByRole('button', {name: 'Move Skin Library later', exact: true}).click();
    await page.waitForFunction(() => JSON.parse(sessionStorage.getItem('phase-f-review')).layout.widgets[0].id === 'cape-selector');
    await page.getByRole('button', {name: 'Recent Servers', exact: true}).click();
    await page.getByRole('button', {name: 'Playtime', exact: true}).click();
    await page.getByRole('button', {name: 'Finish editing Home', exact: true}).click();
    await page.reload();
    await page.locator('[data-home-widget=skin-manager].widget-wide').waitFor();
    await page.locator('[data-home-widget=playtime]').waitFor();
    await page.locator('[data-home-widget=recent-servers]').waitFor();
    assert.equal(await page.locator('[data-home-widget]').first().getAttribute('data-home-widget'), 'cape-selector');
    await contained();
    await page.evaluate(() => sessionStorage.clear());
  });
  await check('Mods search, categories, sorting, pagination and installed state', async () => {
    await openReview(page, 'mods');
    assert.equal(await page.locator('.browse-row').count(), 20);
    assert.equal(await page.getByRole('button', {name: 'Sodium is installed', exact: true}).isDisabled(), true);
    await page.getByRole('button', {name: 'Load more', exact: true}).click();
    await page.waitForFunction(() => document.querySelectorAll('.browse-row').length === 24);
    assert.equal(await page.evaluate(() => window.ui1Requests.filter(r => r.command === 'browse_modrinth').at(-1).request.offset), 20);
    const search = page.getByRole('searchbox', {name: 'Find mods', exact: true});
    await search.fill('Lithium'); await search.press('Enter');
    await page.waitForFunction(() => document.querySelectorAll('.browse-row').length === 2);
    await page.getByRole('button', {name: /^Categories:/}).click();
    await page.getByRole('checkbox', {name: 'Utility', exact: true}).check();
    await page.keyboard.press('Escape');
    await page.getByRole('combobox', {name: 'Sort results'}).selectOption('downloads');
    await page.waitForFunction(() => window.ui1Requests.filter(r => r.command === 'browse_modrinth').at(-1).request.sort === 'downloads');
    const request = await page.evaluate(() => window.ui1Requests.filter(r => r.command === 'browse_modrinth').at(-1).request);
    assert.deepEqual({search: request.search, categories: request.categories, sort: request.sort, instance: request.instanceId, kind: request.contentType}, {search: 'Lithium', categories: ['utility'], sort: 'downloads', instance: 'a'.repeat(32), kind: 'mod'});
    await page.getByRole('button', {name: 'Clear filters', exact: true}).click();
    await page.waitForFunction(() => document.querySelectorAll('.browse-row').length === 20);
  });
  await check('Mods empty/loading/error and safe fixture install feedback', async () => {
    const search = page.getByRole('searchbox', {name: 'Find mods', exact: true});
    await search.fill('NoSuchMod'); await search.press('Enter');
    await page.getByText('No matching projects for this instance.', {exact: true}).waitFor();
    await page.screenshot({path: 'docs/ui-1/states/mods-empty.png'});
    await page.evaluate(() => window.ui1Mode = 'loading');
    await search.fill('Lithium'); await search.press('Enter');
    await page.getByText('Loading Modrinth projects…', {exact: true}).waitFor();
    await page.screenshot({path: 'docs/ui-1/states/mods-loading.png'});
    await page.locator('.browse-row').first().waitFor();
    await page.evaluate(() => window.ui1Mode = 'error'); await search.press('Enter');
    await page.getByRole('alert').filter({hasText: 'Synthetic provider unavailable'}).waitFor();
    await page.screenshot({path: 'docs/ui-1/states/mods-error.png'});
    await page.evaluate(() => window.ui1Mode = 'normal'); await search.press('Enter');
    await page.getByRole('button', {name: 'Install newest eligible version of Lithium', exact: true}).click();
    await page.getByRole('status').filter({hasText: 'Modrinth is unavailable. Try again later.'}).waitFor();
    assert.equal(await page.evaluate(() => window.ui1Requests.filter(r => r.command === 'quick_install_modrinth').length), 1);
    await contained();
  });
  await check('125%/200% text scale, long mod titles, filters and minimum-size wrapping', async () => {
    for (const width of [1024, 720]) for (const percent of [125, 200]) {
      await page.setViewportSize({width, height: 768}); await openReview(page, 'mods');
      await page.addStyleTag({content: `html { font-size: ${percent}% !important; }`});
      const search = page.getByRole('searchbox', {name: 'Find mods', exact: true});
      await search.fill('Very Long Mod Title'); await search.press('Enter');
      await page.locator('.browse-row').first().waitFor(); await contained();
      const controlSizes = await page.locator('.browse-mods .filter-input').evaluateAll(es => es.map(e => ({height: e.getBoundingClientRect().height, font: parseFloat(getComputedStyle(e).fontSize)})));
      assert.ok(controlSizes.every(s => s.height >= s.font * 1.1 + 16), 'filters grow with scaled text');
      const row = page.locator('.browse-row').first();
      assert.ok(await row.evaluate(e => e.scrollWidth <= e.clientWidth + 1));
      await page.getByRole('button', {name: /^Categories:/}).click();
      const popover = await page.getByRole('dialog', {name: 'Category filter'}).boundingBox();
      assert.ok(popover.x >= 0 && popover.x + popover.width <= width + 1);
      await page.keyboard.press('Escape');
      if (width === 720 && percent === 200) await page.screenshot({path: 'docs/ui-1/states/mods-text-200.png'});
      await openReview(page);
      await page.addStyleTag({content: `html { font-size: ${percent}% !important; }`});
      await fixtureState('long-name'); await contained();
      await page.getByRole('button', {name: 'Select Play instance'}).click();
      const picker = await page.getByRole('listbox').boundingBox();
      assert.ok(picker.x >= 0 && picker.x + picker.width <= width + 1);
      await page.keyboard.press('Escape');
      if (width === 720 && percent === 200) await page.screenshot({path: 'docs/ui-1/states/home-text-200.png'});
    }
  });
  await check('Reduced motion, readable metadata contrast and no per-result blur', async () => {
    await page.setViewportSize({width: 1280, height: 800});
    await openReview(page, 'mods', {background: 'simple'});
    const styles = await page.evaluate(() => {
      const row = document.querySelector('.browse-row'), meta = row.querySelector('.browse-meta');
      return {blur: getComputedStyle(row).backdropFilter, color: getComputedStyle(meta).color, background: getComputedStyle(row).backgroundColor, focus: getComputedStyle(document.querySelector('.rail-link')).transitionDuration};
    });
    assert.equal(styles.blur, 'none'); assert.equal(styles.focus, '0s');
    const luminance = color => {
      const rgb = color.match(/[\d.]+/g).slice(0, 3).map(Number).map(x => {x /= 255; return x <= .04045 ? x / 12.92 : ((x + .055) / 1.055) ** 2.4;});
      return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722;
    };
    const a = luminance(styles.color), b = luminance(styles.background), contrast = (Math.max(a, b) + .05) / (Math.min(a, b) + .05);
    assert.ok(contrast >= 4.5, `metadata contrast ${contrast}`);
    console.log(`Metadata contrast on Simple result backing: ${contrast.toFixed(2)}:1`);
  });
  assert.deepEqual(errors, []);
  await writeFile('docs/ui-1/functional.json', JSON.stringify({cases: results.length, results, pageErrors: errors, nativeAcceptance: false, mutationBoundary: 'All interactions use synthetic DTOs and session storage; external network blocked'}, null, 2));
  console.log(`PASS ${results.length} UI-1 browser cases; zero page errors`);
} finally {await browser.close();}
