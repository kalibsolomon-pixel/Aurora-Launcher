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
  assert.match(render(general).body, /Add Desktop Shortcut/);
  assert.match(render(general).body, /Add to Start Menu/);
  assert.match(render(general).body, /Open Path/);
  desktop.state = state('conflict'); assert.match(render(general).body, /Another shortcut occupies this location/);
  desktop.state = state('present', false); assert.match(render(general).body, /installed production build/);
  assert.doesNotMatch(render(general).body, /Remove shortcut/);
});
it('desktop, Start menu and path actions use narrow native commands without frontend paths', async () => {
  const calls: string[] = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string, args: unknown) => {
    assert.deepEqual(args, {});
    calls.push(command); return state(command === 'remove_desktop_shortcut' ? 'absent' : 'present');
  } } };
  await desktop.refresh(); await desktop.create(); await desktop.addToStart(); await desktop.openPath(); await desktop.remove();
  assert.equal(desktop.state.desktopShortcut.state, 'absent');
  assert.deepEqual(calls, ['get_desktop_integration', 'create_desktop_shortcut', 'create_start_menu_shortcut', 'open_launcher_folder', 'remove_desktop_shortcut']);
});
it('a native refusal remains visible and never reports success', async () => {
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string) => {
    if (command === 'get_desktop_integration') return state('conflict');
    throw { code: 'shortcut_conflict', message: 'A different shortcut occupies this location.' };
  } } };
  await desktop.addToStart();
  assert.equal(desktop.error.code, 'shortcut_conflict');
  assert.equal(desktop.notice, '');
  assert.match(render(general).body, /role="alert"/);
});
