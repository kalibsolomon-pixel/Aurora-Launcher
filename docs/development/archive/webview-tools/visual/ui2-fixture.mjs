import {openReview, geometry} from './ui1-fixture.mjs';
export {geometry};

// Browser-only fixtures; no application state or native files are mutated.
export async function homeFixture(page, state = 'normal', options = {}) {
  if (page.url().startsWith('http://127.0.0.1:1422/')) await page.evaluate(() => sessionStorage.clear());
  await openReview(page, 'home', {servers: '1', ...options});
  await page.evaluate(async state => {
    const source = await (await fetch('/Review.svelte')).text();
    const module = name => source.match(new RegExp('from "([^"\\n]*' + name + '[^"\\n]*)"'))[1];
    const {launcher} = await import(module('store.svelte'));
    const {homeWidgets} = await import(module('homeLayout.svelte'));
    const {wrapReviewInvoke} = await import(module('review-core'));
    const {widgetCatalog} = await import(new URL('./widgets.ts', new URL(module('homeLayout.svelte'), location.href)).href);
    window.ui2Requests = [];
    wrapReviewInvoke(previous => async (command, args) => {
      window.ui2Requests.push({command, request: args?.request});
      if (state === 'error' && ['get_playtime_summary', 'get_recent_servers'].includes(command)) throw {code: 'fixture_unavailable', message: 'Synthetic history unavailable. Try again later.'};
      if (state === 'loading' && ['get_playtime_summary', 'get_recent_servers'].includes(command)) await new Promise(() => {});
      if (state === 'empty' && command === 'get_playtime_summary') return {allTimeMs: 0, last7DaysMs: 0, last30DaysMs: 0};
      if (state === 'empty' && command === 'get_recent_servers') return [];
      // Never start a real process, including repeated-click guard coverage.
      if (command === 'play_instance') {
        await new Promise(r => setTimeout(r, 250));
        return {instanceId: args.request.instanceId, status: 'running', exitCode: null, message: null};
      }
      return previous(command, args);
    });
    const ids = state === 'sparse' ? ['playtime'] : state === 'dense' ? widgetCatalog.map(w => w.id) : ['playtime', 'recent-servers'];
    homeWidgets.layout = {widgets: ids.map((id, i) => ({id, enabled: true, size: state === 'dense' && i === 3 ? 'wide' : state === 'dense' && i === 5 ? 'large' : 'small'}))};
    if (state === 'long') launcher.launcherState.instances[0].displayName = 'Aurora — A Very Long Instance Name With 日本語 and Adventure Worlds '.repeat(3);
    if (state === 'blocked') launcher.playReadiness = {...launcher.playReadiness, ready: false, blockers: [{code: 'auth_required', message: 'Synthetic sign-in required. Open Accounts to continue.'}]};
    if (['starting', 'running', 'failed'].includes(state)) launcher.playProcess = {instanceId: launcher.selectedInstance.id, status: state, exitCode: null, message: state === 'failed' ? 'Synthetic launch failure. Check the instance before trying again.' : null};
    if (state === 'checking') launcher.playReadinessBusy = true;
    if (state === 'missing-skin') delete launcher.accountAvatars[launcher.accountsState.selectedAccountId];
    if (state === 'unavailable') launcher.playReadiness = {...launcher.playReadiness, ready: false, blockers: [{code: 'runtime_missing', message: 'The managed Java runtime is unavailable.'}]};
  }, state);
  await page.waitForTimeout(300);
}

export async function homeGeometry(page) {
  return {...await geometry(page), ...await page.evaluate(() => {
    const box = e => {const r = e.getBoundingClientRect(); return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom};};
    return {gridColumns: getComputedStyle(document.querySelector('.widget-grid')).gridTemplateColumns,
      cards: [...document.querySelectorAll('[data-home-widget]')].map(e => ({id: e.dataset.homeWidget, ...box(e)})),
      previewDisplay: getComputedStyle(document.querySelector('.player-preview')).display,
      canvas: document.querySelector('.player-preview canvas') ? box(document.querySelector('.player-preview canvas')) : null};
  })};
}
