import {
  applySkinPreset, disableOwnedCape, getCosmetics, importSkinPreset,
  listSkinPresets, removeSkinPreset, selectOwnedCape,
  type CosmeticsState, type SkinModel, type SkinPreset,
} from "$lib/backend";
import { launcher } from "./store.svelte";

function message(cause: unknown): string { return cause instanceof Error ? cause.message : "Minecraft cosmetics are unavailable."; }
class CosmeticsStore {
  presets = $state<SkinPreset[]>([]);
  remote = $state<CosmeticsState | null>(null);
  accountId = $state<string | null>(null);
  loading = $state(false);
  busy = $state(false);
  offline = $state(false);
  error = $state("");
  private serial = 0;
  private presetsLoaded = false;

  async loadPresets(): Promise<void> {
    if (this.presetsLoaded) return;
    try { this.presets = await listSkinPresets(); this.presetsLoaded = true; }
    catch (cause) { this.error = message(cause); }
  }
  async load(accountId: string | null, force = false): Promise<void> {
    void this.loadPresets();
    if (!force && this.accountId === accountId && (this.remote || this.loading || this.offline)) return;
    const serial = ++this.serial;
    const changedAccount = this.accountId !== accountId;
    this.accountId = accountId;
    if (changedAccount) this.remote = null;
    this.offline = false;
    this.error = "";
    if (!accountId) { this.loading = false; return; }
    this.loading = true;
    try {
      const state = await getCosmetics(accountId);
      if (serial === this.serial) { this.remote = state; this.offline = false; }
    } catch (cause) {
      if (serial === this.serial) { this.error = message(cause); this.offline = true; }
    } finally { if (serial === this.serial) this.loading = false; }
  }
  async importFile(file: File, model: SkinModel): Promise<void> {
    if (this.busy) return;
    this.busy = true; this.error = "";
    try {
      if (file.size > 128 * 1024) throw new Error("The PNG exceeds the 128 KiB skin limit.");
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      const name = file.name.replace(/\.png$/i, "").slice(0, 80).trim() || "Imported skin";
      await importSkinPreset(name, model, bytes);
      this.presets = await listSkinPresets();
    } catch (cause) { this.error = message(cause); }
    finally { this.busy = false; }
  }
  async remove(id: string): Promise<void> {
    if (this.busy) return;
    this.busy = true; this.error = "";
    try { await removeSkinPreset(id); this.presets = await listSkinPresets(); }
    catch (cause) { this.error = message(cause); }
    finally { this.busy = false; }
  }
  async apply(id: string, model: SkinModel): Promise<void> {
    const accountId = this.accountId;
    if (!accountId || this.busy || this.offline) return;
    this.busy = true; this.error = "";
    try {
      const state = await applySkinPreset(accountId, id, model);
      if (this.accountId === accountId) {
        this.remote = state;
        await launcher.refreshAvatar(accountId, true);
      }
    } catch (cause) {
      const failure = message(cause);
      if (this.accountId === accountId) await this.load(accountId, true);
      if (this.accountId === accountId) this.error = failure;
    }
    finally { this.busy = false; }
  }
  async selectCape(id: string): Promise<void> { await this.changeCape(id); }
  async disableCape(): Promise<void> { await this.changeCape(null); }
  private async changeCape(id: string | null): Promise<void> {
    const accountId = this.accountId;
    if (!accountId || this.busy || this.offline) return;
    this.busy = true; this.error = "";
    try {
      const state = id ? await selectOwnedCape(accountId, id) : await disableOwnedCape(accountId);
      if (this.accountId === accountId) this.remote = state;
    } catch (cause) {
      const failure = message(cause);
      if (this.accountId === accountId) await this.load(accountId, true);
      if (this.accountId === accountId) this.error = failure;
    } finally { this.busy = false; }
  }
}
export const cosmetics = new CosmeticsStore();
