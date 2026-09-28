import assert from "node:assert/strict";
import { after, before, it } from "node:test";
import { createServer } from "vite";

let server: Awaited<ReturnType<typeof createServer>>;
let render: any;
let Playtime: any;
let Recent: any;
let launcher: any;
before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: "silent" });
  ({ render } = await server.ssrLoadModule("svelte/server"));
  Playtime = (await server.ssrLoadModule("/src/lib/launcher/widgets/PlaytimeWidget.svelte")).default;
  Recent = (await server.ssrLoadModule("/src/lib/launcher/widgets/RecentTargetsWidget.svelte")).default;
  ({ launcher } = await server.ssrLoadModule("/src/lib/launcher/store.svelte.ts"));
  launcher.launcherState = { instances: [
    { id: "a".repeat(32), displayName: "Same name" },
    { id: "b".repeat(32), displayName: "Same name" },
  ] };
  launcher.accountsState = { selectedAccountId: "c".repeat(32) };
  launcher.playProcess = null;
  launcher.playBusy = false;
});
after(async () => { await server?.close(); });
function html(component: any, props: object): string { return render(component, { props }).body; }
const base = { todayMs: 0, last7DaysMs: (12 * 60 + 42) * 60000, last30DaysMs: 20 * 3600000, allTimeMs: 100 * 3600000 };
const daily = Array.from({ length: 30 }, (_, index) => ({ day: 20000 + index, durationMs: index * 60000 }));

it("playtime renders a textual total and seven ordered daily values", () => {
  const view = html(Playtime, { size: "small", testFixture: { summary: base, daily } });
  assert.match(view, /12h 42m/);
  assert.match(view, /Past 7 days/);
  assert.equal((view.match(/role="listitem"/g) ?? []).length, 7);
  assert.match(view, /Daily playtime for the past 7 days/);
  assert.match(view, /aria-pressed="true"/);
});
it("playtime zero state is intentional", () => {
  const view = html(Playtime, { size: "small", testFixture: { summary: { todayMs: 0, last7DaysMs: 0, last30DaysMs: 0, allTimeMs: 0 }, daily: [] } });
  assert.match(view, /No playtime recorded yet/);
  assert.doesNotMatch(view, /role="listitem"/);
});
it("recent targets render bounded rows, long labels, owning instance, and missing state", () => {
  const entries = Array.from({ length: 6 }, (_, index) => ({ id: `${index}`.repeat(64), instanceId: index % 2 ? "b".repeat(32) : "a".repeat(32), displayName: index === 0 ? "A very long world name ".repeat(10) : `World ${index}`, lastPlayedAt: 1_000_000 - index, durationMs: 1000, available: index !== 0 }));
  const compact = html(Recent, { mode: "world", size: "small", testFixture: entries });
  const expanded = html(Recent, { mode: "world", size: "large", testFixture: entries });
  assert.equal((compact.match(/<li\b/g) ?? []).length, 3);
  assert.equal((expanded.match(/<li\b/g) ?? []).length, 5);
  assert.match(compact, /Unavailable/);
  assert.match(compact, /Same name · aaaaaaaa/);
  assert.match(compact, /title="A very long world name/);
  assert.match(compact, /Quick Launch world/);
  assert.match(compact, /disabled/);
});
it("server display does not require or render a raw destination", () => {
  const entry = { id: "e".repeat(64), instanceId: "a".repeat(32), displayName: "Fixture realm", lastPlayedAt: 1_000_000, durationMs: 1000, available: true };
  const view = html(Recent, { mode: "server", size: "small", testFixture: [entry] });
  assert.match(view, /Fixture realm/);
  assert.match(view, /Quick Launch server/);
  assert.doesNotMatch(view, /fixture\.invalid:25565/);
  assert.match(html(Recent, { mode: "server", size: "small", testFixture: [] }), /No recently played servers/);
  assert.match(html(Recent, { mode: "world", size: "small", testFixture: [] }), /No recently played worlds/);
});
it("Quick Launch sends only the opaque target, kind, and account to native Play", async () => {
  const calls: Array<{ command: string; args: any }> = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string, args: any) => {
    calls.push({ command, args });
    return { instanceId: "a".repeat(32), status: "running" };
  } } };
  const targetId = "f".repeat(64);
  await launcher.runQuickPlay(targetId, "server");
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, "quick_play_history");
  assert.deepEqual(calls[0].args.request, { targetId, mode: "server", accountId: "c".repeat(32) });
  launcher.playBusy = true;
  await launcher.runQuickPlay(targetId, "server");
  assert.equal(calls.length, 1);
  launcher.playBusy = false;
});
