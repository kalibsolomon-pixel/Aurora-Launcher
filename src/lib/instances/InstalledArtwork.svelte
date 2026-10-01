<script module lang="ts">
  import { ArtworkCache } from "./projectArtwork";
  const cache = new ArtworkCache();
</script>
<script lang="ts">
  import { getModrinthProjectArtwork } from "$lib/backend";
  // Managed provider identity in, Aurora-cached artwork bytes out; local and
  // unknown content renders the caller's type fallback.
  let { projectId, fallback, compact = false }: { projectId: string | null; fallback: string; compact?: boolean } = $props();
  let url = $state<string | null>(null);
  let failed = $state(false);
  $effect(() => {
    const project = projectId;
    let current = true;
    url = null; failed = false;
    if(project) void cache.get(project, getModrinthProjectArtwork).then(result => { if(current) url = result; });
    return () => { current = false; };
  });
</script>
<div class="installed-artwork" class:compact aria-hidden="true">
  {#if url && !failed}<img src={url} alt="" loading="lazy" onerror={() => failed = true} />
  {:else}{fallback}{/if}
</div>
<style>
  .installed-artwork { width: 44px; height: 44px; flex: 0 0 44px; display: grid; place-items: center; border-radius: var(--radius-sm); background: var(--color-surface-raised); color: var(--color-text-secondary); font-weight: 600; overflow: hidden; }
  .installed-artwork.compact { width: 34px; height: 34px; flex-basis: 34px; }
  img { width: 100%; height: 100%; object-fit: contain; }
</style>
