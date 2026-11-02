import assert from 'node:assert/strict';
import {mkdir, writeFile} from 'node:fs/promises';
import {openContentReview} from './ui3-fixture.mjs';
import {openReview} from './ui1-fixture.mjs';
import {homeFixture} from './ui2-fixture.mjs';

const {chromium} = await import(process.env.AURORA_PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({headless: true});
const page = await browser.newPage({viewport: {width: 1280, height: 800}, reducedMotion: 'reduce'});
const errors = [], results = [];
page.on('pageerror', error => errors.push(error.message));
await page.route(/^https?:\/\/(?!127\.0\.0\.1:1422)/, route => route.abort());
const out = 'docs/ui-3-accent-fix';
const accents = [
  [{type: 'custom', hex: '#ff0000'}, '#ff0000'],
  [{type: 'preset', id: 'violet'}, '#8b80ff'],
  [{type: 'preset', id: 'cyan'}, '#5bd0e0'],
];
async function applyAccent(accent) {
  await page.evaluate(async accent => {
    const source = await (await fetch('/Review.svelte')).text();
    const {appearance} = await import(source.match(/from "([^"\n]*appearance.svelte[^"\n]*)"/)[1]);
    await appearance.setAccent(accent);
    if (appearance.error) throw Error(appearance.error.message);
  }, accent);
}
const rgb = hex => `rgb(${[1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16)).join(', ')})`;
await mkdir(out, {recursive: true});
try {
  for (const background of ['simple', 'borealis']) {
    for (const surface of ['installed-mods', 'mods', 'installed-packs', 'packs', 'modpacks']) {
      await openContentReview(page, surface, {theme: 'oled', background});
      for (const [accent, hex] of accents) {
        await applyAccent(accent);
        const selectors = surface === 'modpacks' ? [] : ['.workspace-tab-active', '.content-view-tabs [aria-pressed="true"]'];
        if (['mods', 'packs'].includes(surface)) selectors.push('.browse-kind-tabs [aria-pressed="true"]');
        assert.equal(await page.locator('.rail-link[aria-current="page"]').evaluate(e => getComputedStyle(e).color), rgb(hex));
        const styles = await page.evaluate(selectors => selectors.map(selector => {
          const element = document.querySelector(selector);
          if (!element) throw Error(`Missing selected tab: ${selector}`);
          const style = getComputedStyle(element);
          return {selector, shadow: style.boxShadow, background: style.backgroundColor, color: style.color};
        }), selectors);
        for (const style of styles) {
          assert.equal(style.shadow, `${rgb(hex)} 0px -3px 0px 0px inset`, JSON.stringify(style));
          assert.equal(style.background, styles[0].background, 'Selected bars share the same accent tint');
        }
        results.push({background, surface, accent, styles});
        if (background === 'simple' && surface === 'installed-mods' && accent.type === 'custom') {
          await page.screenshot({path: `${out}/red-selected-tabs.png`});
        }
      }
    }
    for (const surface of ['home', 'instances', 'settings', 'cosmetics']) {
      if (surface === 'home') await homeFixture(page, 'normal', {theme: 'oled', background});
      else await openReview(page, surface, {theme: 'oled', background});
      for (const [accent, hex] of accents) {
        await applyAccent(accent);
        const colorSelectors = ['.rail-link[aria-current="page"]'];
        if (surface === 'home') colorSelectors.push('.range .active', '.history-tabs [aria-pressed="true"]');
        for (const selector of colorSelectors) {
          const colors = await page.locator(selector).evaluateAll(elements => elements.map(e => getComputedStyle(e).color));
          assert.ok(colors.length, `${surface}: ${selector}`);
          assert.ok(colors.every(color => color === rgb(hex)), `${surface}: ${selector}: ${colors}`);
        }
        if (surface === 'settings') {
          assert.ok((await page.locator('.settings-nav [aria-current]').evaluate(e => getComputedStyle(e).boxShadow)).includes(rgb(hex)));
        }
        if (surface === 'instances' || surface === 'cosmetics') {
          const selector = surface === 'instances' ? '.group-row-selected' : '.cosmetics-tabs [aria-selected="true"]';
          const backgroundColor = await page.locator(selector).first().evaluate(e => getComputedStyle(e).backgroundColor);
          assert.ok(backgroundColor.startsWith(rgb(hex).replace('rgb(', 'rgba(').replace(')', ',')), backgroundColor);
        }
        results.push({background, surface, accent});
        if (background === 'simple' && accent.type === 'custom') {
          await page.screenshot({path: `${out}/red-${surface}.png`});
        }
      }
    }
    await openReview(page, 'home', {tab: 'overview', theme: 'oled', background});
    for (const tab of ['overview', 'mods', 'resourcePacks', 'shaders', 'settings']) {
      await page.evaluate(async tab => {
        const source = await (await fetch('/Review.svelte')).text();
        const {navigation} = await import(source.match(/from "([^"\n]*navigation.svelte[^"\n]*)"/)[1]);
        navigation.openInstance('a'.repeat(32), tab);
      }, tab);
      for (const [accent, hex] of accents) {
        await applyAccent(accent);
        assert.ok((await page.locator('.workspace-tab-active').evaluate(e => getComputedStyle(e).boxShadow)).includes(rgb(hex)), tab);
        results.push({background, surface: `workspace-${tab}`, accent});
      }
    }
  }
  assert.deepEqual(errors, []);
  await writeFile(`${out}/results.json`, JSON.stringify({cases: results.length, results, pageErrors: errors, provenance: 'Actual components; deterministic browser fixtures; no native or owner-data mutation.'}, null, 2));
  console.log(`PASS ${results.length} live accent/page combinations across Home, Instances, Settings, Cosmetics, every workspace tab, Library and Discover; no page errors.`);
} finally {
  await browser.close();
}
