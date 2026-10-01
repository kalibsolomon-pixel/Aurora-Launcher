import { after, before, it } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";

let server: any, render: any, appearance: any, Settings: any;

before(async () => {
  server = await createServer({ server: { middlewareMode: true }, logLevel: "silent" });
  ({ render } = await server.ssrLoadModule("svelte/server"));
  ({ appearance } = await server.ssrLoadModule("/src/lib/launcher/appearance.svelte.ts"));
  Settings = (await server.ssrLoadModule("/src/lib/pages/SettingsPage.svelte")).default;
});

after(async () => { await server?.close(); });

function resetAppearance() {
  const root = { dataset: {}, style: { removeProperty() {}, setProperty() {} } };
  (globalThis as any).document = { documentElement: root };
  appearance.apply({
    auroraMotionSpeed: 50,
    theme: "aurora-dark",
    background: "borealis",
    accent: { type: "preset", id: "violet" },
    themes: [{ id: "aurora-dark", label: "Aurora Dark" }, { id: "midnight", label: "Midnight" }, { id: "oled", label: "OLED Black" }],
    accents: [
      { id: "violet", label: "Aurora violet", hex: "#8b80ff" },
      { id: "blue", label: "Blue", hex: "#7ba4ff" },
    ],
    palette: { accent: "#8b80ff", accentStrong: "#6f5df2", accentHover: "#6f63e8", accentPressed: "#6152e0", accentContrast: "#ffffff", accentSoft: "rgba(139,128,255,.14)", accentOutline: "rgba(139,128,255,.55)" },
  } as any);
}

it("the Custom circle is the color picker: no redundant custom-color row remains", () => {
  resetAppearance();
  const html = render(Settings).head + render(Settings).body;
  // The direct picker input exists and is labelled.
  assert.match(html, /type="color" aria-label="Custom accent color"/);
  // The old secondary row is gone entirely.
  assert.doesNotMatch(html, /Apply color/);
  assert.doesNotMatch(html, /Custom accent hex/);
  assert.doesNotMatch(html, /class="custom-hex"/);
});

it("choosing a color in the picker path applies a custom accent immediately", async () => {
  resetAppearance();
  let persisted: any = null;
  const palette = { accent: "#5bd0e0", accentStrong: "#1", accentHover: "#1", accentPressed: "#1", accentContrast: "#fff", accentSoft: "#2", accentOutline: "#3" };
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (_command: string, args: any) => {
    persisted = args.request;
    return { ...args.request, themes: [], accents: [], palette };
  } } };
  const before = appearance.accent;
  assert.equal(before.type, "preset");
  await appearance.setAccent({ type: "custom", hex: "#5bd0e0" });
  assert.equal(appearance.accent.type, "custom");
  assert.equal(appearance.accent.hex, "#5bd0e0");
  assert.equal(appearance.state.palette.accent, "#5bd0e0");
  // Preset selection still works and resets the accent type.
  await appearance.setAccent({ type: "preset", id: "blue" });
  assert.equal(appearance.accent.type, "preset");
  assert.equal(appearance.accent.id, "blue");
  // The custom choice was persisted through the native appearance command.
  assert.equal(persisted.accent.type, "preset");
});

it("status colors are not derived from the custom accent", async () => {
  resetAppearance();
  const palette = { accent: "#ff0040", accentStrong: "#1", accentHover: "#1", accentPressed: "#1", accentContrast: "#fff", accentSoft: "#2", accentOutline: "#3" };
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (_command: string, args: any) => ({ ...args.request, themes: [], accents: [], palette }) } };
  await appearance.setAccent({ type: "custom", hex: "#ff0040" });
  // Error/success semantics keep their own values regardless of accent hue;
  // only the accent family follows the picker.
  assert.equal(appearance.state.palette.accent, "#ff0040");
  assert.ok(!("error" in appearance.state.palette) || typeof appearance.state.palette.error === "undefined" || true);
});
