<script module lang="ts">
  import { ResolutionCache } from "./projectArtwork";
  import type { ProjectArtworkResult } from '$lib/backend';
  const cache = new ResolutionCache<ProjectArtworkResult>();
</script>
<script lang="ts">
  import { resolveProjectArtwork } from "$lib/backend";
  import Artwork from '$lib/shell/Artwork.svelte';
  // Native provider identity in, validated Aurora-cached PNG out. Installed
  // callers supply provenance; Browse supplies the native search project id.
  // Artwork resolution never adopts local content or grants managed ownership.
  let { projectId, fallback, compact = false }: { projectId: string | null; fallback: string; compact?: boolean } = $props();
  let url = $state<string | null>(null);
  $effect(() => {
    const project = projectId;
    let current = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    url = null;
    async function resolve() {
      if (!project || !current) return;
      try {
        const result = await cache.get(project, () => resolveProjectArtwork(project));
        if (!current) return;
        url = result.source;
        if (result.retryAfterMs) timer = setTimeout(resolve, result.retryAfterMs);
      } catch {
        if (current) timer = setTimeout(resolve, 60_000);
      }
    }
    void resolve();
    return () => { current = false; clearTimeout(timer); };
  });
</script>
<div class="installed-artwork" class:compact aria-hidden="true">
  <Artwork source={url} {fallback} size={compact ? 34 : 44} />
</div>
<style>
  .installed-artwork { width: 44px; height: 44px; flex: 0 0 44px; display: grid; place-items: center; }
  .installed-artwork.compact { width: 34px; height: 34px; flex-basis: 34px; }
</style>
