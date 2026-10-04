import { after, before, it } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'vite';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

let server: any, render: any, general: any, desktop: any, navigation: any;
const state = (status = 'present', manageable = true) => ({ supported: true, manageable,
  desktopShortcut: { state: status, managedBy: 'aurora' }, startMenuShortcut: { state: 'present', managedBy: 'installer' } });
before(async () => {
  // This new SSR suite must not invalidate the other workers' shared Vite pre-bundle.
  server = await createServer({ cacheDir: mkdtempSync(join(tmpdir(), 'aurora-general-test-')), server: { middlewareMode: true }, logLevel: 'silent' });
  ({ render } = await server.ssrLoadModule('svelte/server'));
  ({ desktopIntegration: desktop } = await server.ssrLoadModule('/src/lib/launcher/desktopIntegration.svelte.ts'));
  ({ navigation } = await server.ssrLoadModule('/src/lib/launcher/navigation.svelte.ts'));
  general = (await server.ssrLoadModule('/src/lib/launcher/GeneralSettings.svelte')).default;
});
after(async () => { await server?.close(); delete (globalThis as any).window; });
it('General starts with live desktop conveniences, honoring native capability and conflicts', () => {
  assert.equal(navigation.settingsCategory, 'General');
  desktop.error = null; desktop.busy = false; desktop.state = state();
  assert.match(render(general).body, /Remove shortcut/);
  desktop.state = state('absent'); assert.match(render(general).body, /Create shortcut/);
  desktop.state = state('conflict'); assert.doesNotMatch(render(general).body, /(?:Create|Remove) shortcut/);
  desktop.state = state('present', false); assert.match(render(general).body, /installed production build/);
  assert.doesNotMatch(render(general).body, /Remove shortcut/);
  desktop.state = { ...state(), supported: false }; assert.match(render(general).body, /available on Windows/);
});
it('refresh/create/remove use only the existing native boundary without saving preferences', async () => {
  const calls: string[] = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string) => {
    calls.push(command); return state(command === 'remove_desktop_shortcut' ? 'absent' : 'present');
  } } };
  await desktop.refresh(); await desktop.create(); await desktop.remove();
  assert.equal(desktop.state.desktopShortcut.state, 'absent');
  assert.deepEqual(calls, ['get_desktop_integration', 'create_desktop_shortcut', 'remove_desktop_shortcut']);
});
