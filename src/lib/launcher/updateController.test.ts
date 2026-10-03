import assert from "node:assert/strict";
import { test } from "node:test";
import { UpdateController, actionDisabled, actionLabel, hasUpdate } from "./updateController.ts";
import type { ProductUpdateAvailability, UpdateOverview, ClientUpdatePreview } from "../backend.ts";

const current: ProductUpdateAvailability = { kind: "upToDate" };
const available: ProductUpdateAvailability = { kind: "updateAvailable", current: "1.0.0", candidate: "1.1.0", notes: "New features" };
const offline: ProductUpdateAvailability = { kind: "unavailable", reason: "private backend detail" };
function overview(launcher = current, client = current): UpdateOverview {
  return { launcher: { installedVersion: "1.0.0", availability: launcher, phase: null }, client,
    clientInstanceId: "instance", clientInstanceName: "Test instance", clientInstalledVersion: "1.0.0", startupCheckDone: true };
}
function preview(): ClientUpdatePreview {
  return { instanceId: "instance", installedVersion: "1.0.0", outcome: "updateAvailable", candidate: {
    version: "1.1.0", notes: null, minecraftVersion: "1.21.11", fabricLoaderVersion: "0.19.5",
    javaMajorVersion: 21, sizeBytes: 42, sha256: "a".repeat(64), fabricApiVersion: null,
  }, blockers: [], warnings: [], fingerprint: "native-fingerprint" };
}
function lab(value = overview()) {
  const calls: string[] = [];
  const operations = {
    check: async () => { calls.push("check-launcher", "check-client"); return value; },
    startup: async () => { calls.push("startup"); return value; },
    preview: async (id: string) => { assert.equal(id, "instance"); calls.push("preview"); return preview(); },
    apply: async (id: string, fingerprint: string) => { assert.equal(id, "instance"); assert.equal(fingerprint, "native-fingerprint"); calls.push("apply-client"); },
    download: async (version: string) => { assert.equal(version, "1.1.0"); calls.push("download-launcher"); },
    install: async (version: string) => { assert.equal(version, "1.1.0"); calls.push("install-launcher"); },
    afterClient: async () => { calls.push("refresh-local"); },
  };
  const controller = new UpdateController(operations, () => {});
  controller.openView();
  return { controller, operations, calls };
}
function label(controller: UpdateController): string { return actionLabel(controller.snapshot.action); }

test("A: initial action is Check For Updates", () => { assert.equal(label(lab().controller), "Check For Updates"); });
test("B/L: click checks both once; repeated checking clicks are deduplicated", async () => {
  const { controller, operations, calls } = lab();
  let finish!: (value: UpdateOverview) => void;
  operations.check = () => { calls.push("both"); return new Promise(resolve => { finish = resolve; }); };
  const first = controller.activate();
  assert.equal(label(controller), "Checking…");
  assert.equal(actionDisabled(controller.snapshot.action), true);
  const second = controller.activate();
  assert.equal(first, second);
  finish(overview()); await first;
  assert.deepEqual(calls, ["both"]); controller.closeView();
});
test("C/D/E/F: None Available is disabled for exactly five seconds; local reset performs zero checks", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const { controller, calls } = lab();
  await controller.activate(); assert.equal(label(controller), "None Available");
  assert.equal(actionDisabled(controller.snapshot.action), true);
  await controller.activate(); assert.deepEqual(calls, ["check-launcher", "check-client"]);
  t.mock.timers.tick(4999); assert.equal(label(controller), "None Available");
  t.mock.timers.tick(1); assert.equal(label(controller), "Check For Updates");
  assert.deepEqual(calls, ["check-launcher", "check-client"]);
});
test("G: leaving the view cancels its timer and prevents later mutation", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const { controller } = lab(); await controller.check(); controller.closeView();
  controller.receive(overview(available));
  t.mock.timers.tick(10000); assert.equal(label(controller), "Update");
});
test("G: a check finishing after view destruction cannot create a timer", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const { controller } = lab(); const checking = controller.check(); controller.closeView();
  await checking; assert.equal(label(controller), "Check For Updates");
  t.mock.timers.tick(5000); assert.equal(label(controller), "Check For Updates");
});
for (const [name, launcher, client] of [
  ["H/I/AF: Launcher only", available, current], ["H/J/AG: Client only", current, available],
  ["H/K/AH: both domains", available, available],
] as const) {
  test(`${name} becomes one Update action`, async () => {
    const { controller, calls } = lab(overview(launcher, client)); await controller.check();
    assert.equal(label(controller), "Update");
    assert.deepEqual(calls, ["check-launcher", "check-client"]);
  });
}
test("M/AP: manual offline check becomes retry without backend details", async () => {
  const { controller } = lab(overview(offline, offline)); await controller.check();
  assert.equal(label(controller), "Check Failed — Retry");
  assert.doesNotMatch(controller.snapshot.context, /private backend detail/);
});
test("M: rejected manual check becomes retry", async () => {
  const { controller, operations } = lab(); operations.check = async () => { throw new Error("offline"); };
  await controller.check(); assert.equal(label(controller), "Check Failed — Retry");
});
test("N/AL: failed Client apply stops Launcher mutation and retains both offers", async () => {
  const { controller, operations, calls } = lab(overview(available, available)); await controller.check(); calls.length = 0;
  operations.apply = async () => { throw new Error("bad digest"); };
  await controller.activate(); assert.equal(label(controller), "Update Failed — Retry");
  assert.deepEqual(calls, ["preview"]); assert.equal(hasUpdate(controller.snapshot.overview), true);
});
test("O: Client success returns to current local state with no extra network check", async () => {
  const { controller, calls } = lab(overview(current, available)); await controller.check(); calls.length = 0;
  await controller.activate(); assert.equal(label(controller), "Check For Updates");
  assert.equal(controller.snapshot.overview?.client.kind, "upToDate");
  assert.equal(controller.snapshot.overview?.clientInstalledVersion, "1.1.0");
  assert.equal(controller.snapshot.context, "Aurora Client updated.");
  assert.deepEqual(calls, ["preview", "apply-client", "refresh-local"]);
});
test("AC/AE: one manual check invokes both domains exactly once and both current confirms none", async () => {
  const { controller, calls } = lab(); await controller.check();
  assert.deepEqual(calls, ["check-launcher", "check-client"]);
  assert.equal(label(controller), "None Available"); controller.closeView();
});
for (const [launcher, client] of [[offline, available], [available, offline]] as const) {
  test("AD: one offline domain preserves the other domain's exact offer", async () => {
    const { controller } = lab(overview(launcher, client)); await controller.check();
    assert.equal(label(controller), "Update");
    assert.deepEqual(controller.snapshot.overview, overview(launcher, client));
    assert.match(controller.snapshot.context, /could not be checked/);
  });
}
test("AI/AJ: Client fully completes before Launcher download and install; no parallel mutations", async () => {
  const { controller, operations, calls } = lab(overview(available, available)); await controller.check(); calls.length = 0;
  let finish!: () => void;
  operations.apply = async () => { calls.push("client-start"); await new Promise<void>(resolve => { finish = resolve; }); calls.push("client-end"); };
  const updating = controller.activate();
  await Promise.resolve(); assert.equal(label(controller), "Updating…");
  assert.deepEqual(calls, ["preview", "client-start"]);
  assert.equal(actionDisabled(controller.snapshot.action), true);
  assert.equal(controller.activate(), updating);
  finish(); await updating;
  assert.deepEqual(calls, ["preview", "client-start", "client-end", "refresh-local", "download-launcher", "install-launcher"]);
  assert.doesNotMatch(controller.snapshot.context, /Everything updated/);
});
test("AK: Client success and Launcher failure keeps Client updated; retry only reruns Launcher", async () => {
  const { controller, operations, calls } = lab(overview(available, available)); await controller.check(); calls.length = 0;
  operations.download = async () => { calls.push("failed-download"); throw new Error("bad signature"); };
  await controller.activate();
  assert.equal(label(controller), "Update Failed — Retry");
  assert.equal(controller.snapshot.overview?.client.kind, "upToDate");
  assert.equal(controller.snapshot.context, "Aurora Client updated. Launcher update failed.");
  calls.length = 0; await controller.activate(); assert.deepEqual(calls, ["failed-download"]);
});
test("Launcher-only update performs download and install with one action", async () => {
  const { controller, calls } = lab(overview(available)); await controller.check(); calls.length = 0;
  await controller.activate(); assert.deepEqual(calls, ["download-launcher", "install-launcher"]);
  assert.equal(controller.snapshot.overview?.launcher.installedVersion, "1.0.0", "running version never fabricated");
});
test("Launcher install failure is truthful and retryable", async () => {
  const { controller, operations } = lab(overview(available)); await controller.check();
  operations.install = async () => { throw new Error("installer refused"); }; await controller.activate();
  assert.equal(label(controller), "Update Failed — Retry");
  assert.equal(controller.snapshot.context, "Launcher update failed.");
});
for (const mutate of [
  (value: ClientUpdatePreview) => { value.candidate!.version = "9.0.0"; },
  (value: ClientUpdatePreview) => { value.blockers = ["Game running"]; },
  (value: ClientUpdatePreview) => { value.instanceId = "other"; },
]) {
  test("stale or blocked Client preview stops both mutations", async () => {
    const { controller, operations, calls } = lab(overview(available, available)); await controller.check(); calls.length = 0;
    operations.preview = async () => { const value = preview(); mutate(value); return value; };
    await controller.activate(); assert.deepEqual(calls, []); assert.equal(label(controller), "Update Failed — Retry");
  });
}
test("AM/AZ: startup is bounded once; no-update startup is silent and creates no timer", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const { controller, calls } = lab(); await controller.startup(); await controller.startup();
  assert.equal(label(controller), "Check For Updates"); assert.equal(controller.snapshot.context, "");
  t.mock.timers.tick(60000); assert.deepEqual(calls, ["startup"]); assert.equal(hasUpdate(controller.snapshot.overview), false);
});
test("AN: startup availability is true and remains true across navigation", async () => {
  const { controller } = lab(overview(available)); await controller.startup();
  assert.equal(hasUpdate(controller.snapshot.overview), true); controller.closeView(); controller.openView();
  assert.equal(label(controller), "Update");
});
test("AO: offline startup is silent/nonfatal and manual retry remains available", async () => {
  const { controller } = lab(overview(offline, offline)); await controller.startup();
  assert.equal(label(controller), "Check For Updates"); assert.equal(controller.snapshot.context, "");
  await controller.check(); assert.equal(label(controller), "Check Failed — Retry");
});
test("AO: rejected startup is nonfatal", async () => {
  const { controller, operations } = lab(); operations.startup = async () => { throw new Error("offline"); };
  await controller.startup(); assert.equal(label(controller), "Check For Updates");
});
test("BB: an unconfigured Launcher is never reported as current", async () => {
  const { controller } = lab(overview({ kind: "unavailable", reason: "Launcher updates are not configured in this build." }));
  await controller.check(); assert.equal(label(controller), "Check Failed — Retry");
});

test("manual check during startup waits, then makes one fresh check for both domains", async () => {
  const { controller, operations, calls } = lab();
  let finish!: (value: UpdateOverview) => void;
  operations.startup = () => new Promise(resolve => { finish = resolve; });
  const startup = controller.startup();
  const manual = controller.activate();
  assert.equal(label(controller), "Checking…");
  assert.equal(controller.activate(), manual);
  finish(overview(available)); await startup; await manual;
  assert.deepEqual(calls, ["check-launcher", "check-client"]);
  assert.equal(label(controller), "None Available"); controller.closeView();
});

test("a changed Client offer makes Retry check fresh truth and requires another explicit Update", async () => {
  const { controller, operations, calls } = lab(overview(available, available));
  await controller.check(); calls.length = 0;
  operations.preview = async () => { const value = preview(); value.candidate!.version = "9.0.0"; return value; };
  await controller.activate();
  assert.equal(label(controller), "Update Failed — Retry");
  await controller.activate();
  assert.deepEqual(calls, ["check-launcher", "check-client"]);
  assert.equal(label(controller), "Update");
});
