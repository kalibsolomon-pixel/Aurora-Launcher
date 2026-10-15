<script lang="ts">
  import type { Snippet } from 'svelte';
  let { id, title, description, controlId, children }: {
    id: string; title: string; description: string; controlId?: string; children?: Snippet;
  } = $props();
</script>
<div class="group-row setting-row">
  <div class="group-row-main">
    {#if controlId}<label class="group-row-title" for={controlId}>{title}</label>
    {:else}<span class="group-row-title">{title}</span>{/if}
    <span class="group-row-detail" id={`${id}-description`}>{description}</span>
  </div>
  {#if children}<div class="setting-control">{@render children()}</div>{/if}
</div>
