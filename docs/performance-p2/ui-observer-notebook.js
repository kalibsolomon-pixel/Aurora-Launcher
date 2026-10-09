// Load in node_repl under the Computer Use skill. Provide sky and fs.
// Pass the current research root explicitly to p2observe(cohort, trial, root).
// One startup per call; inspect its sanitized result before a separate close.
globalThis.p2observe = async (cohort, trial, researchRoot) => {
  if (typeof researchRoot !== 'string') throw Error('provide the current research root explicitly');
  const folder = researchRoot + '/trials/' + cohort + '-' + String(trial).padStart(2, '0');
  globalThis.p2folder = folder;
  const start = JSON.parse((await fs.readFile(folder + '/start.json', 'utf8')).replace(/^\uFEFF/, ''));
  const begun = performance.now();
  let windows = [];
  while (performance.now() - begun < 10000) {
    windows = (await sky.list_windows()).filter(w => String(w.app).replaceAll('\\', '/').toLowerCase().includes('/artifacts/' + cohort + '/aurora-launcher.exe'));
    if (windows.length === 1) break;
    if (windows.length > 1) throw Error('ambiguous source-built window');
    await new Promise(resolve => setTimeout(resolve, 60));
  }
  if (windows.length !== 1) throw Error('source-built window absent');
  await sky.activate_window({ window: windows[0] });
  globalThis.p2window = windows[0];
  const observerAttachedMs = Date.now() - start.createdUnixMs;
  let first = null, stableStart = null, stableUpper = null, polls = 0, maxCaptureMs = 0, flashes = 0, previouslyReady = false;
  // Observe for at least eight seconds, including after the first hold, so
  // late runtime completion cannot be hidden by an early Ready flash/close.
  while (performance.now() - begun < 30000) {
    const before = performance.now();
    globalThis.p2state = await sky.get_window_state({ window: p2window, include_screenshot: false, include_text: true });
    globalThis.p2window = p2state.window;
    const now = performance.now();
    maxCaptureMs = Math.max(maxCaptureMs, now - before); polls++;
    const tree = String(p2state.accessibility?.tree || '');
    const ready = /button Play(?:\s|$)/.test(tree) && /Ready/.test(tree) && !/Checking/.test(tree);
    if (ready) {
      if (first === null) first = Date.now() - start.createdUnixMs;
      if (stableStart === null) stableStart = now;
      if (stableUpper === null && now - stableStart >= 1000) stableUpper = Date.now() - start.createdUnixMs;
    } else {
      if (previouslyReady) flashes++;
      stableStart = null; stableUpper = null;
    }
    previouslyReady = ready;
    if (performance.now() - begun >= 8000 && stableUpper !== null) break;
    await new Promise(resolve => setTimeout(resolve, 60));
  }
  const row = { ...start, success: stableUpper !== null, firstReadyUpperMs: first,
    stableReadyUpperMs: stableUpper, observerAttachedMs, stableHoldMs: 1000,
    observedForMs: performance.now() - begun, polls, maxCaptureMs, observedReadyChecking: flashes };
  await fs.writeFile(folder + '/ui.json', JSON.stringify(row, null, 2), { flag: 'wx' });
  nodeRepl.write({ cohort, trial, success: row.success, firstReadyUpperMs: first,
    stableReadyUpperMs: stableUpper, observedReadyChecking: flashes });
};
globalThis.p2close = async () => {
  if (!p2state?.accessibility) throw Error('observe the target before closing');
  const window = p2state.window;
  globalThis.p2state = null;
  await sky.press_key({ window, key: 'Alt_L+F4' });
  const begun = performance.now();
  while (performance.now() - begun < 5000) {
    if (!(await sky.list_windows()).some(w => w.id === window.id)) {
      await fs.writeFile(p2folder + '/close.json', JSON.stringify({ normalWindowClose: true }), { flag: 'wx' });
      nodeRepl.write({ closedWindow: true }); return;
    }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw Error('normal close not observed');
};
