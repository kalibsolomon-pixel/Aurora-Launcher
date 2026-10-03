<script lang="ts">
  import { updates } from "$lib/launcher/updates.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import type { InstanceSummary } from "$lib/backend";
  let { instance }: { instance: InstanceSummary } = $props();
  const available = $derived(updates.overview?.clientInstanceId === instance.id && updates.overview.client.kind === "updateAvailable");
</script>
{#if available}
  <p class="client-update-notice">Aurora Client update available <button type="button" class="btn btn-quiet" onclick={() => navigation.openUpdates()}>View Updates</button></p>
{/if}
<style>.client-update-notice { display: flex; align-items: center; gap: 12px; font-size: 12px; color: var(--color-text-secondary); }</style>
