import assert from "node:assert/strict";
import { after, before, beforeEach, it } from "node:test";
import { createServer } from "vite";

// Exercise the actual compiled store and typed native adapter, with controlled
// command completion. No timers, network, owner files, or game processes.
let server: Awaited<ReturnType<typeof createServer>>;
let Store: any, store: any, navigation: any;
const instanceId = "a".repeat(32), accountId = "c".repeat(32), other = "b".repeat(32);
function deferred() {
  let resolve!: (value: any) => void, reject!: (reason: any) => void;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
let runtime: ReturnType<typeof deferred>[], readiness: ReturnType<typeof deferred>[];
let calls: string[], accountsFail: boolean;
const flush = () => new Promise<void>(resolve => setImmediate(resolve));
function state() {
  return { config: { selectedInstanceId: instanceId }, instances: [{ id: instanceId, state: "ready" }] };
}
function decision(extra = {}) {
  return { ready: true, instanceId, accountId, blockers: [], runtimeStatus: "ready", ...extra };
}
before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: "silent" });
  const module = await server.ssrLoadModule("/src/lib/launcher/store.svelte.ts");
  Store = module.launcher.constructor;
  ({ navigation } = await server.ssrLoadModule("/src/lib/launcher/navigation.svelte.ts"));
});
after(async () => { await server?.close(); });
beforeEach(() => {
  store = new Store(); runtime = []; readiness = []; calls = []; accountsFail = false;
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: (command: string) => {
    calls.push(command);
    if (command === "get_application_status") return Promise.resolve({});
    if (command === "get_launcher_state") return Promise.resolve(state());
    if (command === "list_aurora_releases") return Promise.resolve([]);
    if (command === "get_accounts") return accountsFail
      ? Promise.reject({ code: "accounts_unavailable", message: "Unavailable" })
      : Promise.resolve({ selectedAccountId: accountId, accounts: [] });
    if (command === "get_cached_account_avatars") return Promise.resolve([]);
    if (command === "get_instance_runtime_status" || command === "ensure_instance_runtime") {
      const request = deferred(); runtime.push(request); return request.promise;
    }
    if (command === "get_play_readiness") {
      const request = deferred(); readiness.push(request); return request.promise;
    }
    throw new Error(`Unexpected command ${command}`);
  } } };
});
async function start() {
  const completion = store.loadInitialStatus();
  await flush();
  assert.equal(runtime.length, 1); assert.equal(readiness.length, 1);
  assert(calls.indexOf("get_instance_runtime_status") < calls.indexOf("get_play_readiness"));
  return { completion };
}

for (const first of ["runtime", "readiness"] as const) {
  it(`overlapping startup, ${first} completes first: one current readiness and no Checking tail`, async () => {
    const { completion } = await start();
    assert.equal(store.playReadinessBusy, true);
    if (first === "runtime") runtime[0].resolve({ instanceId, status: "ready" });
    else readiness[0].resolve(decision());
    await flush();
    assert.equal(readiness.length, 1);
    assert.equal(store.playReadinessBusy, first === "runtime");
    if (first === "runtime") readiness[0].resolve(decision());
    else runtime[0].resolve({ instanceId, status: "ready" });
    await completion;
    assert.equal(readiness.length, 1); assert.equal(store.playReadiness.ready, true);
    assert.equal(store.playReadinessBusy, false);
  });
}
it("runtime can finish before accounts dispatch initial readiness", async () => {
  const completion = store.loadInitialStatus();
  // Resolve as soon as the runtime command is dispatched, ahead of accounts.
  while (!runtime.length) await Promise.resolve();
  runtime[0].resolve({ instanceId, status: "ready" });
  await flush();
  assert.equal(readiness.length, 1);
  readiness[0].resolve(decision()); await completion;
  assert.equal(readiness.length, 1);
});
for (const kind of ["instance", "account"] as const) {
  it(`${kind} generation changes: obsolete startup response never replaces current result`, async () => {
    const { completion } = await start();
    if (kind === "instance") store.launcherState.config.selectedInstanceId = other;
    else store.accountsState.selectedAccountId = other;
    const current = store.refreshPlayReadiness();
    readiness[1].resolve(decision(kind === "instance" ? { instanceId: other } : { accountId: other }));
    await current;
    readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" });
    await completion;
    assert.equal(readiness.length, 2);
    assert.equal(store.playReadiness[kind === "instance" ? "instanceId" : "accountId"], other);
    assert.equal(store.playReadinessBusy, false);
  });
}
it("same identities after an intervening refresh still discard the old response", async () => {
  const { completion } = await start();
  const current = store.refreshPlayReadiness();
  readiness[1].resolve(decision({ blockers: [{ code: "new-generation" }], ready: false })); await current;
  readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" }); await completion;
  assert.equal(readiness.length, 2); assert.equal(store.playReadiness.ready, false);
});
it("changed content/configuration with matching identities requires a new check", async () => {
  const { completion } = await start();
  await store.refreshState(); // Same identities, freshly loaded native state.
  readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" });
  await completion; await flush();
  assert.equal(readiness.length, 2); assert.equal(store.playReadinessBusy, true);
  readiness[1].resolve(decision({ ready: false })); await flush();
  assert.equal(store.playReadiness.ready, false);
});
it("mutation callback refresh remains authoritative while startup finishes", async () => {
  const { completion } = await start();
  await store.refreshState();
  const fresh = store.refreshPlayReadiness();
  readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" }); await completion;
  assert.equal(readiness.length, 2); assert.equal(store.playReadinessBusy, true);
  readiness[1].resolve(decision({ ready: false })); await fresh;
  assert.equal(store.playReadiness.ready, false);
});
it("runtime-status failure preserves the independent readiness result and error", async () => {
  const { completion } = await start();
  runtime[0].reject({ code: "runtime_unavailable", message: "Unavailable" });
  readiness[0].resolve(decision()); await completion; await flush();
  assert.equal(readiness.length, 1); assert.equal(store.playReadiness.ready, true);
  assert.equal(store.runtimeError.code, "runtime_unavailable");
});
for (const outcome of ["failure", "blocked", "runtimeDamaged"] as const) {
  it(`${outcome} retains the required startup follow-up`, async () => {
    const { completion } = await start();
    runtime[0].resolve({ instanceId, status: outcome === "runtimeDamaged" ? "damaged" : "ready" });
    if (outcome === "failure") readiness[0].reject({ code: "readiness_failed", message: "Unavailable" });
    else readiness[0].resolve(decision({ ready: outcome === "runtimeDamaged" }));
    await completion;
    assert.equal(readiness.length, 2); assert.equal(store.playReadinessBusy, true);
    readiness[1].resolve(decision()); await flush();
    assert.equal(store.playReadiness.ready, true);
  });
}
it("failed accounts load retains runtime-triggered readiness", async () => {
  accountsFail = true;
  const completion = store.loadInitialStatus(); await flush();
  assert.equal(readiness.length, 0);
  runtime[0].resolve({ instanceId, status: "ready" }); await completion;
  assert.equal(readiness.length, 1);
  readiness[0].resolve(decision({ ready: false, accountId: null })); await flush();
});
it("repeated refreshes always dispatch and only the latest response publishes", async () => {
  store.launcherState = state(); store.accountsState = { selectedAccountId: accountId };
  const first = store.refreshPlayReadiness(), second = store.refreshPlayReadiness(), third = store.refreshPlayReadiness();
  readiness[2].resolve(decision({ ready: false })); await third;
  readiness[1].resolve(decision()); readiness[0].reject({ code: "obsolete", message: "Unavailable" });
  await Promise.all([first, second]);
  assert.equal(readiness.length, 3); assert.equal(store.playReadiness.ready, false);
  assert.equal(store.playError, null); assert.equal(store.playReadinessBusy, false);
});
it("rapid navigation leaves the root-owned startup check intact", async () => {
  const { completion } = await start();
  for (let i = 0; i < 10; i++) {
    navigation.goTo("settings"); navigation.openInstance(instanceId); navigation.goTo("home");
  }
  readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" }); await completion;
  assert.equal(readiness.length, 1); assert.equal(store.playReadiness.ready, true);
});
it("explicit runtime checks and runtime installation each request fresh readiness", async () => {
  store.launcherState = state(); store.accountsState = { selectedAccountId: accountId };
  for (const method of ["runRuntimeStatus", "runEnsureRuntime"]) {
    const operation = store[method](instanceId);
    runtime.at(-1)!.resolve({ instanceId, status: "ready" }); await operation;
    readiness.at(-1)!.resolve(decision()); await flush();
  }
  assert.equal(readiness.length, 2);
});
it("disposing an in-flight startup discards its response and follow-up", async () => {
  const { completion } = await start(); store.dispose();
  readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" }); await completion;
  assert.equal(readiness.length, 1); assert.equal(store.playReadiness, null);
});
it("a changed process snapshot requires fresh readiness", async () => {
  const { completion } = await start();
  store.playProcess = { instanceId, status: "running" };
  readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" }); await completion;
  assert.equal(readiness.length, 2);
  readiness[1].resolve(decision({ ready: false, processStatus: "running" })); await flush();
  assert.equal(store.playReadiness.ready, false);
});
it("older runtime status cannot replace a newer status or launch another refresh", async () => {
  const { completion } = await start();
  const manual = store.runRuntimeStatus(instanceId);
  runtime[1].resolve({ instanceId, status: "damaged" }); await manual;
  readiness[1].resolve(decision({ ready: false })); await flush();
  runtime[0].resolve({ instanceId, status: "ready" }); readiness[0].resolve(decision()); await completion;
  assert.equal(store.runtimeStatus.status, "damaged");
  assert.equal(store.playReadiness.ready, false); assert.equal(readiness.length, 2);
});
it("obsolete readiness errors cannot erase a newly selected identity even before its refresh", async () => {
  store.launcherState = state(); store.accountsState = { selectedAccountId: accountId };
  const pending = store.refreshPlayReadiness();
  store.accountsState.selectedAccountId = other;
  store.playReadiness = decision({ accountId: other });
  readiness[0].reject({ code: "obsolete", message: "Unavailable" }); await pending;
  assert.equal(store.playReadiness.accountId, other); assert.equal(store.playError, null);
});
it("equal read-only startup discovery reload does not invalidate in-flight readiness", async () => {
  const { completion } = await start();
  await store.refreshState(false);
  readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" }); await completion;
  assert.equal(readiness.length, 1); assert.equal(store.playReadiness.ready, true);
});
it("read-only reload with changed DTO inputs still requires fresh readiness", async () => {
  const { completion } = await start();
  store.launcherState.instances[0].configuration = { memoryMib: 4096 };
  readiness[0].resolve(decision()); runtime[0].resolve({ instanceId, status: "ready" }); await completion;
  assert.equal(readiness.length, 2); assert.equal(store.playReadiness, null);
  readiness[1].resolve(decision()); await flush();
});
it("a mutation reload invalidates checks started during the reload as well as before it", async () => {
  store.launcherState = state(); store.accountsState = { selectedAccountId: accountId };
  const reloaded = deferred();
  const invoke = (globalThis as any).window.__TAURI_INTERNALS__.invoke;
  (globalThis as any).window.__TAURI_INTERNALS__.invoke = (command: string) => command === "get_launcher_state" ? reloaded.promise : invoke(command);
  const mutationReload = store.refreshState();
  const check = store.refreshPlayReadiness();
  reloaded.resolve(state()); await mutationReload;
  readiness[0].resolve(decision()); await check;
  assert.equal(store.playReadiness, null);
  const fresh = store.refreshPlayReadiness(); readiness[1].resolve(decision()); await fresh;
  assert.equal(store.playReadiness.ready, true);
});
