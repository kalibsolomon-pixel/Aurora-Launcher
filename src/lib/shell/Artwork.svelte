<script lang="ts">
  import Icon from './Icon.svelte';
  import { artworkSource, artworkSymbol } from './artwork';
  let { source = null, fallback, size = 44 }: {
    source?: string | null; fallback: string; size?: number;
  } = $props();
  const safeSource = $derived(artworkSource(source));
  let loaded = $state<string | null>(null);
  let failed = $state<string | null>(null);
</script>
<span class="artwork" style:--artwork-size={`${size}px`} aria-hidden="true">
  <span class="placeholder"><Icon name={artworkSymbol(fallback)} size={size < 40 ? 20 : 24} /></span>
  {#if safeSource && failed !== safeSource}
    {#key safeSource}
      {@const imageSource = safeSource}
      <img src={imageSource} alt="" loading="lazy" class:loaded={loaded === imageSource}
        onload={() => loaded = imageSource} onerror={() => failed = imageSource} />
    {/key}
  {/if}
</span>
<style>
  .artwork { position: relative; display: inline-grid; place-items: center; width: var(--artwork-size); height: var(--artwork-size); flex: 0 0 var(--artwork-size); border-radius: 10px; overflow: hidden; background: var(--color-surface-raised); color: var(--color-text-secondary); box-shadow: inset 0 0 0 1px var(--f-edge); }
  .placeholder { display: grid; place-items: center; width: 100%; height: 100%; }
  img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; opacity: 0; }
  img.loaded { opacity: 1; background: var(--color-surface-raised); }
</style>
