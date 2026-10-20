<script lang="ts">
  import { tick } from "svelte";
  import InstanceIcon from "./InstanceIcon.svelte";
  import { DEFAULT_INSTANCE_ICONS, iconLabel, searchIcons } from "./instanceIcons";

  let { value = null, onselect, disabled = false, label = "Instance icon" }: {
    value?: string | null;
    onselect: (icon: string | null) => void;
    disabled?: boolean;
    label?: string;
  } = $props();

  const PAGE = 240;
  let dialog: HTMLDialogElement | undefined = $state();
  let search: HTMLInputElement | undefined = $state();
  let trigger: HTMLButtonElement | undefined = $state();
  let query = $state("");
  let browsing = $state(false);
  let limit = $state(PAGE);
  // The full symbol list (~60 KB) loads only once the user searches or browses.
  let allNames = $state<readonly string[] | null>(null);
  let loadError = $state(false);

  async function loadAll(): Promise<void> {
    if (allNames || loadError) return;
    try { allNames = (await import("./materialSymbolNames")).materialSymbolNames; }
    catch { loadError = true; }
  }

  const searching = $derived(query.trim() !== "");
  const results = $derived.by(() => {
    if (!searching && !browsing) return [...DEFAULT_INSTANCE_ICONS];
    if (!allNames) return [];
    return searchIcons(allNames, query, limit);
  });
  const total = $derived(allNames && (searching || browsing)
    ? (searching ? searchIcons(allNames, query, Number.MAX_SAFE_INTEGER).length : allNames.length)
    : DEFAULT_INSTANCE_ICONS.length);

  async function open(): Promise<void> {
    query = ""; browsing = false; limit = PAGE;
    dialog?.showModal();
    await tick();
    search?.focus();
  }
  function close(): void { dialog?.close(); }
  function choose(icon: string | null): void { onselect(icon); close(); }
  async function browseAll(): Promise<void> { browsing = true; limit = PAGE; await loadAll(); }
  $effect(() => { if (searching) void loadAll(); });
</script>

<button bind:this={trigger} type="button" class="icon-trigger" {disabled} onclick={() => void open()}
  aria-haspopup="dialog" aria-label={value ? `${label}: ${iconLabel(value)}. Change icon` : `${label}: none. Choose icon`}>
  <InstanceIcon icon={value} size={24} tile />
  <span class="icon-trigger-text">{value ? "Change" : "Icon"}</span>
</button>

<dialog bind:this={dialog} class="aurora-dialog icon-picker" aria-labelledby="icon-picker-title" onclose={() => trigger?.focus()}>
  <header class="picker-head">
    <h3 id="icon-picker-title">Choose an icon</h3>
    <div class="picker-modes" role="group" aria-label="Icon set">
      <button type="button" class="btn btn-quiet" aria-pressed={!browsing && !searching} onclick={() => { browsing = false; query = ""; }}>Suggested</button>
      <button type="button" class="btn btn-quiet" aria-pressed={browsing && !searching} onclick={() => void browseAll()}>Browse all</button>
    </div>
  </header>
  <label class="field picker-search"><span class="field-label">Search icons</span>
    <input bind:this={search} type="search" bind:value={query} placeholder="e.g. sword, tree, rocket" autocomplete="off" spellcheck="false" />
  </label>

  {#if loadError && (searching || browsing)}
    <p class="picker-note" role="alert">The full icon list could not be loaded. Suggested icons remain available.</p>
  {:else if (searching || browsing) && !allNames}
    <p class="picker-note" role="status">Loading icons…</p>
  {:else if results.length === 0}
    <p class="picker-note" role="status">No icons match “{query.trim()}”.</p>
  {:else}
    <div class="icon-grid" role="group" aria-label={searching ? "Search results" : browsing ? "All icons" : "Suggested icons"}>
      {#each results as icon (icon)}
        <button type="button" class="icon-option" class:chosen={icon === value} aria-pressed={icon === value}
          title={iconLabel(icon)} aria-label={iconLabel(icon)} onclick={() => choose(icon)}>
          <InstanceIcon {icon} size={26} />
        </button>
      {/each}
    </div>
  {/if}

  <footer class="picker-foot">
    <span class="picker-count" role="status">
      {#if searching || browsing}{#if allNames}Showing {results.length.toLocaleString("en-US")} of {total.toLocaleString("en-US")}{/if}{:else}{DEFAULT_INSTANCE_ICONS.length} suggested · Material Symbols{/if}
    </span>
    {#if allNames && (searching || browsing) && results.length < total}
      <button type="button" class="btn btn-quiet" onclick={() => limit += PAGE}>Show more</button>
    {/if}
    <span class="picker-spacer"></span>
    {#if value}<button type="button" class="btn btn-quiet" onclick={() => choose(null)}>Remove icon</button>{/if}
    <button type="button" class="btn" onclick={close}>Cancel</button>
  </footer>
</dialog>

<style>
  .icon-trigger { display: inline-flex; align-items: center; gap: var(--space-2); padding: 4px 12px 4px 4px; border: 1px solid var(--f-edge); border-radius: var(--f-radius-control); background: rgb(255 255 255 / 3%); color: var(--color-text-secondary); font: inherit; font-size: var(--text-secondary); cursor: pointer; transition: background-color var(--f-fast) ease, color var(--f-fast) ease, transform 160ms var(--f-ease); }
  .icon-trigger:hover:not(:disabled) { background: rgb(255 255 255 / 7%); color: var(--color-text); }
  .icon-trigger:active:not(:disabled) { transform: scale(.97); }
  .icon-trigger:disabled { opacity: .55; cursor: default; }
  .icon-picker { width: min(600px, calc(100vw - 32px)); display: none; }
  .icon-picker[open] { display: grid; gap: var(--space-4); }
  .picker-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex-wrap: wrap; }
  .picker-head h3 { margin: 0; }
  .picker-modes { display: flex; gap: var(--space-1); }
  .picker-modes .btn[aria-pressed="true"] { color: var(--color-text); background: var(--color-accent-soft); }
  .picker-search input { width: 100%; }
  .icon-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(52px, 1fr)); gap: var(--space-1); max-height: min(360px, 48dvh); overflow-y: auto; overscroll-behavior: contain; padding: 2px; }
  .icon-option { display: grid; place-items: center; height: 52px; border: 0; border-radius: 12px; background: transparent; color: var(--color-text-secondary); cursor: pointer; transition: background-color 120ms ease, color 120ms ease, transform 160ms var(--f-ease); }
  .icon-option:hover { background: rgb(255 255 255 / 7%); color: var(--color-text); }
  .icon-option:active { transform: scale(.94); }
  .icon-option.chosen { background: var(--color-accent-soft); color: var(--color-text); box-shadow: inset 0 0 0 1px var(--color-accent-outline); }
  .picker-note { margin: 0; padding: var(--space-5) 0; text-align: center; }
  .picker-foot { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
  .picker-count { color: var(--color-text-muted); font-size: var(--text-metadata); }
  .picker-spacer { flex: 1; }
  @media (prefers-reduced-motion: reduce) { .icon-trigger, .icon-option { transition: none; } .icon-trigger:active:not(:disabled), .icon-option:active { transform: none; } }
</style>
