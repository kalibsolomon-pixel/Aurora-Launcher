import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, it } from "node:test";
import { createServer } from "vite";

// Render the actual Svelte surfaces with deterministic native DTO fixtures.
// Vite supplies the same Svelte compiler and aliases the application uses.
let server: Awaited<ReturnType<typeof createServer>>;
let launcher: any;
let navigation: any;
let render: any;
let Home: any;
let Accounts: any;
let Identity: any;
let Dialog: any;
let accountManager: any;
let Conflicts: any;
let Artwork: any;
let UpdateSettings: any;
let updates: any;
const id = "a".repeat(32);
const secondId = "b".repeat(32);
function instance(platform: any = { kind: "vanilla" }, aurora: any = null) {
  return { id, displayName: "Acceptance instance", state: "ready", minecraftVersion: "1.21.11", platform, aurora,
    configuration: { minecraftVersion: "different-desired-version", loader: { kind: "fabric", policy: { type: "pinned", version: "different-desired-loader" } }, auroraEnabled: false, memoryMib: 2048, additionalJvmArguments: "", window: null } };
}
const account = { accountId: "c".repeat(32), minecraftName: "PlayerName", status: "signedIn" };
function readiness(ready = true) {
  return { instanceId: id, ready, accountId: account.accountId, accountName: account.minecraftName, instanceStatus: "ready", runtimeStatus: "ready", accountStatus: ready ? "ready" : "missing", processStatus: "stopped", blockers: ready ? [] : [{ code: "launch_account_missing", message: "Sign in to play Minecraft." }] };
}
function reset(selected = instance()) {
  launcher.launcherState = { config: { schemaVersion: 1, selectedInstanceId: selected?.id ?? null }, instances: selected ? [selected] : [], platformCapabilities: [] };
  launcher.accountsState = { selectedAccountId: account.accountId, accounts: [account] };
  launcher.playReadiness = readiness(); launcher.playProcess = null; launcher.playBusy = false; launcher.playReadinessBusy = false;
  launcher.instanceBusy = null; launcher.stateError = null; launcher.playError = null; launcher.instanceError = null;
  launcher.accountsError = null; launcher.accountError = null; launcher.signInError = null;
  launcher.signInBusy = false; launcher.accountBusy = null; launcher.accountAvatars = {}; launcher.avatarBusy = null;
  navigation.goTo("home");
}
before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: "silent" });
  ({ launcher } = await server.ssrLoadModule("/src/lib/launcher/store.svelte.ts"));
  ({ navigation } = await server.ssrLoadModule("/src/lib/launcher/navigation.svelte.ts"));
  ({ render } = await server.ssrLoadModule("svelte/server"));
  Home = (await server.ssrLoadModule("/src/lib/pages/HomePage.svelte")).default;
  Accounts = (await server.ssrLoadModule("/src/lib/pages/AccountsPage.svelte")).default;
  ({ accountManager } = await server.ssrLoadModule("/src/lib/launcher/accountManager.svelte.ts"));
  Dialog = (await server.ssrLoadModule("/src/lib/shell/AccountDialog.svelte")).default;
  Identity = (await server.ssrLoadModule("/src/lib/launcher/AccountIdentity.svelte")).default;
  Conflicts = (await server.ssrLoadModule("/src/lib/instances/InstallConflicts.svelte")).default;
  Artwork = (await server.ssrLoadModule("/src/lib/instances/InstalledArtwork.svelte")).default;
  UpdateSettings = (await server.ssrLoadModule("/src/lib/launcher/UpdateSettings.svelte")).default;
  ({ updates } = await server.ssrLoadModule("/src/lib/launcher/updates.svelte.ts"));
});
after(async () => { await server?.close(); });
function html(component: any, props = {}) { return render(component, { props }).body; }

function updateOverview(available = false) {
  const offer = available ? { kind: "updateAvailable", current: "1.0.0", candidate: "1.1.0", notes: "<script>alert('remote')</script>" } : { kind: "upToDate" };
  return { launcher: { installedVersion: "1.0.0", availability: offer, phase: null }, client: offer,
    clientInstanceId: id, clientInstanceName: "Acceptance instance", clientInstalledVersion: "1.0.0", startupCheckDone: true };
}

it("Updates renders one primary action, concise versions, and no public channels", () => {
  updates.snapshot = { action: { kind: "idle" }, overview: updateOverview(), context: "" };
  const view = html(UpdateSettings);
  assert.equal((view.match(/<button\b/g) ?? []).length, 1);
  assert.match(view, /Check For Updates/); assert.match(view, /Version 1\.0\.0/);
  assert.doesNotMatch(view, /Release channel|Stable|Beta|Nightly|type="radio"|Download update|Update in instance/);
  assert.doesNotMatch(view, /What&#39;s New|What's New/);
});
it("both-domain offers remain one Update action and secondary plain-text notes", () => {
  updates.snapshot = { action: { kind: "available" }, overview: updateOverview(true), context: "" };
  const view = html(UpdateSettings);
  assert.equal((view.match(/<button\b/g) ?? []).length, 1);
  assert.match(view, />Update<|>Update<!--/);
  assert.match(view, /Aurora Launcher 1\.0\.0 → 1\.1\.0/);
  assert.match(view, /Aurora Client 1\.0\.0 → 1\.1\.0/);
  assert.match(view, /&lt;script>/); assert.doesNotMatch(view, /<script>alert/);
});
it("no-update confirmation and mutation render a disabled primary action with honest progress", () => {
  for (const action of [{ kind: "noUpdate" }, { kind: "updating", domain: "client", phase: "updating" }]) {
    updates.snapshot = { action, overview: updateOverview(), context: "" };
    const view = html(UpdateSettings); assert.match(view, /<button[^>]*disabled/);
    if (action.kind === "noUpdate") { assert.match(view, /None Available/); assert.doesNotMatch(view, /<progress/); }
    else { assert.match(view, /<progress/); assert.doesNotMatch(view, /<progress[^>]*value=/); }
  }
});
it("Home update notice is small, silent when current or offline, and independent of Play", () => {
  reset();
  for (const kind of ["upToDate", "unavailable"]) {
    const overview = updateOverview(); overview.client = { kind };
    overview.launcher.availability = { kind };
    updates.snapshot = { action: { kind: "idle" }, overview, context: "" };
    assert.doesNotMatch(html(Home), /Aurora update available/);
  }
  updates.snapshot = { action: { kind: "available" }, overview: updateOverview(true), context: "" };
  const view = html(Home); assert.match(view, /Aurora update available/); assert.match(view, />Play</);
  assert.doesNotMatch(view, /Later|Release channel|modal.*update/i);
  updates.snapshot = { action: { kind: "idle" }, overview: null, context: "" };
});

it("Home with no selected instance offers creation and keeps local navigation", () => {
  reset(null); assert.match(html(Home), /No instances yet/); assert.match(html(Home), /Create instance/);
});

it("installation blockers render native identity, filename, ownership and reason", () => {
  const conflicts=[{modId:"fixture",fileName:"manual-fixture.jar",ownership:"userManaged",reason:"Native duplicate reason"}];
  const view=html(Conflicts,{conflicts});
  assert.match(view,/Installation blocked/);assert.match(view,/fixture.*manual-fixture.jar.*Local/);assert.match(view,/Native duplicate reason/);
  assert.equal(html(Conflicts,{conflicts:[]}).includes("Installation blocked"),false);
});
it("installed artwork failure and Local/Unknown rows preserve a nonfatal glyph", () => {
  for(const projectId of ["ABCDEFGH", null]) {
    const view = html(Artwork,{projectId,fallback:"M"});
    assert.match(view, /installed-artwork/); assert.match(view, /class="placeholder[ "]/);
    assert.match(view, /<svg/); assert.doesNotMatch(view, /<img/);
  }
});
it("Home permits selection when instances exist without a selected target", () => {
  reset(); launcher.launcherState.config.selectedInstanceId = null;
  assert.match(html(Home), /No instance selected/); assert.match(html(Home), /Select Play instance/);
});
it("Home renders installed Vanilla authority, omitting fake loader identity", () => {
  reset(); const view = html(Home);
  assert.match(view, /Minecraft 1\.21\.11 · Vanilla · Aurora Off/);
  assert.doesNotMatch(view, /different-desired|Loader: None|Fabric/);
});
it("Home renders Fabric without Aurora as a complete installed configuration", () => {
  reset(instance({ kind: "fabric", version: "0.19.5" }));
  assert.match(html(Home), /Minecraft 1\.21\.11 · Fabric 0\.19\.5 · Aurora Off/);
  assert.doesNotMatch(html(Home), /Aurora missing|Aurora required/);
});
it("Home renders Fabric and optional Aurora separately", () => {
  reset(instance({ kind: "fabric", version: "0.19.5" }, { version: "2.1.2", channel: "stable" }));
  assert.match(html(Home), /Fabric 0\.19\.5 · Aurora 2\.1\.2/);
});
it("Home Ready enables the primary Play action from native readiness", () => {
  reset(); assert.match(html(Home), /status-success[^>]*>Ready/);
  assert.match(html(Home), /class="btn btn-primary[^>]*>Play/);
});
it("Home Starting disables duplicate Play", () => {
  reset(); launcher.playProcess = { instanceId: id, status: "starting" };
  assert.match(html(Home), /class="btn btn-primary[^>]*disabled[^>]*>Starting…/);
});
it("Home Running disables duplicate Play", () => {
  reset(); launcher.playProcess = { instanceId: id, status: "running" };
  assert.match(html(Home), /class="btn btn-primary[^>]*disabled[^>]*>Running/);
});
it("Home renders native running readiness after switching back without a matching latest event", () => {
  reset(); launcher.playReadiness.processStatus = "running"; launcher.playReadiness.ready = false;
  launcher.playProcess = { instanceId: secondId, status: "exited" };
  assert.match(html(Home), /disabled[^>]*>Running/);
});
it("Home blocks Play for native authentication and validation requirements", () => {
  reset(); launcher.playReadiness = readiness(false);
  assert.match(html(Home), /disabled[^>]*>Play/); assert.match(html(Home), /Sign in to play Minecraft/);
  launcher.playReadiness.blockers = [{ code: "launch_instance_damaged", message: "Repair this instance before playing." }];
  assert.match(html(Home), /Repair this instance/);
});
it("Home ignores readiness and process belonging to another instance", () => {
  reset(); launcher.playReadiness.instanceId = secondId; launcher.playProcess = { instanceId: secondId, status: "running" };
  assert.match(html(Home), /disabled[^>]*>Play/); assert.doesNotMatch(html(Home), />Running</);
});
it("Home keeps management in Instances and typed Workspace navigation keeps selection", () => {
  reset(); assert.doesNotMatch(html(Home), /Manage Instance/);
  navigation.openInstance(id);
  assert.equal(navigation.state.kind, "instance"); assert.equal(navigation.state.instanceId, id);
  assert.equal(launcher.launcherState.config.selectedInstanceId, id);
});
it("instance selection invokes only native selection and read-only status, preserving configuration", async () => {
  reset(); const second = { ...instance({ kind: "fabric", version: "0.19.5" }), id: secondId, displayName: "Second" };
  const snapshots = JSON.stringify([launcher.launcherState.instances[0], second]);
  launcher.launcherState.instances.push(second);
  const calls: string[] = [];
  const state = structuredClone(launcher.launcherState);
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string, args: any) => {
    calls.push(command);
    if (command === "select_instance") state.config.selectedInstanceId = args.request.instanceId;
    if (command === "get_launcher_state") return state;
    if (command === "get_play_readiness") return { ...readiness(), instanceId: secondId };
    if (command === "get_instance_runtime_status") return { instanceId: secondId, status: "ready" };
  } } };
  await launcher.runSelect(secondId);
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.equal(launcher.selectedInstance.id, secondId);
  assert.equal(JSON.stringify(launcher.launcherState.instances), snapshots);
  assert(calls.includes("select_instance"));
  assert(calls.every(command => ["select_instance", "get_launcher_state", "get_instance_runtime_status", "get_play_readiness"].includes(command)));
  assert.match(html(Home), /Second/);
});
it("account dialog signed out leaves instance management available", () => {
  reset(); launcher.accountsState = { accounts: [], selectedAccountId: null };
  assert.match(html(Accounts), /No account signed in/); assert.match(html(Accounts), /still browse and manage your instances/);
});
it("signed-out Home explains native authentication requirements without management clutter", () => {
  reset(); launcher.accountsState = { accounts: [], selectedAccountId: null };
  launcher.playReadiness = { ...readiness(false), accountId: null, accountName: null };
  const view = html(Home); assert.doesNotMatch(view, /Not signed in|Manage accounts|Minecraft account/); assert.match(view, /Sign in to play Minecraft/);
  assert.match(view, /disabled[^>]*>Play/); assert.doesNotMatch(view, /Manage Instance/);
});
it("account dialog shows active Minecraft identity and management actions", () => {
  reset(); const view = html(Accounts); assert.match(view, /Active account/); assert.match(view, /PlayerName/);
  assert.match(view, />Active</); assert.match(view, /Add account/); assert.match(view, /Remove/);
});
it("account dialog with multiple identities distinguishes the active one and switch action", () => {
  reset(); launcher.accountsState.accounts.push({ ...account, accountId: secondId, minecraftName: "SecondPlayer" });
  assert.match(html(Accounts), /SecondPlayer/); assert.match(html(Accounts), /Use account/);
});
it("account switching uses Rust selection without assigning accounts to instances", async () => {
  reset(); launcher.accountsState.accounts.push({ ...account, accountId: secondId, minecraftName: "SecondPlayer" });
  const before = JSON.stringify(launcher.launcherState);
  const accounts = structuredClone(launcher.accountsState); const calls: string[] = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string, args: any) => {
    calls.push(command);
    if (command === "select_account") accounts.selectedAccountId = args.request.accountId;
    if (command === "get_accounts") return accounts;
    if (command === "get_play_readiness") return readiness();
    if (command === "get_account_avatar") return null;
  } } };
  await launcher.runSelectAccount(secondId); await new Promise(resolve => setTimeout(resolve, 0));
  assert.equal(launcher.selectedAccount.minecraftName, "SecondPlayer"); assert.equal(JSON.stringify(launcher.launcherState), before);
  assert(calls.includes("select_account")); assert(!calls.some(command => /instance|configuration/.test(command)));
});
it("cosmetic avatar failure leaves the model region empty without disabling native Ready", async () => {
  reset(); (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async () => { throw new Error("offline"); } } };
  await launcher.refreshAvatar(account.accountId, true);
  assert.match(html(Home), /Player skin loading/); assert.doesNotMatch(html(Home), /Default Minecraft player/); assert.match(html(Home), /class="btn btn-primary[^>]*>Play/);
  assert.equal(launcher.playReadiness.ready, true); assert.equal(launcher.playError, null);
});
it("invalid avatar pixels use a fallback and valid pixels produce the head canvas", () => {
  reset(); launcher.accountAvatars[account.accountId] = { rgba: [0], model: "classic" };
  assert.match(html(Identity, { account }), /Default account avatar/);
  launcher.accountAvatars[account.accountId] = { rgba: Array(256).fill(255), model: "slim" };
  assert.match(html(Identity, { account }), /Minecraft head for PlayerName/); assert.match(html(Identity, { account }), /<canvas/);
});
it("account identity preserves a long username with accessible full text", () => {
  reset(); const long = { ...account, minecraftName: "LongUsername".repeat(10) };
  assert.match(html(Identity, { account: long }), new RegExp(`title="${long.minecraftName}"`));
});
it("late avatar completion cannot replace the newly selected account's image", async () => {
  reset(); let finish: (value: any) => void;
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: (command: string, args: any) => args.request.accountId === account.accountId ? new Promise(resolve => finish = resolve) : Promise.resolve(null) } };
  const pending = launcher.refreshAvatar(account.accountId);
  await launcher.refreshAvatar(secondId);
  finish!({ rgba: Array(256).fill(255), model: "classic" }); await pending;
  assert.equal(launcher.accountAvatars[account.accountId], undefined);
  assert.equal(launcher.avatarBusy, null);
});
it("late readiness cannot overwrite a newer selected instance decision", async () => {
  reset(); let finish: (value: any) => void;
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: (_command: string, args: any) => args.request.instanceId === id ? new Promise(resolve => finish = resolve) : Promise.resolve({ ...readiness(), instanceId: secondId }) } };
  const pending = launcher.refreshPlayReadiness();
  launcher.launcherState.config.selectedInstanceId = secondId;
  await launcher.refreshPlayReadiness();
  finish!(readiness()); await pending;
  assert.equal(launcher.playReadiness.instanceId, secondId); assert.equal(launcher.playReadinessBusy, false);
});
it("Play invokes the established native command once for the exact selected target", async () => {
  reset(); const calls: any[] = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string, args: any) => { calls.push({command, args}); return {instanceId: id, status: "running"}; } } };
  await launcher.runPlay(id);
  assert.equal(calls.length, 1); assert.equal(calls[0].command, "play_instance");
  assert.equal(calls[0].args.request.instanceId, id); assert.equal(calls[0].args.request.accountId, account.accountId);
  launcher.playBusy = true;
  await launcher.runPlay(id); assert.equal(calls.length, 1);
});
it("Play rejects stale instance or account display decisions before invoking native launch", async () => {
  reset(); const calls: any[] = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string) => calls.push(command) } };
  await launcher.runPlay(secondId);
  launcher.accountsState.selectedAccountId = secondId;
  await launcher.runPlay(id); assert.equal(calls.length, 0);
});
it("a large instance list and long names retain accessible options and selected marker", () => {
  reset(); launcher.launcherState.instances = Array.from({length: 80}, (_, index) => ({ ...instance(), id: index === 0 ? id : String(index), displayName: `Long instance name ${index} `.repeat(5) }));
  const view = html(Home); assert.equal((view.match(/role="option"/g) ?? []).length, 80);
  assert.match(view, /Long instance name 79/); assert.match(view, /selected-marker/); assert.match(view, />Selected</);
});

it("shell account modal opens and closes without changing Home, Settings or workspace navigation", () => {
  reset();
  for (const open of [() => navigation.goTo("home"), () => navigation.goTo("settings"), () => navigation.openInstance(id)]) {
    open(); const before=JSON.stringify(navigation.state);
    accountManager.show(); assert.equal(accountManager.open,true);
    assert.equal(JSON.stringify(navigation.state),before);
    assert.match(html(Dialog), /<dialog[^>]*aria-labelledby="account-dialog-title"/);
    assert.match(html(Dialog), /Close accounts/); assert.match(html(Dialog), /Add account/);
    accountManager.close(); assert.equal(accountManager.open,false); assert.equal(JSON.stringify(navigation.state),before);
  }
});
it("signed-out account dialog contains Microsoft sign-in without changing the route", () => {
  reset(); launcher.accountsState={accounts:[],selectedAccountId:null}; accountManager.show();
  assert.match(html(Dialog), /Sign in with Microsoft/); assert.equal(navigation.state.page,"home"); accountManager.close();
});
it("Home integrates its picker within the selected-instance card and has no account management row", () => {
  reset(); const view=html(Home);
  assert.match(view, /aria-label="Selected instance"[\s\S]*Select Play instance/);
  assert.doesNotMatch(view, /Play instance<|Manage accounts|Minecraft account|PlayerName/);
  assert.match(view, /Player skin loading/);
  const card=view.slice(view.indexOf('aria-label="Selected instance"'),view.indexOf('</section>',view.indexOf('aria-label="Selected instance"')));
  assert.doesNotMatch(card,/player-preview|<canvas/);
  assert.doesNotMatch(view,/width="560" height="800"/);
  launcher.accountAvatars[account.accountId]={rgba:Array(256).fill(255),model:"classic",skinHeight:64,skinRgba:Array(16384).fill(255)};
  const skinned = html(Home);
  assert.match(skinned, /Current Minecraft player skin/);
  assert.match(skinned, /width="560" height="800"/);
});

it("Add account and cancellation use the existing native commands and preserve identity", async () => {
  reset(); const calls:string[]=[]; const state=structuredClone(launcher.accountsState);
  (globalThis as any).window={__TAURI_INTERNALS__:{invoke:async (command:string) => {
    calls.push(command); if(command === "get_accounts") return state;
    if(command === "get_play_readiness") return readiness();
    if(command === "get_account_avatar") return null;
  }}};
  await launcher.runSignIn(); await launcher.runCancelSignIn();
  assert(calls.includes("begin_microsoft_login")); assert(calls.includes("cancel_microsoft_login"));
  assert.equal(launcher.selectedAccount.accountId,account.accountId);
  assert(!calls.some(command => /remove|install|play_instance|select_instance/.test(command)));
});

it("the bottom-left chip opens the shell modal rather than inventing account navigation", () => {
  const shell=readFileSync(new URL("../shell/AppShell.svelte",import.meta.url),"utf8");
  assert.match(shell,/class="account-chip"[\s\S]*onclick=\{\(\) => accountManager.show\(\)\}/);
  assert.match(shell,/aria-haspopup="dialog"/); assert.match(shell,/<AccountDialog \/>/);
  assert.doesNotMatch(shell,/navigation.goTo\("accounts"\)/);
});
