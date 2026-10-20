import { after, before, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'vite';
import { categoryAtKey, settingsCategories } from './settings.ts';

let server: any, render: any, launcher: any, navigation: any, appearance: any, Settings: any, Feedback: any;
before(async () => {
  server = await createServer({ cacheDir: mkdtempSync(join(tmpdir(), 'aurora-ui5-test-')), server: { middlewareMode: true }, logLevel: 'silent' });
  ({ render } = await server.ssrLoadModule('svelte/server'));
  ({ launcher } = await server.ssrLoadModule('/src/lib/launcher/store.svelte.ts'));
  ({ navigation } = await server.ssrLoadModule('/src/lib/launcher/navigation.svelte.ts'));
  ({ appearance } = await server.ssrLoadModule('/src/lib/launcher/appearance.svelte.ts'));
  Settings = (await server.ssrLoadModule('/src/lib/pages/SettingsPage.svelte')).default;
  Feedback = (await server.ssrLoadModule('/src/lib/launcher/SettingsFeedback.svelte')).default;
});
after(async () => { await server?.close(); });
it('category keyboard movement wraps, supports Home/End, and ignores unrelated keys', () => {
  assert.equal(settingsCategories.length, 7);
  assert.equal(categoryAtKey('Appearance', 'ArrowLeft'), 'About');
  assert.equal(categoryAtKey('About', 'ArrowRight'), 'Appearance');
  assert.equal(categoryAtKey('General', 'Home'), 'Appearance');
  assert.equal(categoryAtKey('General', 'End'), 'About');
  assert.equal(categoryAtKey('General', 'Tab'), null);
});
it('global settings keep game configuration in the instance workspace', () => {
  navigation.settingsCategory = 'General';
  const html = render(Settings).body;
  assert.doesNotMatch(html, /Minecraft &amp; Java|Minecraft & Java|Open instance Settings|Saved memory/);
  for (const label of ['Add Desktop Shortcut', 'Add to Start Menu', 'Open Path']) assert.ok(html.includes(label));
});
it('Discord exposes exactly the five requested preferences', () => {
  navigation.settingsCategory = 'Discord';
  const html = render(Settings).body;
  assert.equal((html.match(/type="checkbox"/g) ?? []).length, 5);
  for (const label of ['Discord Rich Presence', 'Elapsed Play Time', 'Show World', 'Show Server', 'Show Server Address']) assert.ok(html.includes(label));
  assert.doesNotMatch(html, /Discord &amp; privacy|Platform \/ loader|Aurora Client active state|Instance name/);
});
it('About renders only running native version/platform and truthfully handles unavailable values', () => {
  navigation.settingsCategory = 'About';
  launcher.status = { launcherVersion: 'native-test-version', platform: { os: 'test-os', architecture: 'test-arch' }, managedDataRoot: 'PRIVATE-PATH' };
  const html = render(Settings).body;
  assert.match(html, /native-test-version/); assert.match(html, /test-os.*test-arch/);
  assert.doesNotMatch(html, /PRIVATE-PATH|1\.4\.2/);
  launcher.status = null; assert.match(render(Settings).body, /Version unavailable/);
});
it('system accessibility is informational, without persisted override controls', () => {
  navigation.settingsCategory = 'Accessibility'; const html = render(Settings).body;
  assert.match(html, /Reduced motion/); assert.match(html, /Reduced transparency/);
  assert.match(html, /About Reduced motion/); assert.doesNotMatch(html, /type="checkbox"|type="range"|<select/);
});
it('save feedback never claims success while pending or failed', () => {
  const props = { busy: false, error: '', saved: true, retry: () => {} };
  assert.match(render(Feedback, { props }).body, /Saved/);
  assert.doesNotMatch(render(Feedback, { props: { ...props, busy: true } }).body, /Saved\./);
  const failure = render(Feedback, { props: { ...props, error: 'Write refused' } }).body;
  assert.match(failure, /role="alert"/); assert.match(failure, /Retry save/); assert.doesNotMatch(failure, /Saved\./);
});
