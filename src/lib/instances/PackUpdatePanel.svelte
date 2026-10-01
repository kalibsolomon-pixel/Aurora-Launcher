<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    applyModpackUpdate,
    checkModpackUpdate,
    getModpackDetails,
    previewModpackUpdate,
    LauncherBackendError,
    type ModpackDetails,
    type PackConflictResolution,
    type PackUpdateCheck,
    type PackUpdatePlan,
    type PackUpdateRow,
  } from "$lib/backend";

  let {
    instanceId,
    onChanged,
  }: {
    instanceId: string;
    onChanged: () => Promise<void>;
  } = $props();

  let details = $state<ModpackDetails | null>(null);
  let detailsError = $state("");
  let checking = $state(false);
  let error = $state("");
  let success = $state("");
  let check = $state<PackUpdateCheck | null>(null);
  let plan = $state<PackUpdatePlan | null>(null);
  let previewBusy = $state(false);
  let applying = $state(false);
  let phase = $state<string | null>(null);
  let expandedRows = $state(false);
  let expandedChangelog = $state(false);
  let expandedDivergences = $state(false);
  let resolutions = $state<Record<string, "keepLocal" | "useNewPack">>({});
  let stopProgress: (() => void) | null = null;

  const conflicts = $derived(plan?.rows.filter((row) => row.conflict !== null) ?? []);
  const unresolved = $derived(conflicts.filter((row) => !resolutions[row.path]));
  const rowGroups = $derived.by(() => {
    const groups = new Map<string, PackUpdateRow[]>();
    for (const row of plan?.rows ?? []) {
      const list = groups.get(row.action) ?? [];
      list.push(row);
      groups.set(row.action, list);
    }
    return groups;
  });

  const phaseLabels: Record<string, string> = {
    checkingPack: "Checking pack",
    downloading: "Downloading",
    verifying: "Verifying",
    installingGame: "Installing the new game version",
    applyingChanges: "Applying changes",
    rollingBack: "Rolling back",
    validating: "Validating",
    complete: "Finishing",
  };

  function actionLabel(action: string): string {
    switch (action) {
      case "preserve": return "Preserved";
      case "restore": return "Restored";
      case "acquire": return "Added";
      case "adopt": return "Adopted";
      case "replace": return "Updated";
      case "retire": return "Removed";
      case "preserveShared": return "Preserved (still required)";
      default: return action;
    }
  }

  function localLabel(row: PackUpdateRow): string {
    switch (row.localState) {
      case "matchesOld": return row.oldPresent ? "Matches the installed pack" : "";
      case "modified": return "Changed locally";
      case "missing": return "Missing locally";
      case "equivalent": return "Already matches the new pack";
      default: return "";
    }
  }

  function resolutionLabel(value: string): string {
    return value === "keepLocal" ? "Keep my file" : "Use new pack version";
  }

  async function refreshDetails(): Promise<void> {
    try {
      details = await getModpackDetails(instanceId);
      detailsError = "";
    } catch (reason) {
      details = null;
      detailsError = reason instanceof Error ? reason.message : "";
    }
  }

  async function runCheck(): Promise<void> {
    checking = true; error = ""; success = ""; check = null; plan = null; resolutions = {};
    try {
      check = await checkModpackUpdate(instanceId);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "The update check failed.";
    } finally { checking = false; }
  }

  async function openPreview(): Promise<void> {
    previewBusy = true; error = ""; expandedRows = false; expandedChangelog = false;
    try {
      plan = await previewModpackUpdate(instanceId);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "The update preview failed.";
    } finally { previewBusy = false; }
  }

  async function approve(): Promise<void> {
    if (!plan || unresolved.length) return;
    applying = true; error = ""; phase = "checkingPack";
    const chosen: PackConflictResolution[] = conflicts.map((row) => ({
      path: row.path,
      resolution: resolutions[row.path] ?? "keepLocal",
    }));
    try {
      await applyModpackUpdate(instanceId, plan.fingerprint, chosen);
      success = `${plan.currentPackVersion} → ${plan.candidatePackVersion} completed.`;
      plan = null; check = null; resolutions = {};
      await onChanged();
      await refreshDetails();
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "The update failed. The previous modpack version was retained.";
      plan = null;
      await onChanged();
      await refreshDetails();
    } finally {
      applying = false; phase = null;
    }
  }

  onMount(() => {
    let destroyed = false;
    void refreshDetails();
    void listen<{ phase: string }>("modpack-update-progress", (event) => {
      if (applying) phase = event.payload.phase;
    }).then((stop) => { if (destroyed) stop(); else stopProgress = stop; });
    return () => { destroyed = true; };
  });
  onDestroy(() => { stopProgress?.(); });
</script>

<!--
  Modpack status and updates for one pack-managed instance. Pack updates are
  a reconciliation between two authored snapshots and the local files — a
  separate concept from per-mod content updates, which stay on the Content
  page. Nothing here polls; every network action is a user click.
-->
<section class="pack-card" aria-label="Modpack update">
  <div class="pack-head">
    <p class="pack-subtitle">
      Check Modrinth for a newer version of this exact modpack. Aurora never checks in the background.
    </p>
    <button type="button" class="btn btn-quiet" disabled={checking || previewBusy || applying} onclick={runCheck}>
      {checking ? "Checking…" : "Check for update"}
    </button>
  </div>

  {#if detailsError}<p class="pack-error" role="alert">{detailsError}</p>{/if}

  {#if details}
    <p class="pack-quiet">
      Installed: <strong>{details.name} {details.packVersion}</strong>
      · Minecraft {details.minecraftVersion} · Fabric Loader {details.fabricLoaderVersion}
      · {details.componentCount} component{details.componentCount === 1 ? "" : "s"}
      {#if details.divergences.length}
        · <span class="pack-modified">Modified ({details.divergences.length} local change{details.divergences.length === 1 ? "" : "s"})</span>
      {/if}
    </p>
    {#if details.divergences.length}
      <button type="button" class="pack-toggle" aria-expanded={expandedDivergences} onclick={() => { expandedDivergences = !expandedDivergences; }}>
        {expandedDivergences ? "Hide" : "Show"} local changes
      </button>
      {#if expandedDivergences}
        <ul class="pack-divergences">
          {#each details.divergences as divergence (divergence.path)}
            <li>
              <span class="divergence-path">{divergence.path}</span>
              <span class="divergence-note">
                {divergence.expectedSha256 ? "Differs from the pack's file — your local copy is kept" : "The pack does not install this path — your local file is kept"}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  {/if}

  {#if error}<p class="pack-error" role="alert">{error}</p>{/if}
  {#if success && !plan}<p class="pack-success" role="status">{success}</p>{/if}

  {#if check && !plan}
    {#if check.status === "updateAvailable" && check.candidate}
      <div class="pack-panel">
        <div class="pack-panel-head">
          <p class="pack-summary" role="status">
            Update available: <strong>{check.currentPackVersion} → {check.candidate.versionNumber}</strong>
            <span class="pack-type">({check.candidate.versionType})</span>
          </p>
          <button type="button" class="btn" disabled={previewBusy} onclick={openPreview}>
            {previewBusy ? "Preparing…" : "Review update"}
          </button>
        </div>
        <p class="pack-quiet">
          Published {check.candidate.datePublished}
          · Minecraft {check.candidate.gameVersions.join(", ")}
          · Fabric
        </p>
      </div>
    {:else if check.status === "upToDate"}
      <p class="pack-quiet" role="status">
        {check.currentPackVersion} is the newest published version of this pack.
      </p>
    {:else if check.status === "blocked" && check.blockedReason}
      <p class="pack-quiet" role="status">{check.blockedReason}</p>
    {:else if check.status === "providerUnavailable" && check.blockedReason}
      <p class="pack-quiet" role="status">{check.blockedReason}</p>
    {:else}
      <p class="pack-quiet" role="status">
        The installed pack version could not be verified against Modrinth.
      </p>
    {/if}
  {/if}

  {#if plan}
    <div class="pack-preview" role="group" aria-label="Reconciliation preview">
      <strong>Updating {plan.name}: {plan.currentPackVersion} → {plan.candidatePackVersion}</strong>
      <p class="pack-quiet">
        {plan.counts.added} added
        · {plan.counts.updated} updated
        · {plan.counts.removed} removed
        · {plan.counts.preserved} preserved
        {#if plan.counts.conflicts}· <span class="pack-modified">{plan.counts.conflicts} conflict{plan.counts.conflicts === 1 ? "" : "s"}</span>{/if}
      </p>
      {#if plan.gameTransition}
        <p class="pack-transition">
          Minecraft {plan.minecraftCurrent} → {plan.minecraftCandidate}
          · Fabric Loader {plan.loaderCurrent} → {plan.loaderCandidate}.
          The new game version is installed through Aurora's verified pipeline.
        </p>
      {/if}
      {#if plan.changelog}
        <button type="button" class="pack-toggle" aria-expanded={expandedChangelog} onclick={() => { expandedChangelog = !expandedChangelog; }}>
          {expandedChangelog ? "Hide" : "Show"} release notes
        </button>
        {#if expandedChangelog}<pre class="pack-changelog">{plan.changelog}</pre>{/if}
      {/if}

      <button type="button" class="pack-toggle" aria-expanded={expandedRows} onclick={() => { expandedRows = !expandedRows; }}>
        {expandedRows ? "Hide" : "Show"} all {plan.rows.length} change{plan.rows.length === 1 ? "" : "s"}
      </button>
      {#if expandedRows}
        <ul class="pack-rows">
          {#each plan.rows as row (row.path)}
            <li class="pack-row" class:pack-row-conflict={row.conflict !== null}>
              <span class="row-path">{row.title ?? row.path}</span>
              <span class="row-detail">
                {actionLabel(row.action)}
                {#if localLabel(row)}· {localLabel(row)}{/if}
                {#if row.note}· {row.note}{/if}
              </span>
            </li>
          {/each}
        </ul>
      {/if}

      {#if conflicts.length}
        <div class="pack-conflicts" role="group" aria-label="Conflicts requiring a choice">
          <p class="pack-conflicts-title">
            {conflicts.length} path{conflicts.length === 1 ? "" : "s"} need{conflicts.length === 1 ? "s" : ""} your choice before updating
          </p>
          {#each conflicts as row (row.path)}
            <div class="pack-conflict">
              <span class="row-path">{row.title ?? row.path}</span>
              <span class="row-detail">{row.note ?? ""}</span>
              <fieldset class="conflict-choices">
                {#each row.resolutions as option (option)}
                  <label class="conflict-choice">
                    <input
                      type="radio"
                      name={`resolution-${row.path}`}
                      value={option}
                      checked={resolutions[row.path] === option}
                      onchange={(event) => {
                        resolutions = { ...resolutions, [row.path]: event.currentTarget.value as "keepLocal" | "useNewPack" };
                      }}
                      disabled={applying}
                    />
                    {resolutionLabel(option)}
                  </label>
                {/each}
              </fieldset>
            </div>
          {/each}
        </div>
      {/if}

      {#if applying && phase}
        <p class="pack-quiet" role="status">{phaseLabels[phase] ?? phase}…</p>
      {/if}

      <div class="pack-actions">
        <button type="button" class="btn btn-quiet" disabled={applying} onclick={() => { plan = null; resolutions = {}; }}>
          Cancel
        </button>
        <button
          type="button"
          class="btn btn-primary"
          disabled={applying || unresolved.length > 0}
          title={unresolved.length ? "Choose how to handle every conflict first" : undefined}
          onclick={approve}
        >
          {applying ? "Updating…" : unresolved.length ? `Resolve ${unresolved.length} conflict${unresolved.length === 1 ? "" : "s"} first` : "Update Modpack"}
        </button>
      </div>
    </div>
  {/if}
</section>

<style>
  .pack-card { display: grid; gap: var(--space-2); margin-bottom: var(--space-4); font-size: var(--text-secondary); }
  .pack-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
  .pack-subtitle { margin: 0; color: var(--color-text-muted); font-size: var(--text-metadata); }
  .pack-quiet { margin: 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .pack-success { margin: 0; color: var(--color-working); font-size: var(--text-metadata); }
  .pack-error { margin: 0; padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: var(--color-error-soft); color: var(--color-error); font-size: var(--text-secondary); }
  .pack-modified { color: var(--color-warning); }
  .pack-panel { display: grid; gap: var(--space-2); padding: var(--space-3); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-lg); background: var(--f-panel); box-shadow: var(--f-shadow); }
  .pack-panel-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex-wrap: wrap; }
  .pack-summary { margin: 0; color: var(--color-text); font-size: var(--text-body); font-weight: 600; }
  .pack-type { color: var(--color-text-muted); font-weight: 400; }
  .pack-preview { display: grid; gap: var(--space-2); padding: var(--space-3); border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface-raised); }
  .pack-preview strong { color: var(--color-text); font-size: var(--text-body); }
  .pack-transition { margin: 0; color: var(--color-accent); font-size: var(--text-metadata); }
  .pack-toggle { justify-self: start; padding: 0; border: none; background: transparent; color: var(--color-accent); font: inherit; font-size: var(--text-metadata); cursor: pointer; text-decoration: underline; }
  .pack-changelog { max-height: 220px; overflow: auto; margin: 0; padding: var(--space-2); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text-secondary); font-size: var(--text-metadata); white-space: pre-wrap; overflow-wrap: anywhere; }
  .pack-rows { display: grid; gap: var(--space-1); margin: 0; padding: 0; list-style: none; }
  .pack-row { display: grid; gap: 2px; padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: var(--color-surface-sunken); }
  .pack-row-conflict { background: var(--color-error-soft); }
  .row-path { color: var(--color-text); font-size: var(--text-body); font-weight: 500; overflow-wrap: anywhere; }
  .row-detail { color: var(--color-text-muted); font-size: var(--text-metadata); }
  .pack-divergences { display: grid; gap: var(--space-1); margin: 0; padding: 0; list-style: none; }
  .pack-divergences li { display: grid; gap: 2px; padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: var(--color-surface-sunken); }
  .divergence-path { color: var(--color-text); font-size: var(--text-metadata); font-weight: 500; overflow-wrap: anywhere; }
  .divergence-note { color: var(--color-text-muted); font-size: var(--text-metadata); }
  .pack-conflicts { display: grid; gap: var(--space-2); padding: var(--space-3); border: 1px solid var(--color-warning, var(--color-border)); border-radius: var(--radius-md); background: var(--color-surface-raised); }
  .pack-conflicts-title { margin: 0; color: var(--color-warning); font-size: var(--text-body); font-weight: 600; }
  .pack-conflict { display: grid; gap: var(--space-1); }
  .conflict-choices { display: flex; gap: var(--space-3); margin: 0; padding: 0; border: none; }
  .conflict-choice { display: flex; align-items: center; gap: var(--space-1); color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .pack-actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-2); }

  @media (max-width: 800px) {
    .pack-head, .pack-panel-head { flex-wrap: wrap; }
  }
</style>
