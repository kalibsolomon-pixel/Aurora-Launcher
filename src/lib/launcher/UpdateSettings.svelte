<script lang="ts">
  import { onMount } from "svelte";
  import { updates } from "$lib/launcher/updates.svelte";
  import { actionDisabled, actionLabel } from "$lib/launcher/updateController";
  import { formatBytes, presentationNotes } from "$lib/launcher/updatePresentation";
  onMount(() => {
    updates.controller.openView();
    void updates.refresh();
    return () => updates.controller.closeView();
  });
  const overview = $derived(updates.overview);
  const action = $derived(updates.snapshot.action);
  const launcherOffer = $derived(overview?.launcher.availability.kind === "updateAvailable" ? overview.launcher.availability : null);
  const clientOffer = $derived(overview?.client.kind === "updateAvailable" ? overview.client : null);
  const launcherNotes = $derived(presentationNotes(launcherOffer, null));
  const clientNotes = $derived(presentationNotes(clientOffer, null));
  const byteProgress = $derived(action.kind === "updating" && action.phase === "downloading"
    ? [formatBytes(updates.launcherDownloaded), formatBytes(updates.launcherTotal)].filter(Boolean).join(" / ") : "");
</script>

<section class="group f-surface updates-surface" aria-labelledby="updates-title">
  <div class="section-intro"><h3 id="updates-title">Updates</h3></div>
  <div class="versions">
    <div class="version-row"><span class="group-row-title">Launcher</span><span class="version">Version {overview?.launcher.installedVersion ?? "—"}</span></div>
    <div class="version-row"><span class="group-row-title">Aurora Client</span><span class="version">{overview?.clientInstalledVersion ? `Version ${overview.clientInstalledVersion}` : "Not installed"}</span></div>
  </div>
  <div class="update-action">
    <button type="button" class="btn btn-primary" disabled={actionDisabled(action)} onclick={() => void updates.controller.activate()}>{actionLabel(action)}</button>
    {#if action.kind === "checking" || action.kind === "updating"}
      <progress aria-label={actionLabel(action)}></progress>
    {/if}
  </div>
  <div class="update-status" aria-live="polite">
    {#if launcherOffer}<p>Aurora Launcher {launcherOffer.current} → {launcherOffer.candidate}</p>{/if}
    {#if clientOffer}<p>Aurora Client {clientOffer.current} → {clientOffer.candidate}</p>{/if}
    {#if byteProgress}<p class="version">{byteProgress}</p>{/if}
    {#if updates.snapshot.context}<p class:failure={action.kind === "error"}>{updates.snapshot.context}</p>{/if}
    {#if overview?.launcher.availability.kind === "unavailable" && overview.launcher.availability.reason === "Launcher updates are not configured in this build."}
      <p class="version">Launcher updates are not configured in this build.</p>
    {/if}
  </div>
  {#if launcherNotes || clientNotes}
    <details class="notes"><summary>What's New</summary>
      {#if clientNotes}<div class="note-domain"><span class="version">Aurora Client</span><p class="notes-body">{clientNotes}</p></div>{/if}
      {#if launcherNotes}<div class="note-domain"><span class="version">Aurora Launcher</span><p class="notes-body">{launcherNotes}</p></div>{/if}
    </details>
  {/if}
</section>

<style>
  .updates-surface { padding: 28px; max-width: 640px; }
  .versions { display: grid; gap: 18px; padding: 8px 0 24px; }
  .version-row { display: flex; align-items: baseline; justify-content: space-between; gap: 20px; }
  .version { color: var(--color-text-secondary); font-size: 12px; }
  .update-action { display: flex; align-items: center; gap: 16px; }
  .update-action .btn { min-width: 170px; }
  progress { width: 90px; height: 4px; accent-color: var(--color-accent); }
  .update-status { display: grid; gap: 5px; font-size: 13px; line-height: 1.5; }
  .update-status:not(:empty) { margin-top: 16px; }
  .update-status p { margin: 0; overflow-wrap: anywhere; }
  .failure { color: var(--color-text-secondary); }
  .notes { margin-top: 20px; padding-top: 16px; border-top: 1px solid var(--f-edge); }
  .notes summary { cursor: pointer; font-size: 12px; color: var(--color-text-secondary); }
  .note-domain { margin-top: 14px; }
  .notes-body { margin: 5px 0 0; font-size: 12px; line-height: 1.6; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 220px; overflow-y: auto; }
  @media (max-width: 760px) { .updates-surface { padding: 20px; } .version-row { flex-wrap: wrap; gap: 6px; } }
</style>
