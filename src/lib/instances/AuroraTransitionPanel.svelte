<script lang="ts">
  import { launcher } from "$lib/launcher/store.svelte";
  import { previewAuroraTransition, applyAuroraTransition, type AuroraTransitionPreview, type InstanceSummary } from "$lib/backend";
  import { transitionRows, executeApprovedTransition } from "./configurationChoices";
  let { instance }: { instance: InstanceSummary } = $props();
  let preview = $state<AuroraTransitionPreview | null>(null);
  let busy = $state(false);
  let error = $state("");
  async function prepare(): Promise<void> {
    busy = true; error = ""; preview = null;
    try { preview = await previewAuroraTransition(instance.id, instance.aurora === null); }
    catch (cause) { error = cause instanceof Error ? cause.message : "Could not prepare the Aurora transition."; }
    finally { busy = false; }
  }
  async function approve(): Promise<void> {
    if (!preview) return;
    busy = true; error = "";
    try {
      const updated = await executeApprovedTransition(preview, applyAuroraTransition, () => launcher.refreshState());
      launcher.detailDrafts[instance.id] = {
        ...(launcher.detailDrafts[instance.id] ?? updated.configuration),
        auroraEnabled: updated.configuration.auroraEnabled,
      };
      preview = null;
      await launcher.runLoadMods(instance.id);
      await launcher.refreshPlayReadiness();
    } catch (cause) {
      error = `${cause instanceof Error ? cause.message : "The transition failed."} Request a new preview before retrying.`;
      preview = null;
      await launcher.refreshState();
    } finally { busy = false; }
  }
</script>
<section class="group" aria-labelledby="aurora-transition-title">
  <div class="group-heading">
    <div><h3 id="aurora-transition-title" class="group-title">Aurora configuration</h3><p class="group-subtitle">{instance.aurora ? `Originally configured with Aurora ${instance.aurora.version} · Content ${instance.auroraContentState ?? "not detected"}` : "No Aurora bootstrap configured"}</p></div>
    <button class="btn" type="button" onclick={() => void prepare()} disabled={busy || instance.state !== "ready" || !launcher.launcherState?.platformCapabilities.find(capability => capability.kind === instance.platform.kind)?.auroraSupported}>
      {busy ? "Working…" : instance.aurora ? "Preview disable" : "Preview enable"}
    </button>
  </div>
  {#if instance.platform.kind === "vanilla"}<p class="group-footer">Aurora is unavailable for this platform.</p>{/if}
  {#if preview}
    <div class="group-row"><p>{preview.current.platform.kind} · Minecraft {preview.current.minecraftVersion} · Aurora {preview.current.aurora?.version ?? "disabled"} → {preview.target.platform.kind} · Minecraft {preview.target.minecraftVersion} · Aurora {preview.target.aurora?.version ?? "disabled"}</p></div>
    {#each preview.requirementsAdded as requirement}<p class="group-row">Add requirement: {requirement}</p>{/each}
    {#each preview.requirementsRemoved as requirement}<p class="group-row">Remove requirement: {requirement}</p>{/each}
    {#each transitionRows(preview) as row}<div class="group-row"><div><strong>{row.action}: {row.path}</strong><p class="group-row-detail">{row.reason}</p></div></div>{/each}
    {#each preview.warnings as warning}<p class="group-row">{warning}</p>{/each}
    {#each preview.blockers as blocker}<p class="inline-message inline-message-error group-row" role="alert">{blocker}</p>{/each}
    <div class="group-row"><button class="btn btn-primary" type="button" disabled={busy || preview.blockers.length > 0} onclick={() => void approve()}>Approve transition</button><button class="btn btn-quiet" type="button" disabled={busy} onclick={() => {preview = null;}}>Cancel</button></div>
  {/if}
  {#if error}<p class="inline-message inline-message-error group-row" role="alert">{error}</p>{/if}
</section>
