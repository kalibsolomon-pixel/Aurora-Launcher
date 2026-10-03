<script lang="ts">
  import { launcher } from "$lib/launcher/store.svelte";
  import { updates } from "$lib/launcher/updates.svelte";
  import {
    applyClientUpdate,
    previewClientUpdate,
    LauncherBackendError,
    type ClientUpdatePreview,
    type InstanceSummary,
  } from "$lib/backend";
  import { clientPhaseLabel, presentationNotes } from "$lib/launcher/updatePresentation";

  let { instance }: { instance: InstanceSummary } = $props();
  let preview = $state<ClientUpdatePreview | null>(null);
  let busy = $state(false);
  let checking = $state(false);
  let error = $state("");
  let phase = $state("");

  // The workspace renders real transaction phases emitted by the backend.
  $effect(() => { phase = updates.clientPhase ?? ""; });

  const notes = $derived(preview?.candidate ? presentationNotes(null, preview.candidate.notes) : null);

  async function check(): Promise<void> {
    checking = true;
    error = "";
    preview = null;
    try {
      preview = await previewClientUpdate(instance.id);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Could not check for an Aurora Client update.";
    } finally {
      checking = false;
    }
  }

  async function update(): Promise<void> {
    if (!preview?.candidate || preview.blockers.length > 0) return;
    busy = true;
    error = "";
    try {
      await applyClientUpdate(instance.id, preview.fingerprint);
      preview = null;
      await launcher.refreshState();
      await launcher.runLoadMods(instance.id);
      await launcher.refreshPlayReadiness();
      await updates.refresh();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "The update failed.";
      preview = null;
      await launcher.refreshState();
    } finally {
      busy = false;
    }
  }
</script>

{#if instance.aurora}
  <section class="group" aria-labelledby="aurora-update-title">
    <div class="group-heading">
      <div>
        <h3 id="aurora-update-title" class="group-title">Aurora Client update</h3>
        <p class="group-subtitle">Installed Aurora Client {instance.aurora.version} · channel {instance.aurora.channel}</p>
      </div>
      <button type="button" class="btn" disabled={checking || busy || instance.state !== "ready"} onclick={() => void check()}>
        {checking ? "Checking…" : "Check for update"}
      </button>
    </div>
    {#if preview}
      {#if preview.outcome === "updateAvailable" && preview.candidate}
        <div class="group-row">
          <div class="group-row-main">
            <span class="group-row-title">Aurora Client {preview.candidate.version} available</span>
            <span class="group-row-detail">
              {preview.installedVersion} → {preview.candidate.version} · channel {preview.candidate.channel} ·
              Minecraft {preview.candidate.minecraftVersion} · Fabric Loader {preview.candidate.fabricLoaderVersion}
            </span>
          </div>
        </div>
        {#if notes}
          <details class="notes">
            <summary>Release notes</summary>
            <pre class="notes-body">{notes}</pre>
          </details>
        {/if}
      {:else if preview.outcome === "installedNewer"}
        <div class="group-row"><p class="group-row-detail">The installed Aurora Client is newer than every eligible published release; it will not be downgraded.</p></div>
      {:else}
        <div class="group-row"><p class="group-row-detail">The installed Aurora Client is up to date.</p></div>
      {/if}
      {#each preview.warnings as warning}<p class="group-row">{warning}</p>{/each}
      {#each preview.blockers as blocker}<p class="inline-message inline-message-error group-row" role="alert">{blocker}</p>{/each}
      {#if preview.outcome === "updateAvailable"}
        <div class="group-row">
          <button type="button" class="btn btn-primary" disabled={busy || preview.blockers.length > 0} onclick={() => void update()}>
            {busy ? "Updating…" : `Update to ${preview.candidate?.version ?? ""}`}
          </button>
          <button type="button" class="btn btn-quiet" disabled={busy} onclick={() => { preview = null; }}>Cancel</button>
        </div>
      {/if}
    {/if}
    {#if phase && busy}<p class="group-row-detail" role="status">{clientPhaseLabel(phase)}</p>{/if}
    {#if error}<p class="inline-message inline-message-error group-row" role="alert">{error}</p>{/if}
  </section>
{/if}

<style>
  .notes summary { cursor: pointer; font-size: 12px; color: var(--color-text-secondary); }
  .notes-body { margin: 10px 0 0; padding: 12px; border: 1px solid var(--f-edge); border-radius: 10px; background: rgb(8 15 25 / 32%); font-family: inherit; font-size: 12px; line-height: 1.6; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 260px; overflow-y: auto; }
</style>
