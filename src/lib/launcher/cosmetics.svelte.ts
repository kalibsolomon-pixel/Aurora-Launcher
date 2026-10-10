import {
  applySkinPreset, disableOwnedCape, getCosmetics, importSkinPreset, listSkinPresets,
  removeSkinPreset, selectOwnedCape, updateSkinPreset, saveCurrentSkin, setSkinFavorite,
  type CosmeticsState, type SkinModel, type SkinPreset,
} from "$lib/backend";
import { launcher } from "./store.svelte";

function message(cause: unknown): string { return cause instanceof Error ? cause.message : "Minecraft cosmetics are unavailable."; }
export interface LibraryEntryOutcome { id: string; duplicate: boolean }
export class CosmeticsStore {
  presets = $state<SkinPreset[]>([]);
  remote = $state<CosmeticsState | null>(null);
  accountId = $state<string | null>(null);
  loading = $state(false);
  libraryLoading = $state(false);
  localBusy = $state(false);
  remoteBusy = $state(false);
  offline = $state(false);
  libraryError = $state("");
  remoteError = $state("");
  operation = $state("");
  confirmedAt = $state<number | null>(null);
  selectedPresetId = $state<string | null>(null);
  query = $state("");
  favoritesOnly = $state(false);
  sort = $state<"recent" | "name">("recent");
  private serial = 0;
  private presetsLoaded = false;
  private presetsPending: Promise<void> | null = null;
  get busy(): boolean { return this.localBusy || this.remoteBusy; }
  get error(): string { return this.libraryError || this.remoteError; }
  get fresh(): boolean { return !!this.remote && this.remote.accountId === this.accountId && !this.loading && !this.offline; }
  get canChange(): boolean { return this.fresh && !this.remoteBusy; }

  async loadPresets(force = false): Promise<void> {
    if (this.presetsPending) return this.presetsPending;
    if (this.presetsLoaded && !force) return;
    this.libraryLoading = true;
    this.presetsPending = (async () => {
      try { this.presets = await listSkinPresets(); this.presetsLoaded = true; this.libraryError = ""; }
      catch (cause) { this.libraryError = message(cause); }
      finally { this.libraryLoading = false; this.presetsPending = null; }
    })();
    return this.presetsPending;
  }
  async load(accountId: string | null, force = false): Promise<void> {
    void this.loadPresets();
    if (!force && this.accountId === accountId && (this.remote || this.loading || this.offline)) return;
    const serial = ++this.serial;
    if (this.accountId !== accountId) { this.remote = null; this.confirmedAt = null; }
    this.accountId = accountId; this.offline = false; this.remoteError = "";
    if (!accountId) { this.loading = false; return; }
    this.loading = true;
    try {
      const state = await getCosmetics(accountId);
      if (serial !== this.serial) return;
      if (state.accountId !== accountId) throw new Error("Minecraft returned a different account profile.");
      this.remote = state; this.confirmedAt = Date.now();
    } catch (cause) {
      if (serial === this.serial) { this.remoteError = message(cause); this.offline = true; }
    } finally { if (serial === this.serial) this.loading = false; }
  }
  private async local<T>(action: () => Promise<T>): Promise<T | null> {
    if (this.localBusy) return null;
    this.localBusy = true; this.libraryError = "";
    // A late initial library read cannot overwrite the result of a local edit.
    await this.presetsPending;
    try { return await action(); }
    catch (cause) { this.libraryError = message(cause); return null; }
    finally { this.localBusy = false; }
  }
  async importFile(file: File, model: SkinModel, name?: string): Promise<LibraryEntryOutcome | null> {
    return this.local(async () => {
      if (file.size > 128 * 1024) throw new Error("The PNG exceeds the 128 KiB skin limit.");
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      const outcome = await importSkinPreset(name?.trim() || "Imported skin", model, bytes);
      this.presets = await listSkinPresets(); this.presetsLoaded = true;
      return { id: outcome.preset.id, duplicate: outcome.duplicate };
    });
  }
  async rename(id: string, name: string): Promise<boolean> {
    return (await this.local(async () => { this.presets = await updateSkinPreset(id, { name }); return true; })) === true;
  }
  async setModel(id: string, model: SkinModel): Promise<void> {
    await this.local(async () => { this.presets = await updateSkinPreset(id, { model }); });
  }
  async favorite(id: string, favorite: boolean): Promise<void> {
    await this.local(async () => { this.presets = await setSkinFavorite(id, favorite); });
  }
  async remove(id: string): Promise<boolean> {
    return (await this.local(async () => { await removeSkinPreset(id); this.presets = await listSkinPresets(); return true; })) === true;
  }
  async saveCurrent(accountId: string): Promise<LibraryEntryOutcome | null> {
    if (accountId !== this.accountId || !this.canChange) return null;
    return this.local(async () => {
      const outcome = await saveCurrentSkin(accountId);
      this.presets = await listSkinPresets(); this.presetsLoaded = true;
      return { id: outcome.preset.id, duplicate: outcome.duplicate };
    });
  }
  async apply(id: string): Promise<boolean> {
    const preset = this.presets.find(entry => entry.id === id);
    if (!preset) return false;
    return this.change("Applying skin…", accountId => applySkinPreset(accountId, id, preset.model), true);
  }
  async selectCape(id: string): Promise<boolean> { return this.change("Selecting cape…", accountId => selectOwnedCape(accountId, id)); }
  async disableCape(): Promise<boolean> { return this.change("Removing active cape…", accountId => disableOwnedCape(accountId)); }
  private async change(label: string, action: (accountId: string) => Promise<CosmeticsState>, skin = false): Promise<boolean> {
    const accountId = this.accountId;
    if (!accountId || !this.canChange) return false;
    const serial = ++this.serial;
    this.remoteBusy = true; this.remoteError = ""; this.operation = label;
    try {
      const state = await action(accountId);
      if (serial !== this.serial) return false;
      if (state.accountId !== accountId) throw new Error("Minecraft returned a different account profile.");
      this.remote = state; this.confirmedAt = Date.now(); this.offline = false;
      // Only the matching account's avatar cache changes. Current profile pixels
      // remain the authority on this page, including an unavailable texture.
      if (skin) {
        if (state.currentSkin) launcher.accountAvatars[accountId] = state.currentSkin;
        else await launcher.refreshAvatar(accountId, true);
      }
      return serial === this.serial && this.accountId === accountId;
    } catch (cause) {
      if (serial === this.serial) {
        this.remoteError = message(cause);
        // A request may have reached the service. Retain its last-known data,
        // explicitly stale, until the user asks for a fresh profile.
        this.offline = true;
      }
      return false;
    } finally { this.remoteBusy = false; this.operation = ""; }
  }
}
export const cosmetics = new CosmeticsStore();
