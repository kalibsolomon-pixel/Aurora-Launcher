<script module lang="ts">
  import { ArtworkCache } from "./projectArtwork";
  const cache = new ArtworkCache();
</script>
<script lang="ts">
  import { getModrinthProjectArtwork } from "$lib/backend";
  import Artwork from '$lib/shell/Artwork.svelte';
  // Managed provider identity in, Aurora-cached artwork bytes out; local and
  // unknown content renders the caller's type fallback.
  let { projectId, fallback, compact = false }: { projectId: string | null; fallback: string; compact?: boolean } = $props();
  let url = $state<string | null>(null);
  $effect(() => {
    const project = projectId;
    let current = true;
    url = null;
    if(project) void cache.get(project, getModrinthProjectArtwork).then(result => { if(current) url = result; });
    return () => { current = false; };
  });
</script>
<div class="installed-artwork" class:compact aria-hidden="true">
  <Artwork source={url} {fallback} size={compact ? 34 : 44} />
</div>
<style>
  .installed-artwork { width: 44px; height: 44px; flex: 0 0 44px; display: grid; place-items: center; }
  .installed-artwork.compact { width: 34px; height: 34px; flex-basis: 34px; }
</style>
