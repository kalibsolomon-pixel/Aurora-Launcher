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

it("instance Settings has one Rename, leads with readiness, and keeps Maintenance free of duplicates", () => {
  reset();
  const view = html(Settings, { instance: instance() });
  assert.doesNotMatch(view, />\s*Rename\s*</);
  // Readiness (former Overview) owns Validate and Java checks; Maintenance no longer repeats them.
  assert.ok(view.indexOf("Readiness") < view.indexOf("Maintenance"));
  for (const duplicate of ["Validate instance", "Check managed Java", "Install / repair Java"]) assert.ok(!view.includes(duplicate), `duplicate ${duplicate}`);
  for (const label of ["Readiness", "Java runtime", "Validate", "Reinstall / restore configured content", "Recovery backup storage", "Delete instance", "Instance icon"]) {
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
  assert.match(app, /\.btn-danger \{\r?\n  border-color: transparent;\r?\n  background: var\(--color-error-soft\);/);
  const settings = readFileSync(new URL("./settings.css", import.meta.url), "utf8");
  assert.doesNotMatch(settings, /aria-current\] \{[^}]*accent-outline/);
});

it("install and Java progress use human wording with native counts only", async () => {
  const { instanceProgressLabel, runtimeProgressLabel, instanceContentStatus } = await server.ssrLoadModule("/src/lib/launcher/instanceStatus.ts");
  assert.equal(instanceProgressLabel(null), "Working");
  assert.equal(instanceProgressLabel({ phase: "resolvingGame", game: null }), "Resolving Minecraft…");
  assert.equal(instanceProgressLabel({ phase: "installingGame", game: { phase: "acquiring", completedItems: 126, totalItems: 4669, currentItem: null } }),
    "Installing Minecraft: Downloading and verifying files · 126 of 4,669");
  assert.doesNotMatch(instanceProgressLabel({ phase: "installingGame", game: { phase: "committing", completedItems: 0, totalItems: 0, currentItem: null } }), /installingGame|%|0 of 0/);
  assert.equal(runtimeProgressLabel({ phase: "acquiring", completedItems: 3, totalItems: 120, currentItem: null }), "Downloading and verifying Java · 3 of 120");
  // The lock held by this launcher's own creation reads as recovery "busy";
  // live progress wins only for that installing instance.
  const installing = { ...instance(), state: "installing", recovery: { status: "busy", message: "Installation or recovery is busy." } };
  assert.equal(instanceContentStatus(installing, undefined, "Installing Minecraft…").label, "Installing");
  assert.equal(instanceContentStatus(installing, undefined, null).label, "Installation busy");
  assert.equal(instanceContentStatus({ ...installing, recovery: { status: "interrupted", message: "x" } }, undefined, "Installing…").label, "Installation interrupted");
});

it("Java failures summarize in words and keep raw native diagnostics behind a disclosure", async () => {
  const { javaRuntimeStatus, launchProgressLabel, workspaceBadge } = await server.ssrLoadModule("/src/lib/launcher/instanceStatus.ts");
  const failed = javaRuntimeStatus(null, false, "managed runtime validation failed: C:/private/path/jvm.dll", null);
  assert.equal(failed.label, "Status failed");
  assert.doesNotMatch(failed.detail, /private|jvm/);
  assert.match(failed.technical, /jvm\.dll/);
  assert.equal(launchProgressLabel({ phase: "restoringSession" }), "Restoring the Minecraft session…");
  assert.equal(launchProgressLabel(null), null);
  const ready = { tone: "status-success", label: "Ready", detail: null };
  const blocked = { ...readiness(false), blockers: [{ code: "launch_runtime_not_ready", message: "The required managed Java runtime is not installed." }] };
  assert.equal(workspaceBadge(ready, blocked, id).label, "Needs attention");
  assert.equal(workspaceBadge(ready, blocked, "f".repeat(32)).label, "Ready");
  assert.equal(workspaceBadge(ready, readiness(true), id).label, "Ready");
  assert.equal(workspaceBadge(ready, { ...blocked, processStatus: "running" }, id).label, "Ready");
  const installing = { tone: "status-working", label: "Installing", detail: null };
  assert.equal(workspaceBadge(installing, blocked, id), installing);
});

it("first-run Home keeps the launch panel and Playtime hides range controls without data", async () => {
  reset(); launcher.launcherState = { ...launcher.launcherState, instances: [], config: { schemaVersion: 1, selectedInstanceId: null } };
  assert.match(html(Home), /class="home-composition home-empty[^"]*"[^]*No instances yet/);
  const Playtime = (await server.ssrLoadModule("/src/lib/launcher/widgets/PlaytimeWidget.svelte")).default;
  const empty = html(Playtime, { size: "small", testFixture: { summary: { todayMs: 0, last7DaysMs: 0, last30DaysMs: 0, allTimeMs: 0 }, daily: [] } });
  assert.match(empty, /class="note widget-empty[^"]*"[^>]*>No playtime recorded yet/);
  assert.doesNotMatch(empty, /Playtime range/);
  const some = html(Playtime, { size: "small", testFixture: { summary: { todayMs: 0, last7DaysMs: 60000, last30DaysMs: 60000, allTimeMs: 60000 }, daily: [] } });
  assert.match(some, /Playtime range/);
});

it("Home offers one existing command per known blocker and nothing for unknown codes", () => {
  reset({ ready: false });
  launcher.playReadiness.blockers = [
    { code: "launch_runtime_not_ready", message: "The required managed Java runtime is not installed." },
    { code: "launch_authentication_required", message: "Select a Minecraft account before playing." },
    { code: "launch_instance_damaged", message: "The selected instance failed complete content validation." },
    { code: "launch_unknown_future", message: "Something new." },
  ];
  const view = html(Home);
  for (const label of ["Install Java", "Accounts", "Open instance"]) assert.match(view, new RegExp(`>${label}</button>`));
  assert.equal((view.match(/<details/g) ?? []).length, 1);
  assert.match(view, /Something new\.<\/span>\s*<!--\[-1--><!--\]--><\/li>/);
});

it("instance icons: every suggested icon is a real Material Symbol and search is bounded", async () => {
  const { DEFAULT_INSTANCE_ICONS, FALLBACK_INSTANCE_ICON, isValidIconName, searchIcons, iconLabel } = await server.ssrLoadModule("/src/lib/instances/instanceIcons.ts");
  const { materialSymbolNames } = await server.ssrLoadModule("/src/lib/instances/materialSymbolNames.ts");
  assert.ok(materialSymbolNames.length > 3000);
  assert.ok(DEFAULT_INSTANCE_ICONS.length >= 10 && DEFAULT_INSTANCE_ICONS.length <= 20);
  for (const icon of [...DEFAULT_INSTANCE_ICONS, FALLBACK_INSTANCE_ICON]) assert.ok(materialSymbolNames.includes(icon), `${icon} is not a Material Symbol`);
  for (const bad of ["", "Rocket", "rocket-launch", "../x", "a".repeat(65)]) assert.equal(isValidIconName(bad), false);
  assert.equal(searchIcons(materialSymbolNames, "rocket launch")[0], "rocket_launch");
  assert.ok(searchIcons(materialSymbolNames, "", 50).length === 50);
  assert.equal(iconLabel("local_fire_department"), "Local fire department");
});

it("Home names the instance once: no metadata line and no name inside Launch", () => {
  reset();
  const view = html(Home);
  assert.doesNotMatch(view, /instance-metadata/);
  assert.doesNotMatch(view, /picker-name/);
  assert.equal((view.match(/Design instance/g) ?? []).length >= 1, true);
  assert.match(view, /<h1[^>]*>Design instance<\/h1>/);
  assert.match(view, /class="btn btn-primary play-hero[^"]*"[^>]*aria-label="Launch"/);
  assert.match(view, /readiness-dot/);
});

it("Instances shows tile cards with icons, and the creation form offers an icon picker", async () => {
  reset();
  const Instances = (await server.ssrLoadModule("/src/lib/pages/InstancesPage.svelte")).default;
  const view = html(Instances);
  assert.match(view, /class="instance-tiles[^"]*"/);
  assert.match(view, /class="instance-tile[^"]*selected/);
  assert.match(view, /aria-label="Open Design instance"/);
  assert.match(view, /material-symbols-outlined/);
  assert.match(view, /Instance icon: none\. Choose icon/);
  assert.doesNotMatch(view, /group-row-selected/);
  assert.doesNotMatch(view, /tile-selected|tile-badges|>Validate<|>Selected<|>Ready<|Fabric \(release version\)/);
  assert.match(view, /tile-config[^>]*>Minecraft 1\.21\.11/);
});

it("browsing modpacks inside an instance adds to that instance; only the standalone browser creates one", () => {
  const source = readFileSync(new URL("../instances/ModrinthBrowse.svelte", import.meta.url), "utf8");
  assert.match(source, /const packIntoInstance = \$derived\(!standalonePackBrowse && instanceId !== ""\)/);
  assert.match(source, /previewModrinthPackMerge\(instanceId/);
  assert.match(source, /installModrinthPackMerge\(instanceId/);
  assert.match(source, /Add to \$\{instanceName\}/);
  assert.match(source, /mergePreview\?\.blockers\.length/);
});

it("icon search understands everyday Minecraft words and browse-all starts with words, not numeric badges", async () => {
  const { searchIcons } = await server.ssrLoadModule("/src/lib/instances/instanceIcons.ts");
  const { materialSymbolNames } = await server.ssrLoadModule("/src/lib/instances/materialSymbolNames.ts");
  const tree = searchIcons(materialSymbolNames, "tree");
  assert.ok(tree.includes("forest") && tree.includes("park"), "tree finds forest and park");
  assert.ok(searchIcons(materialSymbolNames, "sword").includes("swords"));
  assert.match(materialSymbolNames[0], /^[a-z]/);
  assert.match(materialSymbolNames[materialSymbolNames.length - 1], /^[0-9]/);
});
