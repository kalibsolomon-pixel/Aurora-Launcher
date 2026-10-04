import assert from "node:assert/strict";
import { after, before, it } from "node:test";
import { createServer } from "vite";

let server: Awaited<ReturnType<typeof createServer>>;
let render: any;
let Recent: any;
let launcher: any;
before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: "silent" });
  ({ render } = await server.ssrLoadModule("svelte/server"));
  Recent = (await server.ssrLoadModule("/src/lib/launcher/widgets/RecentTargetsWidget.svelte")).default;
  ({ launcher } = await server.ssrLoadModule("/src/lib/launcher/store.svelte.ts"));
  launcher.launcherState = { instances: [{ id: "a".repeat(32), displayName: "Main instance" }] };
  launcher.accountsState = { selectedAccountId: "c".repeat(32) };
  launcher.playProcess = null;
  launcher.playBusy = false;
});
after(async () => { await server?.close(); });
function html(component: any, props: object): string { return render(component, { props }).body; }

const entry = (id: string, displayName: string) => ({
  id, instanceId: "a".repeat(32), displayName, lastPlayedAt: 1_000_000, durationMs: 1000, available: true,
});
const presentation = (targetId: string, overrides: Partial<any> = {}) => ({
  targetId, status: "online", name: null, motd: [], playersOnline: null, playersMax: null,
  versionText: null, latencyMs: null, favicon: null, ...overrides,
});

it("enriched server rows render favicon, name, MOTD and status without addresses", () => {
  const entries = [entry("e".repeat(64), "Minecraft Server")];
  const presentations = [presentation("e".repeat(64), {
    name: "Enriched Realm",
    motd: [[{ text: "Welcome ", color: "#FFAA00" }, { text: "traveler", bold: true }]],
    playersOnline: 12, playersMax: 100,
    favicon: "data:image/png;base64,aGVsbG8=",
  })];
  const view = html(Recent, { mode: "server", size: "small", testFixture: entries, testPresentations: presentations });
  assert.match(view, /Enriched Realm/);
  assert.match(view, /src="data:image\/png;base64,aGVsbG8="/);
  assert.match(view, /Welcome /);
  assert.match(view, /color:#FFAA00/);
  assert.match(view, /12\/100 online/);
  assert.match(view, /class="dot[^"]*online"/);
  assert.match(view, /Quick Launch server Enriched Realm/);
  // The raw endpoint never appears anywhere in the widget output.
  assert.doesNotMatch(view, /25565/);
  assert.doesNotMatch(view, /example\.invalid/);
});

it("rows without a presentation keep history identity and a deliberate server symbol", () => {
  const view = html(Recent, { mode: "server", size: "small", testFixture: [entry("f".repeat(64), "Plain Realm")] });
  assert.match(view, /Plain Realm/);
  assert.match(view, /class="placeholder[ "]/); assert.match(view, /<svg/); assert.doesNotMatch(view, /<img/);
  assert.doesNotMatch(view, /class="dot/);
  assert.doesNotMatch(view, /Online|Offline/);
});

it('malformed or remote favicon DTOs render the server fallback without loading a URL', () => {
  for (const favicon of ['file:///C:/image.png', 'https://example.com/favicon.png', 'data:image/svg+xml;base64,YWJj', 'data:image/png;base64,!']) {
    const id = '6'.repeat(64);
    const view = html(Recent, { mode: 'server', size: 'small', testFixture: [entry(id, 'Safe Realm')], testPresentations: [presentation(id, { favicon })] });
    assert.doesNotMatch(view, /<img/); assert.match(view, /class="placeholder[ "]/);
    assert.match(view, /Quick Launch server Safe Realm/);
  }
});

it("offline presentations retain favicon and MOTD while reporting offline", () => {
  const id = "0".repeat(64);
  const view = html(Recent, {
    mode: "server", size: "small",
    testFixture: [entry(id, "Kept Realm")],
    testPresentations: [presentation(id, {
      status: "offline", name: "Kept Realm",
      motd: [[{ text: "Cached description" }]],
      favicon: "data:image/png;base64,aGVsbG8=",
    })],
  });
  assert.match(view, /Kept Realm/);
  assert.match(view, /Cached description/);
  assert.match(view, /Offline/);
  assert.match(view, /class="dot[^"]*offline"/);
  assert.match(view, /src="data:image\/png;base64,aGVsbG8="/);
});

it("hostile MOTD text renders as text, never as markup", () => {
  const id = "1".repeat(64);
  const hostile = '<script>alert("xss")<\\/script><img src=x onerror="alert(1)">';
  const view = html(Recent, {
    mode: "server", size: "small",
    testFixture: [entry(id, "Hostile Realm")],
    testPresentations: [presentation(id, { motd: [[{ text: hostile }]] })],
  });
  // SSR escapes text content: the payload appears as literal text, and no
  // script or img element is ever constructed from server data.
  assert.match(view, /&lt;script>alert/);
  assert.doesNotMatch(view, /<script>alert/);
  assert.doesNotMatch(view, /<img/);
});

it("MOTD lines clamp to one line and long text stays bounded", () => {
  const id = "2".repeat(64);
  const long = "y".repeat(400);
  const view = html(Recent, {
    mode: "server", size: "large",
    testFixture: [entry(id, "Long Realm")],
    testPresentations: [presentation(id, { motd: [[{ text: long }], [{ text: "second line" }]] })],
  });
  // Only the first MOTD line renders; later lines exist only inside the
  // tooltip, and the row clamps rather than growing.
  const withoutTitles = view.replace(/title="[^"]*"/g, "");
  assert.match(withoutTitles, new RegExp(long));
  assert.doesNotMatch(withoutTitles, /second line/);
});

it("world mode is unchanged by the enrichment surface", () => {
  const view = html(Recent, { mode: "world", size: "small", testFixture: [entry("3".repeat(64), "Forest world")] });
  assert.match(view, /Forest world/);
  assert.doesNotMatch(view, /favicon/);
  assert.doesNotMatch(view, /class="dot/);
});

it("presentation data never reaches the Quick Launch authority", async () => {
  const calls: Array<{ command: string; args: any }> = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string, args: any) => {
    calls.push({ command, args });
    return { instanceId: "a".repeat(32), status: "running" };
  } } };
  const id = "4".repeat(64);
  // Enrichment exists for this row, then Play is triggered.
  const view = html(Recent, {
    mode: "server", size: "small",
    testFixture: [entry(id, "Authority Realm")],
    testPresentations: [presentation(id, { name: "Renamed by server" })],
  });
  assert.match(view, /Renamed by server/);
  await launcher.runQuickPlay(id, "server");
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, "quick_play_history");
  // Only the opaque history target id, kind, and account cross the boundary.
  assert.deepEqual(calls[0].args.request, { targetId: id, mode: "server", accountId: "c".repeat(32) });
});
