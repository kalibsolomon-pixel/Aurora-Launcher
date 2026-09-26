<script lang="ts">
  import { launcher } from "./store.svelte";
  import { installedConfigurationLabel } from "./instanceStatus";
  let { compact = false }: { compact?: boolean } = $props();
  const instances = $derived(launcher.launcherState?.instances ?? []);
  const selectedId = $derived(launcher.launcherState?.config.selectedInstanceId ?? "");
</script>

<label class="field instance-switcher" class:compact>
  <span class="field-label">Play instance</span>
  <select aria-label="Select Play instance" value={selectedId}
    disabled={launcher.instanceBusy !== null || launcher.playBusy || launcher.createBusy}
    onchange={(event) => { if (event.currentTarget.value) void launcher.runSelect(event.currentTarget.value); }}>
    {#if !selectedId}<option value="" disabled>Choose an instance</option>{/if}
    {#each instances as instance (instance.id)}
      <option value={instance.id}>{instance.displayName} — {installedConfigurationLabel(instance)}{instance.id === selectedId ? " — Selected" : ""}</option>
    {/each}
  </select>
</label>

<style>
  .instance-switcher { display: grid; gap: var(--space-2); min-width: 0; margin-bottom: var(--space-4); }
  select { width: 100%; min-width: 0; text-overflow: ellipsis; }
  .compact { margin-bottom: 0; }
</style>
