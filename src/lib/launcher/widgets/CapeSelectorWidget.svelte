<script lang="ts">
  import { untrack } from "svelte";
  import type { WidgetSize } from "$lib/backend";
  import { launcher } from "../store.svelte";
  import { cosmetics } from "../cosmetics.svelte";
  import CapeThumbnail from "./CapeThumbnail.svelte";
  let { size }: { size: WidgetSize } = $props();
  const accountId = $derived(launcher.selectedAccount?.accountId ?? null);
  const capes = $derived(cosmetics.remote?.accountId === accountId ? cosmetics.remote.capes : []);
  const visible = $derived(size === "small" ? capes.slice(0, 3) : size === "wide" ? capes.slice(0, 5) : capes);
  $effect(() => { const id = accountId; untrack(() => { void cosmetics.load(id); }); });
</script>

<div class="cape-widget">
  {#if accountId}<button class="btn btn-quiet" type="button" disabled={cosmetics.loading || cosmetics.busy} onclick={() => cosmetics.load(accountId, true)}>Refresh capes</button>{/if}
  {#if !accountId}<p>Choose a Minecraft account to see its capes.</p>
  {:else if cosmetics.loading}<p role="status">Loading owned capes…</p>
  {:else if cosmetics.offline && capes.length === 0}<p>Minecraft Services is unavailable. Cape changes are paused.</p>
  {:else if capes.length === 0}<p>No capes are reported for this account.</p>
  {:else}
    <ul>
      {#each visible as cape (cape.id)}
        <li><CapeThumbnail preview={cape.preview} name={cape.name} /><span title={cape.name}>{cape.name}{cape.selected ? " · Active" : ""}</span><button class="btn btn-quiet" type="button" disabled={cosmetics.busy || cosmetics.offline || cape.selected} onclick={() => cosmetics.selectCape(cape.id)} aria-label={`Select ${cape.name} cape`}>{cape.selected ? "Selected" : "Select"}</button></li>
      {/each}
    </ul>
    {#if capes.length > visible.length}<p>Expand this widget to see all owned capes.</p>{/if}
    {#if cosmetics.offline}<p>Showing the last fetched cape list. Changes are paused.</p>{/if}
    <button class="btn btn-quiet" type="button" disabled={cosmetics.busy || cosmetics.offline || !capes.some(cape => cape.selected)} onclick={() => cosmetics.disableCape()}>Disable active cape</button>
  {/if}
  {#if cosmetics.busy}<p role="status">Updating cape…</p>{/if}
  {#if cosmetics.error}<p class="inline-message inline-message-error" role="alert">{cosmetics.error}</p>{/if}
</div>
<style>
  .cape-widget { display: grid; gap: var(--space-2); min-width: 0; }
  p { margin: 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  ul { list-style: none; display: grid; gap: var(--space-2); padding: 0; margin: 0; }
  li { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); min-width: 0; }
  li span { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cape-widget > button { justify-self: start; }
</style>
