<script lang="ts">
  import { launcher } from "./store.svelte";
  let { compact = false }: { compact?: boolean } = $props();
  const instances = $derived(launcher.launcherState?.instances ?? []);
  const selectedId = $derived(launcher.launcherState?.config.selectedInstanceId ?? "");
</script>

<label class="field instance-switcher" class:compact>
  <select aria-label="Select Play instance" value={selectedId}
    disabled={launcher.instanceBusy !== null || launcher.playBusy || launcher.createBusy}
    onchange={(event) => { if (event.currentTarget.value) void launcher.runSelect(event.currentTarget.value); }}>
    {#if !selectedId}<option value="" disabled>Choose an instance</option>{/if}
    {#each instances as instance (instance.id)}
      <option value={instance.id}>{instance.displayName}{instance.id === selectedId ? " — Selected" : ""}</option>
    {/each}
  </select>
</label>

<style>
  .instance-switcher { display: grid; min-width: 0; margin-bottom: 0; }
  select { font-size: 1.5rem; font-weight: 600; border: none; padding-left: 0; background-color: transparent; width: 100%; min-width: 0; text-overflow: ellipsis; }
  option { background-color: var(--color-surface); color: var(--color-text); font-size: var(--text-body); font-weight: 400; }
  .compact { margin-bottom: 0; }
</style>
