// UI-1 browser-only presentation data. This entry never ships in the app.
export const base = 'http://127.0.0.1:1422/';
export async function openReview(page, surface = 'home', options = {}) {
  const query = new URLSearchParams({l3: '', reduced: '1', background: 'borealis', ...options});
  if (surface === 'mods') query.set('tab', 'mods');
  else if (surface !== 'home') query.set('page', surface);
  await page.goto(base + '?' + query);
  await page.locator('.account-chip').waitFor();
  await page.evaluate(async () => {
    const source = await (await fetch('/Review.svelte')).text();
    const module = name => source.match(new RegExp('from "([^"\\n]*' + name + '[^"\\n]*)"'))[1];
    const {launcher} = await import(module('store.svelte'));
    const {wrapReviewInvoke} = await import(module('review-core'));
    window.ui1Requests = [];
    window.ui1Mode = 'normal';
    const names = ['Sodium', 'Lithium', 'Iris Shaders', 'Mod Menu', 'FerriteCore', 'ImmediatelyFast', 'Continuity', 'Entity Culling', 'Reese’s Sodium Options', 'Dynamic FPS', 'AppleSkin', 'Very Long Mod Title — Better Adventures and Rendering Options for Every World'];
    const titles = [...names, ...names.map(name => `${name} Extras`)];
    const hits = titles.map((title, i) => ({projectId: 'UI' + String(i).padStart(6, '0'), title, summary: ['Improves frame rates with a modern rendering engine.', 'Optimizes game logic while preserving vanilla behavior.', 'Adds shader support for a more atmospheric Minecraft.'][i % 3], author: 'Synthetic review', downloads: 1200000 + i * 120300, categories: [i % 2 ? 'utility' : 'optimization'], projectType: 'mod'}));
    const images = hits.map((_, i) => {
      const c = document.createElement('canvas'); c.width = c.height = 64;
      const g = c.getContext('2d'); g.fillStyle = `hsl(${210 + i * 26} 40% 35%)`; g.fillRect(0, 0, 64, 64);
      g.fillStyle = '#edf1fb'; g.font = '600 28px system-ui'; g.fillText(titles[i][0], 21, 42); return c.toDataURL();
    });
    wrapReviewInvoke(previous => async (command, args) => {
      window.ui1Requests.push({command, request: args?.request});
      if (command === 'resolve_project_artwork') return {source: images[Number(args.request.projectId.slice(2)) % hits.length], status: 'available', retryAfterMs: null};
      if (command === 'browse_modrinth' && args.request.contentType === 'mod') {
        if (window.ui1Mode === 'loading') await new Promise(r => setTimeout(r, 1200));
        if (window.ui1Mode === 'error') throw {code: 'provider_network_error', message: 'Synthetic provider unavailable. Try searching again.'};
        let found = hits.filter(h => h.title.toLowerCase().includes(args.request.search.toLowerCase()) && (!args.request.categories.length || args.request.categories.some(c => h.categories.includes(c))));
        if (args.request.sort === 'downloads') found = found.toReversed();
        const offset = args.request.offset ?? 0;
        return {offset, totalHits: found.length, hits: found.slice(offset, offset + 20)};
      }
      if (command === 'quick_install_modrinth') throw {code: 'provider_network_error', message: 'Synthetic install failure; no files were downloaded or installed.'};
      return previous(command, args);
    });
    // One installed direct project exercises disabled state without a mutation.
    const entries = launcher.modInventories['a'.repeat(32)].entries;
    entries[1].provenance = {provider: 'modrinth', projectId: hits[0].projectId};
  });
  if (surface === 'mods') {
    await page.getByRole('button', {name: 'Browse Modrinth', exact: true}).click();
    await page.locator('.browse-row').first().waitFor();
  }
  await page.locator('main.content').evaluate(e => e.scrollTop = 0);
  await page.waitForTimeout(250);
}

export async function geometry(page) {
  return page.evaluate(() => {
    const rect = selector => {
      const e = document.querySelector(selector); if (!e) return null;
      const r = e.getBoundingClientRect();
      return {x: r.x, y: r.y, width: r.width, height: r.height, right: r.right, bottom: r.bottom};
    };
    const content = document.querySelector('main.content');
    return {viewport: [innerWidth, innerHeight], dpr: devicePixelRatio, documentOverflow: document.documentElement.scrollWidth > innerWidth, contentOverflow: content.scrollWidth > content.clientWidth + 1, rail: rect('.sidebar'), account: rect('.account-chip'), content: rect('main.content'), home: rect('.home-composition'), play: rect('.play-hero'), instance: rect('.instance-card'), player: rect('.player-preview'), widgets: rect('.home-widgets'), workspace: rect('.workspace-page'), search: rect('.browse-search input'), searchButton: rect('.browse-search .btn'), category: rect('.category-trigger'), firstResult: rect('.browse-row'), resultColumns: document.querySelector('.browse-list') ? getComputedStyle(document.querySelector('.browse-list')).gridTemplateColumns : null, filters: rect('.browse-filters'), metadataSize: document.querySelector('.browse-meta') ? getComputedStyle(document.querySelector('.browse-meta')).fontSize : null};
  });
}
