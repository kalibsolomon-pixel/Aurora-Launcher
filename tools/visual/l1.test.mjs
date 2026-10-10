// Deterministic component acceptance. Start tools/visual/serve.mjs first.
// Uses the same optional bundled Playwright module/channel as correction.test.mjs.
import assert from 'node:assert/strict';
const { chromium } = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ headless: true, ...(process.env.AURORA_BROWSER_CHANNEL ? { channel: process.env.AURORA_BROWSER_CHANNEL } : {}) });
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 }, reducedMotion: 'reduce' });
const errors = [];
page.on('pageerror', error => errors.push(error.message));
const png = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAN0lEQVR4nO3QQREAMAgDQYpC5CC2XspUBZ+Ngcvs6bovFpebcQcIECBAgAABAgQIECBAgACBLzCTbALj8Oz2OgAAAABJRU5ErkJggg==';
try {
  for (const kind of ['mod', 'resourcePack', 'shaderPack', 'modpack']) {
    await page.goto('http://127.0.0.1:1422/?page=instances&background=simple');
    await page.getByRole('button', { name: 'Browse Modpacks', exact: true }).waitFor();
    await page.evaluate(async ({ kind, png }) => {
      const moduleUrl = (await (await fetch('/Review.svelte')).text()).match(/from "([^"]*review-core[^"]*)"/)[1];
      const { wrapReviewInvoke } = await import(moduleUrl);
      window.l1Requests = []; window.l1Artwork = {};
      wrapReviewInvoke(previous => async (command, args) => {
        if (command === 'resolve_project_artwork') {
          const id = args.request.projectId;
          const count = window.l1Artwork[id] = (window.l1Artwork[id] || 0) + 1;
          if (id === 'NONE0001') return { source: null, status: 'unavailable', retryAfterMs: null };
          if (id === 'RETRY001' && count === 1) return { source: null, status: 'retryable', retryAfterMs: 100 };
          return { source: png, status: 'available', retryAfterMs: null };
        }
        if (command === 'browse_modrinth') {
          window.l1Requests.push(JSON.parse(JSON.stringify(args.request)));
          const projectType = args.request.contentType;
          return { offset: args.request.offset, totalHits: 4, hits: [
            { projectId: 'LONG0001', title: 'A very long modpack title — 日本語の冒険と探索 '.repeat(4), summary: 'Long multilingual description 中文 العربية 日本語 '.repeat(30), author: 'Fixture', downloads: 1234, categories: ['adventure'], projectType },
            { projectId: 'RETRY001', title: 'Transient artwork recovery', summary: 'A short description.', author: 'Fixture', downloads: 12, categories: [], projectType },
            { projectId: 'NONE0001', title: 'No project icon', summary: 'Intentional fallback.', author: 'Fixture', downloads: 1, categories: [], projectType },
            { projectId: 'VALID001', title: 'Valid artwork', summary: 'Valid canonical pixels.', author: 'Fixture', downloads: 1, categories: [], projectType },
          ] };
        }
        return previous(command, args);
      });
    }, { kind, png });
    await page.getByRole('button', { name: 'Browse Modpacks', exact: true }).click();
    if (kind !== 'modpack') {
      // The standalone flow intentionally only offers modpacks. Mount the
      // instance browser, then use its shared destination tabs.
      await page.evaluate(async () => {
        const url = (await (await fetch('/Review.svelte')).text()).match(/from "([^"]*navigation\.svelte[^"]*)"/)[1];
        const { navigation } = await import(url);
        navigation.openInstance('a'.repeat(32), 'mods');
      });
      await page.getByRole('button', { name: 'Browse Modrinth', exact: true }).click();
      const label = { mod: 'Mods', resourcePack: 'Resource Packs', shaderPack: 'Shaders' }[kind];
      await page.getByRole('group', { name: 'Browse content type' }).getByRole('button', { name: label, exact: true }).click();
    }
    await page.locator('.browse-row').first().waitFor();
    await page.waitForFunction(() => [...document.querySelectorAll('.browse-row img')].filter(i => i.complete && i.naturalWidth > 0).length === 3);
    assert.equal(await page.locator('.browse-row').count(), 4);
    if (kind !== 'modpack') {
      const install = page.getByRole('button', { name: 'Install newest eligible version of Valid artwork', exact: true });
      assert.equal(await install.locator('svg[aria-hidden="true"]').count(), 1);
      assert.equal(await install.locator('svg').getAttribute('width'), '18');
      assert.equal((await install.innerText()).includes('↓'), false);
      assert.match(await install.locator('path').getAttribute('d'), /M5 16v5h14v-5/);
    }
    assert.equal(await page.evaluate(() => window.l1Artwork.RETRY001), 2);
    assert.equal(await page.evaluate(() => window.l1Artwork.NONE0001), 1);
    const categories = page.getByRole('button', { name: /^Categories:/ });
    const sort = page.getByRole('combobox', { name: 'Sort results' });
    const controls = await page.locator('.filter-input').evaluateAll(elements => elements.map(el => {
      const s = getComputedStyle(el), r = el.getBoundingClientRect();
      return { top: r.top, height: r.height, padding: s.paddingTop, border: s.borderTop, radius: s.borderRadius, background: s.backgroundColor, font: s.font };
    }));
    assert.deepEqual(controls[0], controls[1]);
    await page.getByRole('searchbox').fill('retained search');
    await page.locator('.browse-search').getByRole('button', { name: 'Search', exact: true }).click();
    await sort.selectOption('downloads');
    await page.waitForFunction(() => window.l1Requests.at(-1)?.sort === 'downloads');
    assert.equal(await page.evaluate(() => window.l1Requests.at(-1).search), 'retained search');
    await categories.click();
    await page.getByRole('dialog', { name: 'Category filter' }).getByRole('checkbox').first().check();
    await page.waitForFunction(() => window.l1Requests.at(-1)?.categories.length === 1);
    assert.equal(await page.evaluate(() => window.l1Requests.at(-1).search), 'retained search');
    assert.equal(await page.evaluate(() => window.l1Requests.at(-1).sort), 'downloads');
    await page.keyboard.press('Escape');
    assert.equal(await categories.evaluate(el => el === document.activeElement), true);
    assert.equal(await page.getByRole('dialog', { name: 'Category filter' }).count(), 0);
    for (const width of [1600, 1120, 720]) {
      await page.setViewportSize({ width, height: 1000 });
      const layout = await page.locator('.browse').evaluate(el => ({
        overflow: el.scrollWidth > el.clientWidth,
        width: el.clientWidth,
        columns: getComputedStyle(el.querySelector('.browse-list')).gridTemplateColumns.split(' ').length,
        overlaps: [...el.querySelectorAll('.browse-row')].some(row => row.querySelector('.browse-actions').getBoundingClientRect().top < row.querySelector('.browse-copy').getBoundingClientRect().bottom - 1),
      }));
      assert.equal(layout.overflow, false); assert.equal(layout.overlaps, false);
      assert.equal(layout.columns, layout.width > 900 ? 2 : 1);
    }
    await page.setViewportSize({ width: 1600, height: 1000 });
    if (kind === 'modpack') assert.equal(await page.getByText('New instance', { exact: true }).count(), 4);
    console.log(`PASS ${kind}: artwork retry/fallback, controls, filters, long metadata, responsive cards and actions`);
  }
  assert.deepEqual(errors, []);
} catch (error) {
  console.error(await page.locator('body').innerText(), errors);
  throw error;
} finally { await browser.close(); }
