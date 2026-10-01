import assert from "node:assert/strict";
import { after, before, it } from "node:test";
import { createServer } from "vite";

let server: Awaited<ReturnType<typeof createServer>>;
let render: any, Skin: any, Cape: any, launcher: any, cosmetics: any, backend: any;
const a = "a".repeat(32), b = "b".repeat(32);
const presets = Array.from({ length: 6 }, (_, i) => ({ id: `preset-${i}`, name: i === 0 ? "Very long saved skin ".repeat(8) : `Saved ${i}`, model: i % 2 ? "slim" : "classic", importedAt: 1, sha256: String(i).padStart(64, "0") }));
function account(id: string | null) {
  launcher.accountsState = { selectedAccountId: id, accounts: id ? [{ accountId: id, minecraftName: "Player", status: "signedIn" }] : [] };
  cosmetics.accountId = id;
}
function state(id = a, active = true, count = 2) {
  return { accountId: id, currentSkinModel: "classic", hasCurrentSkin: true, capes: Array.from({ length: count }, (_, i) => ({ id: `cape-${i}`, name: `Cape ${i}`, selected: active && i === 0 })) };
}
function html(component: any, size: string) { return render(component, { props: { size } }).body; }
before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: "silent" });
  ({ render } = await server.ssrLoadModule("svelte/server"));
  Skin = (await server.ssrLoadModule("/src/lib/launcher/widgets/SkinManagerWidget.svelte")).default;
  Cape = (await server.ssrLoadModule("/src/lib/launcher/widgets/CapeSelectorWidget.svelte")).default;
  ({ launcher } = await server.ssrLoadModule("/src/lib/launcher/store.svelte.ts"));
  ({ cosmetics } = await server.ssrLoadModule("/src/lib/launcher/cosmetics.svelte.ts"));
  backend = await server.ssrLoadModule("/src/lib/backend.ts");
});
after(async () => { await server?.close(); });
it("skin library renders current state, bounded compact rows, and expansion", () => {
  account(a); cosmetics.remote = state(); cosmetics.presets = presets; cosmetics.loading = false; cosmetics.offline = false; cosmetics.error = "";
  const compact = html(Skin, "small"), expanded = html(Skin, "large");
  assert.match(compact, /Current skin · Classic/);
  assert.match(compact, /Import skin/);
  assert.match(compact, /aria-label="Import skin model"/);
  assert.match(compact, /value="slim"/);
  assert.equal((compact.match(/<li\b/g) ?? []).length, 3);
  assert.equal((expanded.match(/<li\b/g) ?? []).length, 6);
  assert.match(compact, /View library \(6 saved\)/);
  assert.match(compact, /title="Very long saved skin/);
  assert.match(compact, /Manage saved skin/);
  // Management actions stay behind per-row disclosure, not in the collapsed row.
  assert.doesNotMatch(compact, /Remove saved skin/);
});
it("skin no-account and offline state keep local presets browsable", () => {
  account(null); cosmetics.remote = null;
  assert.match(html(Skin, "small"), /Choose a Minecraft account/);
  account(a); cosmetics.offline = true;
  const view = html(Skin, "small");
  assert.match(view, /Saved skins remain available/);
  assert.match(view, /Saved 1/);
  assert.match(view, /disabled/);
});
it("current-skin association is proven by content hash", () => {
  account(a); cosmetics.remote = state(); cosmetics.presets = presets; cosmetics.loading = false; cosmetics.offline = false; cosmetics.error = "";
  launcher.accountAvatars[a] = { rgba: Array(256).fill(120), model: "classic", skinRgba: [], skinHeight: 64, sha256: presets[2].sha256 };
  const view = html(Skin, "large");
  assert.match(view, /In library/);
  assert.equal((view.match(/>Current</g) ?? []).length, 1);
  launcher.accountAvatars[a] = null;
});
it("cape widget renders owned, selected, disabled, empty, and offline states", () => {
  account(a); cosmetics.remote = state(); cosmetics.offline = false; cosmetics.error = "";
  assert.match(html(Cape, "small"), /Cape 0 · Active/);
  assert.match(html(Cape, "small"), /Select Cape 1 cape/);
  assert.match(html(Cape, "small"), /Disable active cape/);
  cosmetics.remote = state(a, false, 1);
  assert.match(html(Cape, "small"), /Cape 0/);
  cosmetics.remote = state(a, false, 0);
  assert.match(html(Cape, "small"), /No capes are reported/);
  cosmetics.offline = true;
  assert.match(html(Cape, "small"), /Cape changes are paused/);
});
it("loading, error, expanded capes, and decoded-preview label stay readable", () => {
  account(a); cosmetics.remote = null; cosmetics.loading = true; cosmetics.error = "";
  assert.match(html(Skin, "wide"), /Loading current skin/);
  assert.match(html(Cape, "small"), /Loading owned capes/);
  cosmetics.loading = false; cosmetics.error = "Service is rate limiting";
  cosmetics.remote = state(a, true, 6);
  cosmetics.remote.capes[0].preview = { width: 64, height: 32, rgba: Array(64 * 32 * 4).fill(0) };
  const compact = html(Cape, "small"), expanded = html(Cape, "large");
  assert.equal((compact.match(/<li\b/g) ?? []).length, 3);
  assert.equal((expanded.match(/<li\b/g) ?? []).length, 6);
  assert.match(compact, /Cape 0 cape preview/);
  assert.match(compact, /Service is rate limiting/);
});
it("account switch never renders the prior account's cape list", () => {
  account(a); cosmetics.remote = state(a);
  account(b);
  assert.doesNotMatch(html(Cape, "small"), /Cape 0/);
});
it("semantic native DTOs carry no token or user-selected filesystem path", async () => {
  const calls: Array<{ name: string; args: any }> = [];
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (name: string, args: any) => { calls.push({ name, args }); return state(); } } };
  await backend.applySkinPreset(a, "preset-id", "slim");
  await backend.selectOwnedCape(a, "owned-cape");
  await backend.disableOwnedCape(a);
  await backend.importSkinPreset("Name", "classic", [1, 2, 3]);
  await backend.updateSkinPreset("preset-id", { name: "New", model: "slim" });
  await backend.skinPresetThumbnail("preset-id");
  await backend.saveCurrentSkin(a);
  assert.deepEqual(calls.map(call => call.name), [
    "apply_skin_preset", "select_cape", "disable_cape", "import_skin_preset",
    "update_skin_preset", "skin_preset_thumbnail", "save_current_skin",
  ]);
  assert.deepEqual(calls[0].args.request, { accountId: a, presetId: "preset-id", model: "slim" });
  assert.deepEqual(calls[1].args.request, { accountId: a, capeId: "owned-cape" });
  assert.deepEqual(calls[2].args.request, { accountId: a });
  assert.deepEqual(calls[3].args.request, { name: "Name", model: "classic", bytes: [1, 2, 3] });
  assert.deepEqual(calls[4].args.request, { presetId: "preset-id", changes: { name: "New", model: "slim" } });
  assert.deepEqual(calls[5].args.request, { presetId: "preset-id" });
  assert.deepEqual(calls[6].args.request, { accountId: a });
  assert.doesNotMatch(JSON.stringify(calls), /token|filePath|Authorization/i);
});
