<script lang="ts">
  import type { Snippet } from 'svelte';
  import Tooltip from '$lib/shell/Tooltip.svelte';
  let { id, title, description, controlId, children }: {
    id: string; title: string; description: string; controlId?: string; children?: Snippet;
  } = $props();
</script>
<div class="group-row setting-row">
  <div class="group-row-main">
    <div class="setting-label">
      {#if controlId}<label class="group-row-title" for={controlId}>{title}</label>
      {:else}<span class="group-row-title">{title}</span>{/if}
      {#if description}<Tooltip label={`About ${title}`}>{description}</Tooltip>{/if}
    </div>
  </div>
  {#if children}<div class="setting-control">{@render children()}</div>{/if}
</div>
<style>
  .setting-label { display: flex; align-items: center; gap: 6px; }
</style>
