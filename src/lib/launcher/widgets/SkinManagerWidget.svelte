<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { SkinModel, WidgetSize } from "$lib/backend";
  import { launcher } from "../store.svelte";
  import { cosmetics } from "../cosmetics.svelte";
  import SkinThumbnail from "./SkinThumbnail.svelte";
  let { size }: { size: WidgetSize } = $props();
  let currentCanvas: HTMLCanvasElement | undefined = $state();
  let importModel = $state<SkinModel>("classic");
  let fileInput: HTMLInputElement;
  let showAll = $state(false);
  let manageId = $state<string | null>(null);
  let highlightId = $state<string | null>(null);
  let notice = $state("");
  let renameDraft = $state("");
  let renameId = $state<string | null>(null);
  let renameInput: HTMLInputElement | undefined = $state();
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  const accountId = $derived(launcher.selectedAccount?.accountId ?? null);
  const avatar = $derived(accountId ? launcher.accountAvatars[accountId] : null);
  const currentHash = $derived(avatar?.sha256 ?? null);
  const collapsedLimit = $derived(size === "small" ? 3 : size === "wide" ? 5 : 8);
  const visible = $derived(showAll ? cosmetics.presets : cosmetics.presets.slice(0, collapsedLimit));
  const model = { classic: "Classic", slim: "Slim" } as const;
  function modelLabel(value: SkinModel | null | undefined): string { return value ? model[value] : "Model unavailable"; }
  /** Proven byte identity, never display-name equality. */
  function isCurrent(sha256: string): boolean {
    return !!accountId && !!currentHash && sha256 === currentHash;
  }
  function announce(text: string, id?: string): void {
    notice = text;
    clearTimeout(noticeTimer);
    noticeTimer = id
      ? setTimeout(() => { highlightId = null; }, 2600)
      : undefined;
    if (id) highlightId = id;
  }
  $effect(() => {
    if (currentCanvas && avatar?.rgba.length === 256) {
      currentCanvas.getContext("2d")?.putImageData(new ImageData(new Uint8ClampedArray(avatar.rgba), 8, 8), 0, 0);
    }
  });
  $effect(() => { const id = accountId; untrack(() => { void cosmetics.load(id); }); });
  async function importSelected(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (file) {
      const outcome = await cosmetics.importFile(file, importModel);
      if (outcome) announce(outcome.duplicate ? "Already in library." : "Saved to library.", outcome.id);
    }
    input.value = "";
  }
  async function saveCurrentSkin() {
    if (!accountId) return;
    const outcome = await cosmetics.saveCurrent(accountId);
    if (outcome) announce(outcome.duplicate ? "Already in library." : "Current skin saved to library.", outcome.id);
  }
  function beginRename(id: string, name: string): void {
    renameId = id; renameDraft = name;
    void tick().then(() => renameInput?.focus());
  }
  async function commitRename(): Promise<void> {
    const id = renameId;
    if (!id) return;
    const name = renameDraft.trim();
    if (name && await cosmetics.rename(id, name)) {
      manageId = null;
      announce("Saved skin renamed.");
    }
    renameId = null;
  }
</script>

<div class="cosmetic-widget">
  <div class="current-row">
    <span class="current-head">
      {#if avatar?.rgba.length === 256}
        <canvas bind:this={currentCanvas} width="8" height="8" aria-hidden="true"></canvas>
      {:else}
        <span class="head-fallback" aria-hidden="true"></span>
      {/if}
    </span>
    <p class="cosmetic-current">{#if !accountId}Choose a Minecraft account to see its current skin.{:else if cosmetics.loading}Loading current skin…{:else if cosmetics.remote?.hasCurrentSkin}Current skin · {modelLabel(cosmetics.remote.currentSkinModel)}{#if cosmetics.presets.some(preset => isCurrent(preset.sha256))} · In library{/if}{:else if cosmetics.offline}Current skin unavailable offline. Saved skins remain available.{:else}No current skin reported.{/if}</p>
    {#if accountId && cosmetics.remote?.hasCurrentSkin && !cosmetics.offline}
      <button type="button" class="btn btn-quiet" disabled={cosmetics.busy} onclick={saveCurrentSkin}>Save to library</button>
    {/if}
  </div>
  {#if cosmetics.presets.length}
    <ul class="cosmetic-list">
      {#each visible as preset (preset.id)}
        <li class:current={isCurrent(preset.sha256)} class:highlight={highlightId === preset.id}>
          <SkinThumbnail presetId={preset.id} name={preset.name} />
          <span class="preset-name" title={preset.name}>{preset.name}</span>
          <span class="model-badge" class:slim={preset.model === "slim"}>{model[preset.model]}</span>
          {#if isCurrent(preset.sha256)}<span class="current-badge">Current</span>{/if}
          <button type="button" class="btn btn-quiet apply" disabled={!accountId || cosmetics.busy || cosmetics.offline} onclick={() => void cosmetics.apply(preset.id)}>Apply</button>
          <button type="button" class="f-icon-button manage" aria-label={`Manage saved skin ${preset.name}`} aria-expanded={manageId === preset.id} disabled={cosmetics.busy} onclick={() => { if (manageId === preset.id) { manageId = null; } else { manageId = preset.id; renameId = null; } }}>…</button>
          {#if manageId === preset.id}
            <div class="preset-manage" role="group" aria-label={`Manage ${preset.name}`}>
              {#if renameId === preset.id}
                <label class="rename-row">Rename
                  <input bind:this={renameInput} bind:value={renameDraft} maxlength="80" aria-label={`New name for ${preset.name}`} onkeydown={event => { if (event.key === "Escape") { event.preventDefault(); renameId = null; } if (event.key === "Enter") { event.preventDefault(); void commitRename(); } }} />
                </label>
                <button type="button" class="btn btn-quiet" disabled={cosmetics.busy} onclick={() => void commitRename()}>Save name</button>
                <button type="button" class="btn btn-quiet" onclick={() => { renameId = null; }}>Cancel</button>
              {:else}
                <button type="button" class="btn btn-quiet" disabled={cosmetics.busy} onclick={() => beginRename(preset.id, preset.name)}>Rename…</button>
                <label>Model
                  <select aria-label={`Model for ${preset.name}`} disabled={cosmetics.busy} value={preset.model} onchange={event => void cosmetics.setModel(preset.id, event.currentTarget.value as SkinModel)}>
                    <option value="classic">Classic</option><option value="slim">Slim</option>
                  </select>
                </label>
                <button type="button" class="btn btn-quiet" aria-label={`Remove saved skin ${preset.name} from library`} disabled={cosmetics.busy} onclick={() => void cosmetics.remove(preset.id)}>Remove from library</button>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
    {#if cosmetics.presets.length > visible.length}
      <button type="button" class="library-toggle" onclick={() => { showAll = true; }}>View library ({cosmetics.presets.length} saved)</button>
    {:else if showAll && cosmetics.presets.length > collapsedLimit}
      <button type="button" class="library-toggle" onclick={() => { showAll = false; }}>Show less</button>
    {/if}
  {:else}
    <p class="cosmetic-hint">No saved skins yet. Import a PNG to keep it in your library.</p>
  {/if}
  <div class="cosmetic-toolbar">
    <label>Import as <select aria-label="Import skin model" bind:value={importModel}><option value="classic">Classic</option><option value="slim">Slim</option></select></label>
    <button type="button" class="btn btn-quiet" disabled={cosmetics.busy} onclick={() => fileInput.click()}>Import skin</button>
    <input bind:this={fileInput} class="visually-hidden" type="file" accept="image/png,.png" aria-label="Choose skin PNG" onchange={importSelected} />
    {#if accountId}<button type="button" class="btn btn-quiet" disabled={cosmetics.loading || cosmetics.busy} onclick={() => cosmetics.load(accountId, true)}>Refresh</button>{/if}
  </div>
  {#if notice}<p class="cosmetic-notice" role="status">{notice}</p>{/if}
  {#if cosmetics.busy}<p role="status">Working on your cosmetic change…</p>{/if}
  {#if cosmetics.error}<p class="inline-message inline-message-error" role="alert">{cosmetics.error}</p>{/if}
</div>

<style>
  .cosmetic-widget { display: grid; gap: var(--space-2); min-width: 0; }
  .cosmetic-current, .cosmetic-hint, .cosmetic-notice { margin: 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .current-row { display: flex; align-items: center; gap: var(--space-2); min-width: 0; flex-wrap: wrap; }
  .current-head { display: block; width: 28px; height: 28px; flex: none; border-radius: var(--radius-sm); overflow: hidden; background: var(--color-surface-raised); }
  .current-head canvas { display: block; width: 100%; height: 100%; image-rendering: pixelated; }
  .head-fallback { display: block; width: 100%; height: 100%; }
  .cosmetic-current { flex: 1; min-width: 160px; }
  .cosmetic-list { list-style: none; padding: 0; margin: 0; display: grid; gap: var(--space-2); }
  li { display: flex; align-items: center; gap: var(--space-2); min-width: 0; flex-wrap: wrap; padding: 2px 4px; border-radius: var(--radius-sm); }
  li.current { background: var(--color-accent-soft); }
  li.highlight { outline: 1px dashed var(--color-accent-outline); outline-offset: 2px; }
  .preset-name { min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .model-badge { flex: none; padding: 1px var(--space-2); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .model-badge.slim { color: var(--color-text); }
  .current-badge { flex: none; padding: 1px var(--space-2); border-radius: var(--radius-sm); background: var(--color-accent-soft); color: var(--color-accent); font-size: var(--text-metadata); font-weight: 600; }
  .apply { flex: none; }
  .manage { width: 26px; height: 26px; flex: none; }
  .preset-manage { flex-basis: 100%; display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; padding: var(--space-2) 0 2px 36px; border-top: 1px dashed var(--color-border); }
  .rename-row { display: flex; align-items: center; gap: var(--space-2); flex: 1; min-width: 200px; }
  .rename-row input { flex: 1; min-width: 120px; padding: var(--space-1) var(--space-2); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text); font: inherit; }
  .preset-manage label { display: flex; align-items: center; gap: var(--space-2); color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .preset-manage select { font: inherit; color: var(--color-text); background: var(--color-surface-sunken); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); padding: var(--space-1); }
  .library-toggle { justify-self: start; background: none; border: 0; padding: 0; font: inherit; font-size: var(--text-metadata); color: var(--color-accent); cursor: pointer; text-decoration: underline; text-underline-offset: 2px; }
  .cosmetic-toolbar { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
  .cosmetic-toolbar label { display: flex; align-items: center; gap: var(--space-2); color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .cosmetic-toolbar select { font: inherit; color: var(--color-text); background: var(--color-surface-sunken); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); padding: var(--space-2); }
  .visually-hidden { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }
  @media (max-width: 600px) { .preset-manage { padding-left: 0; } }
</style>
