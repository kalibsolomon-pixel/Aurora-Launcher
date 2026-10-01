<script lang="ts">
  import { untrack } from "svelte";
  import type { SkinModel, WidgetSize } from "$lib/backend";
  import { launcher } from "../store.svelte";
  import { cosmetics } from "../cosmetics.svelte";
  import { playerModel, renderPlayer, PLAYER_RENDER_WIDTH, PLAYER_RENDER_HEIGHT } from "../playerModel";
  let { size }: { size: WidgetSize } = $props();
  let previewCanvas: HTMLCanvasElement | undefined = $state();
  let importModel = $state<SkinModel>("classic");
  let chosenModels = $state<Record<string, SkinModel>>({});
  let fileInput: HTMLInputElement;
  const accountId = $derived(launcher.selectedAccount?.accountId ?? null);
  const visible = $derived(size === "small" ? cosmetics.presets.slice(0, 2) : size === "wide" ? cosmetics.presets.slice(0, 4) : cosmetics.presets);
  const model = $derived(playerModel(accountId ? launcher.accountAvatars[accountId] : null));
  $effect(() => { const context = previewCanvas?.getContext("2d"); if (context) renderPlayer(context, model); });
  $effect(() => { const id = accountId; untrack(() => { void cosmetics.load(id); }); });
  async function importSelected(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (file) await cosmetics.importFile(file, importModel);
    input.value = "";
  }
</script>

<div class="cosmetic-widget">
  <div class="current-row"><canvas bind:this={previewCanvas} class="skin-mini" width={PLAYER_RENDER_WIDTH} height={PLAYER_RENDER_HEIGHT} aria-hidden="true"></canvas><p class="cosmetic-current">{#if !accountId}Choose a Minecraft account to see its current skin.{:else if cosmetics.loading}Loading current skin…{:else if cosmetics.remote?.hasCurrentSkin}Current account skin · {cosmetics.remote.currentSkinModel === "slim" ? "Slim" : cosmetics.remote.currentSkinModel === "classic" ? "Classic" : "Model unavailable"}{:else if cosmetics.offline}Current skin unavailable offline. Saved presets remain available.{:else}No current skin reported.{/if}</p></div>
  <div class="cosmetic-toolbar">
    <label>Import model <select aria-label="Import skin model" bind:value={importModel}><option value="classic">Classic</option><option value="slim">Slim</option></select></label>
    <button type="button" class="btn btn-quiet" disabled={cosmetics.busy} onclick={() => fileInput.click()}>Import PNG</button>
    <input bind:this={fileInput} class="visually-hidden" type="file" accept="image/png,.png" aria-label="Choose skin PNG" onchange={importSelected} />
    {#if accountId}<button type="button" class="btn btn-quiet" disabled={cosmetics.loading || cosmetics.busy} onclick={() => cosmetics.load(accountId, true)}>Refresh</button>{/if}
  </div>
  {#if visible.length}
    <ul class="cosmetic-list">
      {#each visible as preset (preset.id)}
        <li>
          <span class="preset-name" title={preset.name}>{preset.name}</span>
          <select aria-label={`Model for ${preset.name}`} value={chosenModels[preset.id] ?? preset.model} onchange={event => chosenModels[preset.id] = event.currentTarget.value as SkinModel}>
            <option value="classic">Classic</option><option value="slim">Slim</option>
          </select>
          <button type="button" class="btn btn-quiet" disabled={!accountId || cosmetics.busy || cosmetics.offline} onclick={() => cosmetics.apply(preset.id, chosenModels[preset.id] ?? preset.model)}>Apply</button>
          <button type="button" class="btn btn-quiet" aria-label={`Remove saved skin ${preset.name}`} disabled={cosmetics.busy} onclick={() => cosmetics.remove(preset.id)}>Remove</button>
        </li>
      {/each}
    </ul>
    {#if cosmetics.presets.length > visible.length}<p class="cosmetic-hint">Expand this widget to see more saved skins.</p>{/if}
  {:else}<p class="cosmetic-hint">No saved skins. Import a PNG to keep a launcher-local preset.</p>{/if}
  {#if cosmetics.busy}<p role="status">Working on your cosmetic change…</p>{/if}
  {#if cosmetics.error}<p class="inline-message inline-message-error" role="alert">{cosmetics.error}</p>{/if}
</div>

<style>
  .cosmetic-widget { display: grid; gap: var(--space-2); min-width: 0; }
  .cosmetic-current, .cosmetic-hint { margin: 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .current-row { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .skin-mini { width: 52px; height: 74px; object-fit: contain; image-rendering: pixelated; flex: none; }
  .cosmetic-toolbar { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
  label { display: flex; align-items: center; gap: var(--space-2); color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .cosmetic-list { list-style: none; padding: 0; margin: 0; display: grid; gap: var(--space-2); }
  li { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .preset-name { min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  select { font: inherit; color: var(--color-text); background: var(--color-surface-sunken); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); padding: var(--space-2); }
  .visually-hidden { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }
  @media (max-width: 600px) { li { flex-wrap: wrap; } .preset-name { flex-basis: 100%; } }
</style>
