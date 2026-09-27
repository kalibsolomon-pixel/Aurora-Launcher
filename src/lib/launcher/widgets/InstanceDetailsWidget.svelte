<script lang="ts">
  import type { InstanceSummary } from "$lib/backend";
  let { instance }: { instance: InstanceSummary | null } = $props();
</script>
{#if instance}
  <dl>
    <div><dt>Minecraft</dt><dd>{instance.minecraftVersion}</dd></div>
    <div><dt>Platform</dt><dd>{instance.platform.kind === "vanilla" ? "Vanilla" : `Fabric ${instance.platform.version}`}</dd></div>
    <div><dt>Aurora Client</dt><dd>{instance.aurora ? `${instance.aurora.version} · ${instance.auroraContentState ?? "State unavailable"}` : "Not configured"}</dd></div>
  </dl>
{:else}<p>Select an instance to see its installed details.</p>{/if}
<style>
  dl { display: grid; gap: var(--space-2); margin: 0; font-size: var(--text-secondary); }
  dl > div { display: flex; gap: var(--space-3); justify-content: space-between; }
  dt, p { color: var(--color-text-secondary); } dd { margin: 0; text-align: right; overflow-wrap: anywhere; }
</style>
