<script lang="ts">
  import type { ProviderConflict } from "$lib/backend";
  let { conflicts }: { conflicts: ProviderConflict[] } = $props();
  const ownership = { launcherBootstrap: "Launcher bootstrap", launcherManagedRequired: "Launcher Required / Protected", launcherManagedRetained: "Launcher Retained", providerManaged: "Provider Managed", userManaged: "Local", unknown: "Unknown" };
</script>
{#if conflicts.length}
  <div class="inline-message inline-message-error" role="alert">
    <strong>Installation blocked</strong>
    {#each conflicts as conflict}
      <p>{conflict.modId ? `${conflict.modId} · ` : ""}{conflict.fileName} · {ownership[conflict.ownership]}</p>
      <p>{conflict.reason}</p>
    {/each}
  </div>
{/if}
