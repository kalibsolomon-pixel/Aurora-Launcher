import { after, before, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "vite";

let server: any, render: any, launcher: any, homeWidgets: any, discord: any;
let Home: any, WidgetSettings: any, DiscordSettings: any, Mods: any;
let widgets: any;
const id = "a".repeat(32);
const defaults = () => ({ widgets: [
  { id: "instance-details", enabled: true, size: "small" },
  { id: "content-summary", enabled: true, size: "small" },
  { id: "session", enabled: false, size: "small" },
] });
const prefs = () => ({ enabled: false, instanceName: false, minecraftVersion: false, platform: false, auroraActive: false, elapsedTime: false, world: false, server: false, serverAddress: false });
function reset() {
  launcher.launcherState = { config: { schemaVersion: 3, selectedInstanceId: id }, instances: [{ id, displayName: "Controlled", state: "ready", minecraftVersion: "1.21.11", platform: { kind: "fabric", version: "0.19.5" }, aurora: { version: "2.1.2" }, auroraContentState: "active" }], platformCapabilities: [] };
  launcher.stateError = null; launcher.instanceError = null; launcher.playReadiness = { instanceId: id, ready: true, accountId: null, blockers: [], processStatus: "stopped" };
  launcher.accountsState = null; launcher.playProcess = null; launcher.playBusy = false; launcher.playReadinessBusy = false; launcher.accountBusy = null; launcher.instanceBusy = null; launcher.playError = null;
  launcher.modMutationBusy = null; launcher.modError = null;
  launcher.modInventories = { [id]: { instanceId: id, entries: [], missingManaged: [] } };
  launcher.contentInventories = { [`${id}:resourcePack`]: { entries: [] }, [`${id}:shaderPack`]: { entries: [] } };
  homeWidgets.layout = defaults(); homeWidgets.busy = false; homeWidgets.error = "";
  discord.state = { configured: true, connection: "ready", preferences: prefs(), gameplayCapability: true }; discord.busy = false; discord.error = "";
}
before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: "silent" });
  ({ render } = await server.ssrLoadModule("svelte/server"));
  ({ launcher } = await server.ssrLoadModule("/src/lib/launcher/store.svelte.ts"));
  ({ homeWidgets } = await server.ssrLoadModule("/src/lib/launcher/homeLayout.svelte.ts"));
  ({ discord } = await server.ssrLoadModule("/src/lib/launcher/discord.svelte.ts"));
  widgets = await server.ssrLoadModule("/src/lib/launcher/widgets.ts");
  Home = (await server.ssrLoadModule("/src/lib/pages/HomePage.svelte")).default;
  WidgetSettings = (await server.ssrLoadModule("/src/lib/launcher/HomeWidgetSettings.svelte")).default;
  DiscordSettings = (await server.ssrLoadModule("/src/lib/launcher/DiscordSettings.svelte")).default;
  Mods = (await server.ssrLoadModule("/src/lib/instances/InstanceModsPanel.svelte")).default;
});
after(async () => { await server?.close(); });
const html = (component: any, props = {}) => render(component, { props }).body;
it("default widgets sit below the unchanged primary card and 3D player", () => {
  reset(); const view = html(Home);
  assert.match(view, /Instance Details/); assert.match(view, /Content Summary/); assert.doesNotMatch(view, /Current launcher session only/);
  assert(view.indexOf("home-widgets") > view.indexOf("instance-card")); assert.match(view, /Player skin loading/);
});
it("available session is real, hidden widgets disappear, and all-off retains the edit entry point", () => {
  reset(); homeWidgets.layout.widgets[2].enabled = true; assert.match(html(Home), /Current launcher session only/);
  homeWidgets.layout.widgets[0].enabled = false; assert.doesNotMatch(html(Home), /aria-label="Instance Details"/);
  homeWidgets.layout.widgets.forEach((widget: any) => widget.enabled = false); assert.match(html(Home), /aria-label="Edit Home widgets"/);
  assert.match(html(WidgetSettings), /Show Instance Details/);
});
it("unknown and removed widget IDs stay persisted but never render components", () => {
  reset(); homeWidgets.layout.widgets.push({ id: "future-cosmetics", enabled: true, size: "wide" });
  assert.equal(widgets.registeredWidgets(homeWidgets.layout).length, 2); assert.equal(homeWidgets.layout.widgets.length, 4);
  assert.doesNotMatch(html(Home), /future-cosmetics/);
});
it("size and ordering choices are bounded and preserve hidden/unknown entries", () => {
  let layout: any = defaults(); layout.widgets.push({ id: "future", enabled: true, size: "small" });
  layout = widgets.moveWidget(layout, "content-summary", -1); assert.equal(layout.widgets[0].id, "content-summary");
  layout = widgets.setWidget(layout, "content-summary", { size: "large" }); assert.equal(layout.widgets[0].size, "large");
  assert.equal(widgets.setWidget(layout, "session", { size: "large" }), layout);
  assert.equal(layout.widgets[3].id, "future");
});
it("Settings enable/disable, reorder, resize, restart load and reset use typed native persistence", async () => {
  reset(); let saved = defaults(); const calls: string[] = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string, args: any) => {
    calls.push(command); if (command === "set_home_widgets") saved = structuredClone(args.request);
    if (command === "reset_home_widgets") saved = defaults(); return structuredClone(saved);
  } } };
  await homeWidgets.enable("instance-details", false); assert.equal(homeWidgets.layout.widgets[0].enabled, false);
  await homeWidgets.enable("instance-details", true); await homeWidgets.move("content-summary", -1); await homeWidgets.resize("content-summary", "wide");
  homeWidgets.layout = null; await homeWidgets.load(); assert.equal(homeWidgets.layout.widgets[0].size, "wide");
  await homeWidgets.reset(); assert.deepEqual(homeWidgets.layout, defaults()); assert(calls.includes("get_home_widgets"));
});
it("failed save retains the prior layout with an actionable error", async () => {
  reset(); (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async () => { throw { code: "config_malformed", message: "Repair config manually." }; } } };
  await homeWidgets.enable("instance-details", false); assert.equal(homeWidgets.layout.widgets[0].enabled, true); assert.match(homeWidgets.error, /Repair config/);
});
it("Home editor is deliberate, keyboard usable and responsive spans clamp on narrow screens", () => {
  const source = readFileSync(new URL("./HomeWidgets.svelte", import.meta.url), "utf8");
  assert.match(source, /aria-pressed=\{editing\}/); assert.match(source, /\{#if editing\}/);
  assert.match(source, /Move .* earlier/); assert.match(source, /<select aria-label/); assert.match(source, /max-width: 900px/);
});
it("Discord accurately offers Connect and Reconnect with native connection status", () => {
  reset(); assert.match(html(DiscordSettings), /Ready to connect/); assert.match(html(DiscordSettings), /Connect to Discord/); assert.doesNotMatch(html(DiscordSettings), /Link to Discord/);
  discord.state.connection = "connected"; assert.match(html(DiscordSettings), /Connected/); assert.match(html(DiscordSettings), /Reconnect to Discord/);
  discord.state.connection = "notDetected"; assert.match(html(DiscordSettings), /Discord not detected/);
});
it("missing application configuration disables connection honestly without hiding privacy choices", () => {
  reset(); discord.state.configured = false; discord.state.connection = "configurationMissing";
  const view = html(DiscordSettings); assert.match(view, /Application setup required/); assert.match(view, /disabled[^>]*>Connect to Discord/);
  assert.match(view, /Enable Discord Rich Presence/); assert.match(view, /Display Instance name/); assert.match(view, /Display Show World/); assert.match(view, /Display Show Server/); assert.match(view, /Display Show Server Address/);
});
it("Discord preferences and reconnect invoke only native commands and never disturb Play", async () => {
  reset(); const calls: string[] = []; const previous = JSON.stringify(launcher.launcherState);
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string, args: any) => {
    calls.push(command); return { ...discord.state, connection: command === "connect_discord" ? "connected" : discord.state.connection, preferences: args?.request ?? discord.state.preferences };
  } } };
  await discord.connect(); await discord.change("enabled", true); await discord.change("instanceName", true);
  assert.equal(discord.state.preferences.instanceName, true); assert.equal(discord.state.preferences.minecraftVersion, false);
  await discord.change("enabled", false); assert.equal(discord.state.preferences.enabled, false);
  assert.equal(JSON.stringify(launcher.launcherState), previous); assert(calls.every(command => ["connect_discord", "set_discord_preferences"].includes(command)));
});
it("Discord failure leaves Ready and Play available", async () => {
  reset(); (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async () => { throw new Error("Discord unavailable"); } } };
  await discord.connect(); assert(discord.error); assert.match(html(Home), /class="btn btn-primary[^>]*>Play/);
});
it("mod row preserves artwork and switch, promotes one trash, and excludes duplicate overflow removal", () => {
  reset(); launcher.modInventories[id].entries = [{ entryId: "entry", fileName: "fixture.jar", displayName: "Fixture", enabled: true, ownership: "userManaged", fileType: "enabledJar", metadata: null, provenance: null, warnings: [], canToggle: true, canRemove: true }];
  const view = html(Mods, { instance: launcher.selectedInstance });
  assert.match(view, /installed-artwork/); assert.match(view, /role="switch"/); assert.match(view, /aria-label="Remove Fixture"/);
  assert.equal((view.match(/aria-label="Remove Fixture"/g) ?? []).length, 1); assert.doesNotMatch(view, />Remove…</);
  launcher.modInventories[id].entries[0].removalBlockedReason = "Dependent requires fixture";
  assert.match(html(Mods, { instance: launcher.selectedInstance }), /Dependent requires fixture/);
});
it("trash reuses native local confirmation and provider fingerprint lifecycle", () => {
  const source = readFileSync(new URL("../instances/InstanceModsPanel.svelte", import.meta.url), "utf8");
  assert.match(source, /onclick=\{\(\) => askRemove\(entry\)\}/); assert.match(source, /beginRemoval\(entry\)/);
  assert.match(source, /launcher.runRemoveMod\(instance.id, entryId\)/); assert.match(source, /previewProviderRemoval\(targetId, "mod", entry.provenance.projectId\)/);
  assert.match(source, /applyProviderRemoval\(targetId, "mod", providerRemovalEntry.provenance.projectId, providerRemoval.previewFingerprint\)/);
  assert.match(source, /showRemoval=\{false\}/); assert.match(source, /Remove permanently/); assert.match(source, /Cancel/);
});

it("gameplay preferences are separate opt-ins and address control requires server consent", () => {
  reset(); const view=html(DiscordSettings);
  for(const label of ["Show World","Show Server","Show Server Address"]) assert.match(view,new RegExp(`aria-label="Display ${label}"`));
  assert.match(view,/aria-label="Display Show Server Address" disabled/);
  discord.state.preferences.server=true;
  assert.doesNotMatch(html(DiscordSettings),/aria-label="Display Show Server Address" disabled/);
  assert.equal(discord.state.preferences.serverAddress,false);
});
