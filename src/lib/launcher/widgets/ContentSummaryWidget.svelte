<script lang="ts">
  import { untrack } from "svelte";
  import type { InstanceSummary } from "$lib/backend";
  import { launcher } from "../store.svelte";
  let { instance }: { instance: InstanceSummary | null } = $props();
  const mods = $derived(instance ? launcher.modInventories[instance.id] : null);
  const resources = $derived(instance ? launcher.contentInventories[`${instance.id}:resourcePack`] : null);
  const shaders = $derived(instance ? launcher.contentInventories[`${instance.id}:shaderPack`] : null);
  $effect(() => {
    const id = instance?.id;
    untrack(() => { if (id) {
      if (!launcher.modInventories[id]) void launcher.runLoadMods(id);
      if (!launcher.contentInventories[`${id}:resourcePack`]) void launcher.runLoadContent(id, "resourcePack");
      if (!launcher.contentInventories[`${id}:shaderPack`]) void launcher.runLoadContent(id, "shaderPack");
    } });
  });
</script>
{#if instance}
  <div class="counts">
    <div><strong>{mods ? mods.entries.filter(entry => entry.enabled && entry.fileType === "enabledJar").length : "—"}</strong><span>Enabled mods</span></div>
    <div><strong>{mods ? mods.entries.filter(entry => entry.fileType === "disabledJar").length : "—"}</strong><span>Disabled mods</span></div>
    <div><strong>{resources?.entries.length ?? "—"}</strong><span>Resource packs</span></div>
    <div><strong>{shaders?.entries.length ?? "—"}</strong><span>Shader packs</span></div>
  </div>
  <p>Local inventory · pack counts do not imply activation.</p>
{:else}<p>Select an instance to see its local content.</p>{/if}
<style>
  .counts { display: grid; grid-template-columns: repeat(4,minmax(0,1fr)); gap: var(--space-3); }
  .counts div { display: grid; gap: var(--space-1); } strong { font-size: 24px; font-weight: 500; }
  span, p { font-size: var(--text-metadata); color: var(--color-text-secondary); } p { margin: var(--space-3) 0 0; }
  @media (max-width: 1150px) { .counts { grid-template-columns: repeat(2,minmax(0,1fr)); } }
</style>
