<script lang="ts">
  import {
    applyModrinthBulkUpdate,
    checkInstanceUpdates,
    previewModrinthBulkUpdate,
    setProviderUpdatePolicy,
    summarizeUpdates,
    type BulkUpdatePreview,
    type BulkUpdateTarget,
    type ContentType,
    type UpdateAvailability,
    type UpdatesReport,
  } from "$lib/backend";

  let {
    instanceId,
    kind,
    names,
    report = $bindable(null),
    onChanged,
  }: {
    instanceId: string;
    kind: ContentType;
    /** projectId → display title from the installed inventory. */
    names: Record<string, string>;
    report?: UpdatesReport | null;
    onChanged: () => Promise<void>;
  } = $props();

  let checking = $state(false);
  let error = $state("");
  let success = $state("");
  let selected = $state<Record<string, boolean>>({});
  let preview = $state<BulkUpdatePreview | null>(null);
  let previewBusy = $state(false);
  let applying = $state(false);
  let dismissed = $state(false);
  let expandedChangelog = $state<string | null>(null);

  const entries = $derived((report?.entries ?? []).filter((entry) => entry.contentType === kind));
  const counts = $derived(summarizeUpdates(entries));
  const actionable = $derived(counts.ready + counts.pinned);
  // Rows shown inside the actionable card: updateable and pinned items plus
  // blocked ones that need user attention before they can ever update.
  const shown = $derived(
    entries.filter((entry) =>
      entry.status === "updateAvailable"
      || entry.status === "pinnedUpdateAvailable"
      || entry.status === "blocked",
    ),
  );
  const readyEntries = $derived(entries.filter((entry) => entry.status === "updateAvailable"));
  const selectedTargets = $derived(
    readyEntries
      .filter((entry) => selected[`${entry.contentType}:${entry.projectId}`] !== false)
      .map((entry) => ({ contentType: entry.contentType, projectId: entry.projectId }) satisfies BulkUpdateTarget),
  );
  const allSelected = $derived(readyEntries.length === selectedTargets.length);

  function title(entry: UpdateAvailability): string {
    return names[entry.projectId] ?? entry.projectId;
  }

  async function check(): Promise<void> {
    checking = true; error = ""; success = ""; preview = null; dismissed = false; expandedChangelog = null;
    try {
      report = await checkInstanceUpdates(instanceId, kind);
      selected = {};
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Update check failed.";
    } finally { checking = false; }
  }

  function dismiss(): void {
    // Presentation-only: no provider state, policy, or suppression changes.
    dismissed = true;
    preview = null;
    expandedChangelog = null;
  }

  function toggleAll(): void {
    const next: Record<string, boolean> = {};
    for (const entry of readyEntries) {
      next[`${entry.contentType}:${entry.projectId}`] = !allSelected;
    }
    selected = next;
  }

  async function openPreview(targets: BulkUpdateTarget[]): Promise<void> {
    if (!targets.length) return;
    previewBusy = true; error = ""; success = ""; expandedChangelog = null;
    try {
      preview = await previewModrinthBulkUpdate(instanceId, targets);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Update preview failed.";
    } finally { previewBusy = false; }
  }

  async function approve(): Promise<void> {
    if (!preview) return;
    applying = true; error = "";
    const targets: BulkUpdateTarget[] = preview.targets.map((target) => ({
      contentType: target.current.contentType,
      projectId: target.current.projectId,
    }));
    try {
      await applyModrinthBulkUpdate(instanceId, targets, preview.previewFingerprint);
      preview = null; report = null; selected = {}; dismissed = false;
      await onChanged();
      success = targets.length === 1 ? "Update completed." : `${targets.length} updates completed.`;
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Update failed. The previous installation was retained.";
      preview = null;
    } finally { applying = false; }
  }

  async function unpin(projectId: string): Promise<void> {
    error = "";
    try {
      await setProviderUpdatePolicy(instanceId, kind, projectId, { pinned: false });
      const entry = entries.find((item) => item.projectId === projectId);
      if (entry) {
        entry.pinned = false;
        if (entry.status === "pinnedUpdateAvailable") entry.status = "updateAvailable";
      }
      success = `${names[projectId] ?? projectId} unpinned. It can now be updated.`;
      await onChanged();
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Unpinning failed.";
    }
  }
</script>

<section class="updates-card" aria-label="Managed content updates">
  <div class="updates-head">
    <p class="updates-subtitle">Check Modrinth for newer versions of managed {kind === "mod" ? "mods" : kind === "resourcePack" ? "resource packs" : "shader packs"}. Aurora never checks in the background.</p>
    <button type="button" class="btn btn-quiet" disabled={checking || previewBusy || applying} onclick={check}>
      {checking ? "Checking…" : "Check for updates"}
    </button>
  </div>

  {#if error}<p class="updates-error" role="alert">{error}</p>{/if}
  {#if success && !dismissed}<p class="updates-success" role="status">{success}</p>{/if}

  {#if report && !dismissed}
    {#if actionable === 0 && counts.blocked === 0}
      <p class="updates-quiet" role="status">
        Everything is up to date
        {#if counts.noted}· {counts.noted} item{counts.noted === 1 ? "" : "s"} to review in Details{/if}.
      </p>
    {:else}
      <div class="updates-panel">
        <div class="updates-panel-head">
          <p class="updates-summary" role="status">
            {counts.ready} update{counts.ready === 1 ? "" : "s"} available
            {#if counts.pinned}· {counts.pinned} pinned{/if}
            {#if counts.blocked}· {counts.blocked} blocked{/if}
          </p>
          <div class="updates-panel-actions">
            {#if counts.ready > 1}
              <button type="button" class="btn" disabled={previewBusy || applying} onclick={() => openPreview(readyEntries.map((entry) => ({ contentType: entry.contentType, projectId: entry.projectId })))}>
                {previewBusy ? "Preparing…" : "Update all"}
              </button>
            {/if}
            <button type="button" class="updates-dismiss" aria-label="Dismiss update results" title="Dismiss" onclick={dismiss}>×</button>
          </div>
        </div>
        <ul class="updates-list">
          {#each shown as entry (entry.projectId)}
            <li class="update-row" class:update-row-pinned={entry.status === "pinnedUpdateAvailable"} class:update-row-blocked={entry.status === "blocked"}>
              {#if entry.status === "updateAvailable"}
                <input
                  type="checkbox"
                  aria-label={`Include ${title(entry)} in update`}
                  checked={selected[`${entry.contentType}:${entry.projectId}`] !== false}
                  onchange={(event) => { selected = { ...selected, [`${entry.contentType}:${entry.projectId}`]: event.currentTarget.checked }; }}
                />
              {:else}
                <span class="update-dot" aria-hidden="true"></span>
              {/if}
              <div class="update-main">
                <span class="update-name">{title(entry)}</span>
                <span class="update-state">
                  {#if entry.candidate}
                    {entry.currentVersion} → {entry.candidate.versionNumber}
                    <span class="update-type">({entry.candidate.versionType})</span>
                  {:else}
                    {entry.currentVersion}
                  {/if}
                </span>
                {#if entry.status === "blocked"}<span class="update-note">{entry.detail ?? "Updating is blocked"}</span>{/if}
              </div>
              <div class="update-actions">
                {#if entry.status === "pinnedUpdateAvailable"}
                  <span class="badge badge-pinned">Pinned</span>
                  <button type="button" class="btn btn-quiet" disabled={previewBusy || applying} onclick={() => unpin(entry.projectId)}>Unpin</button>
                {:else if entry.status === "updateAvailable"}
                  <button type="button" class="btn btn-quiet" disabled={previewBusy || applying} onclick={() => openPreview([{ contentType: entry.contentType, projectId: entry.projectId }])}>Update…</button>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
        {#if counts.ready > 1}
          <div class="updates-bulk">
            <button type="button" class="btn btn-quiet" onclick={toggleAll} disabled={previewBusy || applying}>
              {allSelected ? "Clear selection" : "Select all"}
            </button>
            <button type="button" class="btn" disabled={!selectedTargets.length || previewBusy || applying} onclick={() => openPreview(selectedTargets)}>
              Update selected ({selectedTargets.length})
            </button>
          </div>
        {/if}
      </div>
    {/if}
  {/if}

  {#if preview}
    <div class="updates-preview" role="group" aria-label="Update preview">
      <strong>
        {preview.targets.length === 1
          ? `Update ${names[preview.targets[0].current.projectId] ?? preview.targets[0].current.projectId}?`
          : `Update ${preview.targets.length} managed items?`}
      </strong>
      <ul class="preview-targets">
        {#each preview.targets as target (target.current.projectId)}
          <li>
            <span>{names[target.current.projectId] ?? target.current.projectId}: {target.current.displayVersion ?? target.current.projectId} → {target.candidate.versionNumber}</span>
            <span class="update-type">({target.candidate.versionType})</span>
            {#if target.changelog}
              <button type="button" class="changelog-toggle" aria-expanded={expandedChangelog === target.current.projectId} onclick={() => { expandedChangelog = expandedChangelog === target.current.projectId ? null : target.current.projectId; }}>
                Release notes
              </button>
              {#if expandedChangelog === target.current.projectId}
                <pre class="changelog">{target.changelog}</pre>
              {/if}
            {/if}
          </li>
        {/each}
      </ul>
      {#if preview.delta.willInstall.length}<p>Will install: {preview.delta.willInstall.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if preview.delta.newRequirements.length}<p>New dependencies: {preview.delta.newRequirements.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if preview.delta.removedRequirements.length}<p>No longer required: {preview.delta.removedRequirements.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if preview.delta.willRemove.length}<p>Will remove: {preview.delta.willRemove.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if preview.delta.willRetain.length}<p>Retained: {preview.delta.willRetain.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if preview.warnings.length}<p class="preview-warnings">Notes: {preview.warnings.join(" ")}</p>{/if}
      <div class="preview-actions">
        <button type="button" class="btn btn-quiet" disabled={applying} onclick={() => { preview = null; expandedChangelog = null; }}>Cancel</button>
        <button type="button" class="btn" disabled={applying} onclick={approve}>{applying ? "Updating…" : "Approve update"}</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .updates-card { display: grid; gap: var(--space-2); margin-bottom: var(--space-4); font-size: var(--text-secondary); }
  .updates-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
  .updates-subtitle { margin: 0; color: var(--color-text-muted); font-size: var(--text-metadata); }
  .updates-quiet { margin: 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .updates-error { margin: 0; padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: var(--color-error-soft); color: var(--color-error); font-size: var(--text-secondary); }
  .updates-success { margin: 0; color: var(--color-working); font-size: var(--text-metadata); }
  .updates-panel { display: grid; gap: var(--space-2); padding: var(--space-3); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-lg); background: var(--f-panel); box-shadow: var(--f-shadow); }
  .updates-panel-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
  .updates-panel-actions { display: flex; align-items: center; gap: var(--space-2); }
  .updates-summary { margin: 0; color: var(--color-text); font-size: var(--text-body); font-weight: 600; }
  .updates-dismiss { display: grid; place-items: center; width: 26px; height: 26px; padding: 0; border: none; border-radius: var(--radius-sm); background: transparent; color: var(--color-text-secondary); font-size: 18px; line-height: 1; cursor: pointer; }
  .updates-dismiss:hover { background: var(--color-surface-raised); color: var(--color-text); }
  .updates-list { display: grid; gap: var(--space-1); margin: 0; padding: 0; list-style: none; }
  .update-row { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: var(--color-surface-sunken); }
  .update-row-pinned { background: var(--color-surface-raised); }
  .update-row-blocked { background: var(--color-error-soft); }
  .update-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--color-error); }
  .update-main { display: grid; gap: 2px; min-width: 0; }
  .update-name { color: var(--color-text); font-size: var(--text-body); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .update-state { color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .update-type { color: var(--color-text-muted); }
  .update-note { color: var(--color-text-muted); font-size: var(--text-metadata); }
  .update-actions { display: flex; align-items: center; gap: var(--space-2); }
  .badge { padding: 2px var(--space-2); border-radius: var(--radius-sm); background: var(--color-accent-soft); color: var(--color-accent); font-size: var(--text-metadata); font-weight: 600; }
  .updates-bulk { display: flex; justify-content: flex-end; gap: var(--space-2); }
  .updates-preview { display: grid; gap: var(--space-2); padding: var(--space-3); border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface-raised); }
  .updates-preview strong { color: var(--color-text); font-size: var(--text-body); }
  .updates-preview p { margin: 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .preview-targets { display: grid; gap: var(--space-2); margin: 0; padding: 0; list-style: none; font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .preview-targets li { display: grid; gap: var(--space-1); }
  .changelog-toggle { padding: 0; border: none; background: transparent; color: var(--color-accent); font: inherit; font-size: var(--text-metadata); cursor: pointer; text-decoration: underline; }
  .changelog { max-height: 220px; overflow: auto; margin: 0; padding: var(--space-2); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text-secondary); font-size: var(--text-metadata); white-space: pre-wrap; overflow-wrap: anywhere; }
  .preview-warnings { color: var(--color-warning); }
  .preview-actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-2); }

  @media (max-width: 800px) {
    .updates-head, .updates-panel-head { flex-wrap: wrap; }
    .update-row { grid-template-columns: auto minmax(0, 1fr); }
    .update-actions { grid-column: 2; justify-content: flex-start; }
  }
</style>
