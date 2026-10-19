import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, it } from "node:test";
import { createServer } from "vite";

// Premium Design consolidation regressions. Real Svelte components render
// against deterministic DTO fixtures; nothing here invokes native commands.
let server: Awaited<ReturnType<typeof createServer>>;
let launcher: any;
let navigation: any;
let render: any;
let Home: any;
let Identity: any;
let Settings: any;
let Preview: any;
const id = "a".repeat(32);
const account = { accountId: "c".repeat(32), minecraftName: "PlayerName", status: "signedIn" };
function instance() {
  return { id, displayName: "Design instance", state: "ready", minecraftVersion: "1.21.11", platform: { kind: "fabric", version: "0.19.5" },
    aurora: { version: "2.1.5", channel: "stable" },
    configuration: { minecraftVersion: "1.21.11", loader: { kind: "fabric", policy: { type: "pinned", version: "0.19.5" } }, auroraEnabled: true, memoryMib: 2048, additionalJvmArguments: "", window: null } };
}
function readiness(ready: boolean) {
  return { instanceId: id, ready, accountId: account.accountId, accountName: account.minecraftName, instanceStatus: "ready", runtimeStatus: "ready",
    accountStatus: ready ? "ready" : "missing", processStatus: "stopped", blockers: [] };
}
function reset({ ready = true, signedIn = true } = {}) {
  launcher.launcherState = { config: { schemaVersion: 1, selectedInstanceId: id }, instances: [instance()], platformCapabilities: [{ kind: "fabric", canInstall: true }] };
  launcher.accountsState = signedIn ? { selectedAccountId: account.accountId, accounts: [account] } : { selectedAccountId: null, accounts: [] };
  launcher.playReadiness = readiness(ready); launcher.playProcess = null; launcher.playBusy = false; launcher.playReadinessBusy = false;
  launcher.instanceBusy = null; launcher.accountBusy = null; launcher.stateError = null; launcher.playError = null; launcher.instanceError = null;
  launcher.accountAvatars = {};
  navigation.goTo("home");
}
before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: "silent" });
  ({ launcher } = await server.ssrLoadModule("/src/lib/launcher/store.svelte.ts"));
  ({ navigation } = await server.ssrLoadModule("/src/lib/launcher/navigation.svelte.ts"));
  ({ render } = await server.ssrLoadModule("svelte/server"));
  Home = (await server.ssrLoadModule("/src/lib/pages/HomePage.svelte")).default;
  Identity = (await server.ssrLoadModule("/src/lib/launcher/AccountIdentity.svelte")).default;
  Settings = (await server.ssrLoadModule("/src/lib/instances/InstanceSettingsPanel.svelte")).default;
  Preview = (await server.ssrLoadModule("/src/lib/launcher/PlayerPreview.svelte")).default;
  launcher.refreshPlayReadiness = async () => {};
});
after(async () => { await server?.close(); });
const html = (component: any, props = {}) => render(component, { props }).body;

it("Home states a blocked or delayed launch in words, while Ready stays the quiet dot", () => {
  reset({ ready: false });
  assert.match(html(Home), /class="launch-readiness status-warning[^"]*"[^>]*>Needs attention</);
  reset({ ready: true });
  assert.doesNotMatch(html(Home), /launch-readiness/);
  launcher.playProcess = { instanceId: id, status: "running" };
  // Running is already the Launch button's own label; no duplicate line.
  assert.doesNotMatch(html(Home), /launch-readiness/);
});

it("signed-out Home offers sign-in from the skin area without nesting a control in an image", () => {
  reset({ signedIn: false });
  const view = html(Preview);
  assert.match(view, /Sign in to preview your Minecraft skin/);
  assert.match(view, /<button[^>]*class="btn btn-quiet[^"]*"[^>]*>Sign in<\/button>/);
  assert.doesNotMatch(view, /role="img"[^]*<button/);
  reset({ signedIn: true });
  assert.doesNotMatch(html(Preview), />Sign in<\/button>/);
});

it("account identity exposes its state so the rail can keep only actionable detail", () => {
  assert.match(html(Identity, { account, small: true }), /data-state="signedIn"/);
  assert.match(html(Identity, { account: { ...account, status: "reauthenticationRequired" }, small: true }), /data-state="reauthenticationRequired"[^]*Sign in again/);
  assert.match(html(Identity, { account: null, small: true }), /data-state="signedOut"[^]*Sign in to play Minecraft/);
  const css = readFileSync(new URL("../shell/phase-f.css", import.meta.url), "utf8");
  assert.match(css, /account-identity:not\(\[data-state="reauthenticationRequired"\]\) \.identity-detail \{ display: none; \}/);
});

it("instance Settings has one Rename (in the workspace header) and labeled maintenance rows", () => {
  reset();
  const view = html(Settings, { instance: instance() });
  assert.doesNotMatch(view, />\s*Rename\s*</);
  for (const label of ["Installation", "Managed Java", "Validate instance", "Check managed Java", "Install / repair Java", "Reinstall / restore configured content", "Recovery backup storage", "Delete instance"]) {
    assert.ok(view.includes(label), `missing ${label}`);
  }
  const workspace = readFileSync(new URL("../instances/InstanceWorkspace.svelte", import.meta.url), "utf8");
  assert.match(workspace, /beginRename\(instance\.id\)[^>]*>Rename</);
});

it("shared controls use exact transition properties, press feedback and unoutlined danger buttons", () => {
  const css = readFileSync(new URL("../shell/phase-f.css", import.meta.url), "utf8");
  const app = readFileSync(new URL("../../app.css", import.meta.url), "utf8");
  assert.doesNotMatch(css, /transition:\s*all\b/);
  assert.match(css, /\.f-pilot \.btn:active:not\(:disabled\)[^{]*\{ transform: scale\(\.97\); \}/);
  assert.match(css, /prefers-reduced-motion: reduce\)[^\n]*\.f-tooltip/);
  assert.match(app, /\.btn-danger \{\n  border-color: transparent;\n  background: var\(--color-error-soft\);/);
  const settings = readFileSync(new URL("./settings.css", import.meta.url), "utf8");
  assert.doesNotMatch(settings, /aria-current\] \{[^}]*accent-outline/);
});
