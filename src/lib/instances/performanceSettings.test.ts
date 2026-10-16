import assert from 'node:assert/strict';
import { before, after, it } from 'node:test';
import { createServer } from 'vite';
let server: any, render: any, Component: any;
const configuration = { minecraftVersion: '1.21.11', loader: { kind: 'fabric', policy: { type: 'pinned', version: '0.19.5' } }, auroraEnabled: true, memoryMib: 3217, additionalJvmArguments: '-Dlabel="two words" -Dlabel=last', window: null };
before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: 'silent' });
  ({ render } = await server.ssrLoadModule('svelte/server'));
  Component = (await server.ssrLoadModule('/src/lib/instances/PerformanceSettings.svelte')).default;
});
after(async () => { await server?.close(); });
it('all modes explain their unmeasured intent and preserve explicit heap/custom configuration', () => {
  for (const performanceMode of [undefined, 'efficient', 'balanced', 'performance', 'custom']) {
    const draft = { ...configuration, performanceMode };
    const before = structuredClone(draft);
    const html = render(Component, { props: { instance: { id: 'fixture', configuration }, draft } }).body;
    assert.match(html, /Saved maximum heap: 3217 MiB/);
    assert.match(html, /Efficient/); assert.match(html, /Balanced/); assert.match(html, /Performance/); assert.match(html, /Custom/);
    assert.match(html, /Configuration mode/); assert.match(html, /Memory \(MiB\)/);
    assert.match(html, /unmeasured/); assert.match(html, /initial heap/);
    assert.deepEqual(draft, before);
    if (performanceMode) assert.match(html, /Unsaved mode:/);
  }
});
it('pending saves disable draft controls without inventing a saved state', () => {
  const html = render(Component, { props: { instance: { id: 'fixture', configuration }, draft: { ...configuration, performanceMode: 'efficient' }, busy: true } }).body;
  assert.match(html, /disabled/); assert.match(html, /Saved mode: Existing Configuration/);
  assert.doesNotMatch(html, /Saved mode: Efficient/);
  assert.match(html, /aria-describedby="mode-description"/); assert.match(html, /aria-describedby="heap-description"/);
});
