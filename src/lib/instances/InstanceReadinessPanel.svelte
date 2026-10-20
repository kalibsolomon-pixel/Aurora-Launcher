<script lang="ts">
  import { launcher } from "$lib/launcher/store.svelte";
  import ReadinessRows from "$lib/instances/ReadinessRows.svelte";
  import InstalledArtwork from "$lib/instances/InstalledArtwork.svelte";
  import PackUpdatePanel from "$lib/instances/PackUpdatePanel.svelte";
  import type { InstanceSummary } from "$lib/backend";

  let { instance }: { instance: InstanceSummary } = $props();

  const isSelected = $derived(
    launcher.launcherState?.config.selectedInstanceId === instance.id,
  );
  const busy = $derived(launcher.instanceBusy === instance.id);

</script>

<!--
  Readiness for one instance, shown at the top of its Settings tab (the
  former Overview tab's configuration summary duplicated the editable form).
  Deep validation and Java checks are explicit, on-demand actions; nothing
  polls while the tab is open.
-->
<section class="group" aria-labelledby="readiness-title">
  <div class="group-heading">
    <div>
      <h3 class="group-title" id="readiness-title">Readiness</h3>
    </div>
    <div class="group-row-actions">
      {#if instance.state === "installing" && !instance.pack}
        <button
          type="button"
          class="btn"
          onclick={() => launcher.runRetry(instance.id)}
          disabled={busy || launcher.createBusy}
        >
          Retry install
        </button>
      {:else}
        <button
          type="button"
          class="btn btn-quiet"
          onclick={() => launcher.refreshPlayReadiness()}
          disabled={launcher.playBusy || launcher.playReadinessBusy || !isSelected}
          title={isSelected ? undefined : "Select this instance to refresh Play readiness"}
        >
          {launcher.playReadinessBusy ? "Checking…" : "Check again"}
        </button>
        <button
          type="button"
          class="btn"
          onclick={() => launcher.runValidate(instance.id)}
          disabled={busy || launcher.createBusy}
        >
          {busy ? "Validating…" : "Validate"}
        </button>
      {/if}
    </div>
  </div>

  <ReadinessRows {instance} />

  {#if !isSelected}
    <div class="group-row">
      <div class="group-row-main">
        <span class="group-row-title">Selected instance</span>
        <span class="group-row-detail">
          This instance is not the one Play launches from — selecting it makes it the
          launch target.
        </span>
      </div>
      <div class="group-row-actions">
        <button
          type="button"
          class="btn"
          onclick={() => launcher.runSelect(instance.id)}
          disabled={busy || launcher.createBusy}
        >
          Select
        </button>
      </div>
    </div>
  {/if}

  <p class="group-footer">
    Readiness is decided by Aurora from validated content, the exact managed Java runtime,
    and a usable authenticated session — the Play button reflects that decision.
  </p>
</section>

{#if instance.pack}
  <section class="group" aria-labelledby="pack-title">
    <div class="group-heading"><div><h3 class="group-title" id="pack-title">Modrinth pack</h3></div></div>
    <div class="group-row">
      <InstalledArtwork projectId={instance.pack.projectId} fallback="P" compact />
      <div class="group-row-main">
        <span class="group-row-title">{instance.pack.name}</span>
        <span class="group-row-detail">Version {instance.pack.packVersion}</span>
      </div>
    </div>
  </section>
{/if}

{#if instance.pack && instance.state === "ready"}
  <PackUpdatePanel instanceId={instance.id} onChanged={() => launcher.refreshState()} />
{/if}

<style>
  section + section { margin-top: var(--space-4); }
</style>
