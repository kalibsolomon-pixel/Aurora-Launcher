<script module lang="ts">
  import { ArtworkCache } from "./projectArtwork";
  const cache = new ArtworkCache();
</script>
<script lang="ts">
  import { artworkProject } from "./projectArtwork";
  import { getModrinthProjectArtwork, type ModEntry } from "$lib/backend";
  let { entry }: { entry: ModEntry } = $props();
  let url = $state<string | null>(null);
  let failed = $state(false);
  $effect(() => {
    const project = artworkProject(entry);
    let current = true;
    url = null; failed = false;
    if(project) void cache.get(project, getModrinthProjectArtwork).then(result => { if(current) url = result; });
    return () => { current = false; };
  });
</script>
<div class="installed-artwork" aria-hidden="true">
  {#if url && !failed}<img src={url} alt="" loading="lazy" onerror={() => failed = true} />
  {:else}{entry.metadata ? "M" : "J"}{/if}
</div>
<style>
  .installed-artwork { width: 44px; height: 44px; flex: 0 0 44px; display: grid; place-items: center; border-radius: var(--radius-sm); background: var(--color-surface-raised); color: var(--color-text-secondary); font-weight: 600; overflow: hidden; }
  img { width: 100%; height: 100%; object-fit: contain; }
</style>
