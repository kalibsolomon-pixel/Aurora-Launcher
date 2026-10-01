<script lang="ts">
  import { tick } from "svelte";
  import { launcher } from "$lib/launcher/store.svelte";
  import {
    beginRemoval,
    confirmedRemovalId,
    formatModSize,
    visibleMods,
    type ModFilter,
    type ModSort,
    type RemovalCandidate,
  } from "$lib/instances/mods";
  import { applyProviderRemoval, previewProviderRemoval, type ProviderRemovalPreview, getInstanceContentContext, getProviderLifecycle, providerOriginLabel, type InstanceContentContext, type InstanceSummary, type ModEntry, type ProviderLifecycleEntry, type UpdatesReport } from "$lib/backend";
  import ModrinthBrowse from "./ModrinthBrowse.svelte";
  import ProviderLifecycleActions from "./ProviderLifecycleActions.svelte";
  import InstalledArtwork from "./InstalledArtwork.svelte";
  import ContentRecognition from "./ContentRecognition.svelte";
  import ContentUpdates from "./ContentUpdates.svelte";
  import Icon from "$lib/shell/Icon.svelte";

  let { instance }: { instance: InstanceSummary } = $props();
  let query = $state("");
  let filter = $state<ModFilter>("all");
  let sort = $state<ModSort>("name");
  let loadedInstance = $state("");
  let providerRemoval = $state<ProviderRemovalPreview | null>(null);
  let providerRemovalEntry = $state<ModEntry | null>(null);
  let removalBusy = $state(false);
  let removal = $state<RemovalCandidate | null>(null);
  let confirmButton: HTMLButtonElement | null = $state(null);
  let expanded = $state<string | null>(null);
  let view = $state<"installed" | "browse">("installed");
  let lifecycleEntries = $state<ProviderLifecycleEntry[]>([]);
  let lifecycleError = $state("");
  let context = $state<InstanceContentContext | null>(null);
  let updatesReport = $state<UpdatesReport | null>(null);

  async function refreshLifecycle(targetId: string): Promise<void> {
    try {
      const capability = await getInstanceContentContext(targetId);
      if (instance.id === targetId) context = capability;
      if (!capability.modrinthAvailable) {
        if (instance.id === targetId) { lifecycleEntries = []; lifecycleError = ""; }
        return;
      }
      const entries = await getProviderLifecycle(targetId);
      if (instance.id === targetId) { lifecycleEntries = entries; lifecycleError = ""; }
    } catch (reason) {
      if (instance.id === targetId) lifecycleError = reason instanceof Error ? reason.message : "Provider lifecycle is unavailable.";
    }
  }

  async function refreshInstalled(targetId: string): Promise<void> {
    await launcher.runLoadMods(targetId);
    await refreshLifecycle(targetId);
  }

  const providerNames = $derived.by(() => {
    const names: Record<string, string> = {};
    for (const entry of entries) {
      if (entry.provenance?.provider === "modrinth") names[entry.provenance.projectId] = entry.displayName;
    }
    for (const item of lifecycleEntries) {
      if (item.record.provider === "modrinth" && !names[item.record.projectId]) {
        names[item.record.projectId] = item.record.fileName;
      }
    }
    return names;
  });

  const inventory = $derived(launcher.modInventories[instance.id] ?? null);
  const entries = $derived(inventory?.entries ?? []);
  const shown = $derived(visibleMods(entries, query, filter, sort));
  const running = $derived(
    launcher.playProcess?.instanceId === instance.id &&
      launcher.playProcess.status === "running",
  );

  $effect(() => {
    if (loadedInstance !== instance.id) {
      loadedInstance = instance.id;
      removal = null; providerRemoval = null; providerRemovalEntry = null; expanded = null;
      context = null; updatesReport = null;
      view = "installed";
      if (launcher.modInventories[instance.id] === undefined) {
        void launcher.runLoadMods(instance.id);
      }
      void refreshLifecycle(instance.id);
    }
  });

  async function askRemove(entry: ModEntry): Promise<void> {
    if (entry.removalBlockedReason) { lifecycleError = entry.removalBlockedReason; return; }
    if (entry.ownership === "providerManaged" && entry.provenance) {
      removalBusy = true; lifecycleError = "";
      const targetId = instance.id;
      try {
        const preview = await previewProviderRemoval(targetId, "mod", entry.provenance.projectId);
        if (instance.id === targetId) { providerRemoval = preview; providerRemovalEntry = entry; }
      } catch (reason) { lifecycleError = reason instanceof Error ? reason.message : "Removal preview failed."; }
      finally { removalBusy = false; }
      return;
    }
    removal = beginRemoval(entry);
    await tick();
    confirmButton?.focus();
  }

  async function confirmRemove(): Promise<void> {
    const entryId = confirmedRemovalId(removal, entries);
    if (!entryId) {
      removal = null;
      return;
    }
    await launcher.runRemoveMod(instance.id, entryId);
    removal = null;
  }

  async function confirmProviderRemove(): Promise<void> {
    if (!providerRemoval || !providerRemovalEntry?.provenance) return;
    removalBusy = true;
    const targetId = instance.id;
    try {
      await applyProviderRemoval(targetId, "mod", providerRemovalEntry.provenance.projectId, providerRemoval.previewFingerprint);
      providerRemoval = null; providerRemovalEntry = null;
      await refreshInstalled(targetId);
      await launcher.refreshState(); await launcher.refreshPlayReadiness();
    } catch (reason) { lifecycleError = reason instanceof Error ? reason.message : "Removal failed."; }
    finally { removalBusy = false; }
  }

  function cancelRemove(): void {
    const triggerId = removal?.entryId;
    removal = null;
    void tick().then(() => document.getElementById(`remove-${triggerId}`)?.focus());
  }

  function onConfirmationKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      cancelRemove();
    }
  }

  function stateLabel(entry: ModEntry): string {
    if (entry.ownership === "launcherManagedRequired") return "Required";
    if (entry.ownership === "launcherManagedRetained") return "Retained";
    if (entry.ownership === "launcherBootstrap") return entry.enabled ? "" : "Disabled";
    if (entry.ownership === "providerManaged") return entry.enabled ? "" : "Disabled";
    if (entry.fileType === "enabledJar") return "Enabled";
    if (entry.fileType === "disabledJar") return "Disabled";
    if (entry.fileType === "link") return "Link";
    if (entry.fileType === "directory") return "Folder";
    return "Unclassified";
  }
</script>

<section class="mods-panel" aria-labelledby="mods-title">
  <div class="content-view-tabs" role="group" aria-label="Mods view">
    <button type="button" class="btn btn-quiet" aria-pressed={view === "installed"} onclick={() => view = "installed"}>Installed</button>
    {#if context?.modrinthAvailable}<button type="button" class="btn btn-quiet" aria-pressed={view === "browse"} onclick={() => view = "browse"}>Browse Modrinth</button>{/if}
  </div>
  {#if view === "browse"}
    <ModrinthBrowse instanceId={instance.id} instanceName={instance.displayName} minecraftVersion={instance.minecraftVersion} kind="mod" installedProjectIds={entries.filter((entry) => entry.provenance?.provider === "modrinth").map((entry) => entry.provenance!.projectId)} dependencyOnlyProjectIds={lifecycleEntries.filter((entry) => entry.record.contentType === "mod" && !entry.record.explicitlyRetained).map((entry) => entry.record.projectId)} onInstalled={async (targetId, targetKind) => { await refreshInstalled(targetId); if (targetKind !== "mod") await launcher.runLoadContent(targetId, targetKind); await launcher.refreshState(); }} />
  {:else}
  {#if inventory?.missingManaged.length}
    <p class="mods-notice" role="status">{inventory.missingManaged.length} managed mod file{inventory.missingManaged.length === 1 ? " is" : "s are"} missing. Refresh or inspect the instance folder; Aurora will not recreate files automatically.</p>
    <details class="missing-details"><summary>Missing managed files</summary><ul>{#each inventory.missingManaged as record}<li>{record.fileName} · {record.provider}</li>{/each}</ul></details>
  {/if}
  <div class="mods-heading">
    <div>
      <h3 id="mods-title" class="group-title">Mods</h3>
      <p class="group-subtitle">Files for {instance.displayName}.{context?.modsLoadable ? " Browse compatible mods on Modrinth." : context ? " This platform does not load mods. Files here are only stored locally." : ""}</p>
    </div>
    <div class="mods-heading-actions">
      <button
        type="button"
        class="btn btn-quiet"
        onclick={() => launcher.runLoadMods(instance.id)}
        disabled={launcher.modInventoryBusy === instance.id || launcher.modMutationBusy !== null}
      >
        {launcher.modInventoryBusy === instance.id ? "Refreshing…" : "Refresh"}
      </button>
      <button
        type="button"
        class="btn"
        onclick={() => launcher.runOpenModsFolder(instance.id)}
        disabled={launcher.modFolderBusy !== null}
      >
        {launcher.modFolderBusy === instance.id ? "Opening…" : "Open mods folder"}
      </button>
    </div>
  </div>

  {#if running}
    <p class="mods-notice" role="status">
      Minecraft is running. Local changes are for the next launch and do not hot-reload the current game.
    </p>
  {/if}

  {#if context?.modrinthAvailable && view === "installed"}
    <ContentRecognition {instance} kind="mod" onChanged={async () => { await refreshInstalled(instance.id); }} />
    <ContentUpdates instanceId={instance.id} kind="mod" names={providerNames} bind:report={updatesReport} onChanged={async () => { await refreshInstalled(instance.id); await launcher.refreshState(); await launcher.refreshPlayReadiness(); }} />
  {/if}

  {#if launcher.modError}
    <div class="mods-error" role="alert">
      <p>{launcher.modError.message}</p>
      <code>{launcher.modError.code}</code>
      <button type="button" class="btn btn-quiet" onclick={() => launcher.runLoadMods(instance.id)}>
        Retry
      </button>
    </div>
  {/if}
  {#if lifecycleError}<p class="mods-error" role="alert">{lifecycleError}</p>{/if}

  {#if inventory}
    <div class="mods-toolbar">
      <label class="mods-search">
        <span class="field-label">Search mods</span>
        <input bind:value={query} type="search" placeholder="Name, ID, filename, or author" />
      </label>
      <label class="mods-control">
        <span class="field-label">Show</span>
        <select bind:value={filter} aria-label="Filter mods">
          <option value="all">All</option>
          <option value="enabled">Enabled</option>
          <option value="disabled">Disabled</option>
          <option value="warnings">Warnings</option>
        </select>
      </label>
      <label class="mods-control">
        <span class="field-label">Sort</span>
        <select bind:value={sort} aria-label="Sort mods">
          <option value="name">Name</option>
          <option value="state">Enabled first</option>
          <option value="warnings">Warnings first</option>
        </select>
      </label>
    </div>

    {#if entries.length === 0}
      <div class="empty-state mods-empty">
        <h4 class="empty-title">No local mods found</h4>
        <p class="empty-detail">This instance's mods directory is empty.</p>
        <button type="button" class="btn" onclick={() => launcher.runOpenModsFolder(instance.id)}>
          Open mods folder
        </button>
      </div>
    {:else if shown.length === 0}
      <div class="mods-no-results" role="status">
        No mods match the current search and filter.
      </div>
    {:else}
      <div class="mod-list f-surface" aria-label={`${shown.length} local mod entries`}>
        {#each shown as entry (entry.entryId)}
          <article class="mod-row" class:mod-row-disabled={!entry.enabled}>
            <div class="mod-row-main">
              <InstalledArtwork {entry} />
              <div class="mod-identity">
                <div class="mod-title-line">
                  <h4>{entry.displayName}</h4>
                  {#if entry.metadata?.version}
                    <span class="mod-version">{entry.metadata.version}</span>
                  {/if}
                </div>
                <p class="mod-meta">
                  {entry.metadata ? "Fabric" : "Metadata unavailable"}
                  {#if entry.metadata?.authors.length}
                    · By {entry.metadata.authors.slice(0, 2).join(", ")}{entry.metadata.authors.length > 2 ? "…" : ""}
                  {/if}
                </p>
                {#if entry.warnings.length}
                  <p class="mod-warning">
                    <span aria-hidden="true">⚠</span>
                    {entry.warnings[0]?.message}
                    {#if entry.warnings.length > 1}
                      <span> (+{entry.warnings.length - 1} more)</span>
                    {/if}
                  </p>
                {/if}
              </div>
            </div>

            <div class="mod-row-actions">
              <span class="mod-state" class:mod-state-required={entry.ownership === "launcherManagedRequired"}>
                {launcher.modMutationBusy === entry.entryId ? "Changing…" : stateLabel(entry)}
              </span>
              {#if entry.provenance?.provider === "modrinth"}
                {@const update = updatesReport?.entries.find((item) => item.contentType === "mod" && item.projectId === entry.provenance?.projectId)}
                {#if update?.status === "updateAvailable"}
                  <span class="row-update-badge">Update</span>
                {:else if update?.status === "pinnedUpdateAvailable"}
                  <span class="row-update-badge badge-pinned" title={update.detail ?? "Pinned with an update available"}>Pinned</span>
                {:else if update?.status === "blocked"}
                  <span class="row-update-badge badge-blocked" title={update.detail ?? "Updating is blocked"}>Blocked</span>
                {/if}
              {/if}
              {#if entry.canToggle}
                <button
                  type="button"
                  role="switch"
                  class="mod-switch"
                  aria-checked={entry.enabled}
                  aria-label={`${entry.enabled ? "Disable" : "Enable"} ${entry.displayName}`}
                  disabled={launcher.modMutationBusy !== null}
                  onclick={() => launcher.runSetModEnabled(instance.id, entry.entryId, !entry.enabled)}
                >
                  <span aria-hidden="true"></span>
                </button>
              {:else}
                <span class="protected-marker" title={entry.actionBlockedReason ?? undefined}>
                  {entry.ownership === "launcherManagedRequired" ? "Protected" : entry.ownership === "providerManaged" ? "No toggle" : "Unavailable"}
                </span>
              {/if}
              {#if entry.canRemove || entry.ownership === "providerManaged"}
                <button id="remove-{entry.entryId}" type="button" class="mod-trash"
                  aria-label={`Remove ${entry.displayName}`} title={entry.removalBlockedReason ?? `Remove ${entry.displayName}`}
                  disabled={launcher.modMutationBusy !== null || removalBusy || !!entry.removalBlockedReason}
                  onclick={() => askRemove(entry)}>
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7" /></svg>
                </button>
              {/if}
              <button
                type="button"
                class="mod-details-toggle"
                aria-expanded={expanded === entry.entryId}
                aria-controls="mod-details-{entry.entryId}"
                aria-label={`Details for ${entry.displayName}`}
                title="Details"
                onclick={() => { expanded = expanded === entry.entryId ? null : entry.entryId; }}
              >
                <span class="mod-details-chevron" class:open={expanded === entry.entryId}><Icon name="chevron" size={14} /></span>
              </button>
            </div>

            {#if expanded === entry.entryId}<div class="mod-details" id="mod-details-{entry.entryId}">
              <dl>
                <div><dt>File</dt><dd>{entry.fileName}</dd></div>
                <div><dt>Source</dt><dd>{entry.ownership === "launcherManagedRequired" ? "Aurora · required" : entry.ownership === "providerManaged" ? `Modrinth${entry.provenance ? ` · ${entry.provenance.displayVersion ?? entry.provenance.versionId}` : ""}` : entry.ownership === "launcherBootstrap" ? "Installed initially by Aurora; user controlled" : entry.ownership === "userManaged" ? "Local mod" : "Unclassified"}</dd></div>
                {#if entry.provenance}<div><dt>Origin</dt><dd>{providerOriginLabel(entry.provenance.origin)}</dd></div>{/if}
                {#if entry.metadata}
                  <div><dt>Mod ID</dt><dd>{entry.metadata.id}</dd></div>
                  {#if entry.metadata.environment}<div><dt>Environment</dt><dd>{entry.metadata.environment}</dd></div>{/if}
                  {#if entry.metadata.depends.length}<div><dt>Requires</dt><dd>{entry.metadata.depends.map((item) => `${item.modId} ${item.requirement}`).join(", ")}</dd></div>{/if}
                {/if}
                {#if entry.removalBlockedReason}
                  <div><dt>Removal</dt><dd>{entry.removalBlockedReason}</dd></div>
                {/if}
                {#if entry.warnings.length}
                  <div class="details-warnings"><dt>Metadata warnings</dt><dd><ul>{#each entry.warnings as warning}<li>{warning.message}</li>{/each}</ul></dd></div>
                {/if}
              </dl>
              {#if entry.provenance?.provider === "modrinth"}
                {@const lifecycle = lifecycleEntries.find((item) => item.record.contentType === "mod" && item.record.projectId === entry.provenance?.projectId)}
                {#if lifecycle}<ProviderLifecycleActions instanceId={instance.id} kind="mod" title={entry.displayName} {lifecycle} showRemoval={false} onChanged={async () => { await refreshInstalled(instance.id); }} />{/if}
              {/if}
            </div>{/if}

            {#if providerRemovalEntry?.entryId === entry.entryId && providerRemoval}
              <div class="remove-confirmation" role="group" aria-label={`Remove ${entry.displayName} preview`}>
                <div><strong>{providerRemoval.delta.willRemove.length ? `Remove ${entry.displayName}?` : `Stop retaining ${entry.displayName}?`}</strong>
                  <p>Will remove: {providerRemoval.delta.willRemove.map(item => item.fileName).join(", ") || "No files; required content stays installed."}</p>
                  {#if providerRemoval.delta.willRetain.length}<p>Retained: {providerRemoval.delta.willRetain.map(item => item.fileName).join(", ")}</p>{/if}
                </div>
                <div class="remove-actions"><button type="button" class="btn btn-quiet" disabled={removalBusy} onclick={() => { providerRemoval = null; providerRemovalEntry = null; }}>Cancel</button>
                  <button type="button" class="btn btn-danger" disabled={removalBusy} onclick={confirmProviderRemove}>Approve removal</button></div>
              </div>
            {/if}
            {#if removal?.entryId === entry.entryId}
              <div
                class="remove-confirmation"
                role="group"
                aria-labelledby="remove-title-{entry.entryId}"
              >
                <div>
                  <strong id="remove-title-{entry.entryId}">Remove {entry.displayName}?</strong>
                  <p>This permanently deletes <span>{entry.fileName}</span>. This cannot be undone.</p>
                </div>
                <div class="remove-actions">
                  <button type="button" class="btn btn-quiet" onclick={cancelRemove} onkeydown={onConfirmationKeydown}>Cancel</button>
                  <button
                    bind:this={confirmButton}
                    type="button"
                    class="btn btn-danger"
                    disabled={launcher.modMutationBusy !== null}
                    onclick={confirmRemove}
                    onkeydown={onConfirmationKeydown}
                  >
                    {launcher.modMutationBusy === entry.entryId ? "Removing…" : "Remove permanently"}
                  </button>
                </div>
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  {:else if launcher.modInventoryBusy === instance.id}
    <div class="mods-loading" aria-live="polite">
      <span class="spinner" aria-hidden="true"></span>
      <span>Inspecting local mod files…</span>
    </div>
  {/if}
  {/if}
</section>

<style>
  .mod-identity .mod-meta { margin-bottom: 0; }
  .mod-row-main { min-height: 52px; }
  .mod-details-toggle { display: grid; place-items: center; width: 28px; height: 28px; padding: 0; border: none; border-radius: var(--radius-sm); background: transparent; color: var(--color-text-secondary); cursor: pointer; }
  .mod-details-toggle:hover { background: var(--color-surface-raised); color: var(--color-text); }
  .mod-details-chevron { display: grid; place-items: center; transition: transform var(--motion-fast) var(--motion-ease); }
  .mod-details-chevron.open { transform: rotate(180deg); }
  .content-view-tabs { display: flex; gap: var(--space-2); margin-bottom: var(--space-4); }
  .content-view-tabs [aria-pressed="true"] { color: var(--color-text); background: var(--color-surface-raised); }
  .mods-panel { min-width: 0; }
  .mods-heading, .mods-heading-actions, .mods-toolbar, .mod-title-line, .mod-row-actions, .remove-actions {
    display: flex;
    align-items: center;
  }
  .mods-heading { justify-content: space-between; gap: var(--space-4); margin-bottom: var(--space-4); }
  .mods-heading-actions, .remove-actions { gap: var(--space-2); flex-wrap: wrap; }
  .mods-notice { margin: 0 0 var(--space-3); padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: var(--color-working-soft); color: var(--color-working); font-size: var(--text-secondary); }
  .missing-details { margin: 0 0 var(--space-4); font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .mods-error { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-3); padding: var(--space-3); border-radius: var(--radius-sm); background: var(--color-error-soft); color: var(--color-error); font-size: var(--text-secondary); flex-wrap: wrap; }
  .mods-error p { margin: 0; flex: 1; }
  .mods-error code { color: var(--color-text-muted); font-size: var(--text-metadata); }
  .mods-toolbar { align-items: end; gap: var(--space-3); margin-bottom: var(--space-4); }
  .mods-search, .mods-control { display: grid; gap: var(--space-1); }
  .mods-search { flex: 1; min-width: 180px; }
  .mods-control { width: 128px; }
  .mods-toolbar input, .mods-toolbar select { width: 100%; padding: var(--space-2) var(--space-3); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text); font: inherit; font-size: var(--text-body); }
  .mod-list { overflow: visible; border: 1px solid var(--color-surface-edge); border-radius: var(--radius-lg); background: var(--f-panel); box-shadow: var(--f-shadow); }
  .mod-row { position: relative; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--space-2) var(--space-4); padding: var(--space-2) var(--space-4); border-bottom: 1px solid var(--color-border); min-width: 0; }
  .mod-row:last-child { border-bottom: none; }
  .mod-row-disabled .mod-identity { opacity: 0.7; }
  .mod-row-main { display: flex; gap: var(--space-3); min-width: 0; }
  .mod-identity { min-width: 0; }
  .mod-title-line { gap: var(--space-2); min-width: 0; }
  .mod-title-line h4 { margin: 0; overflow: hidden; color: var(--color-text); font-size: var(--text-body); font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
  .mod-version { color: var(--color-text-muted); font-size: var(--text-metadata); }
  .mod-meta, .mod-warning { margin: 2px 0 0; }
  .mod-meta { color: var(--color-text-secondary); font-size: var(--text-secondary); }
  .mod-warning { color: var(--color-warning); font-size: var(--text-metadata); line-height: 1.4; }
  .mod-row-actions { align-self: start; justify-content: flex-end; gap: var(--space-2); }
  .mod-state, .protected-marker { color: var(--color-text-secondary); font-size: var(--text-metadata); font-weight: 500; white-space: nowrap; }
  .row-update-badge { padding: 1px var(--space-2); border-radius: var(--radius-sm); background: var(--color-accent-soft); color: var(--color-accent); font-size: var(--text-metadata); font-weight: 600; white-space: nowrap; }
  .row-update-badge.badge-pinned { background: var(--color-working-soft); color: var(--color-working); }
  .row-update-badge.badge-blocked { background: var(--color-error-soft); color: var(--color-error); }
  .mod-state-required { color: var(--color-working); }
  .protected-marker { color: var(--color-text-muted); }
  .mod-switch { position: relative; width: 38px; height: 22px; padding: 2px; border: 1px solid var(--color-border-strong); border-radius: 999px; background: var(--color-surface-sunken); cursor: pointer; }
  .mod-switch span { display: block; width: 16px; height: 16px; border-radius: 50%; background: var(--color-text-muted); transition: transform var(--motion-fast) var(--motion-ease), background-color var(--motion-fast) var(--motion-ease); }
  .mod-switch[aria-checked="true"] { border-color: var(--color-accent); background: var(--color-accent-soft); }
  .mod-switch[aria-checked="true"] span { transform: translateX(16px); background: var(--color-accent); }
  .mod-switch:disabled { opacity: 0.5; cursor: progress; }
  .mod-trash { width: 28px; height: 28px; padding: 0; display: grid; place-items: center; border: none; border-radius: var(--radius-sm); background: transparent; color: var(--color-text-secondary); cursor: pointer; }
  .mod-trash:hover { background: var(--color-error-soft); color: var(--color-error); }
  .mod-trash:disabled { opacity: .45; cursor: default; }

  .mod-details { grid-column: 1 / -1; margin-left: 46px; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .mod-details dl { display: grid; gap: var(--space-1); margin: var(--space-2) 0 0; }
  .mod-details dl > div { display: grid; grid-template-columns: 88px minmax(0, 1fr); gap: var(--space-2); }
  .mod-details dt { color: var(--color-text-muted); }
  .mod-details dd { margin: 0; overflow-wrap: anywhere; }
  .mod-details ul { margin: 0; padding-left: var(--space-4); }
  .remove-confirmation { grid-column: 1 / -1; display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); margin-top: var(--space-2); padding: var(--space-3); border-radius: var(--radius-md); background: var(--color-error-soft); }
  .remove-confirmation strong { color: var(--color-text); font-size: var(--text-body); }
  .remove-confirmation p { margin: 2px 0 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .remove-confirmation p span { color: var(--color-text); }
  .mods-loading, .mods-no-results { display: flex; align-items: center; gap: var(--space-3); min-height: 96px; color: var(--color-text-secondary); font-size: var(--text-body); }
  .mods-no-results { justify-content: center; }
  .mods-empty { padding-top: var(--space-5); }

  @media (max-width: 800px) {
    .mods-heading { align-items: flex-start; flex-direction: column; }
    .mods-toolbar { align-items: stretch; flex-wrap: wrap; }
    .mods-search { flex-basis: 100%; }
    .mods-control { flex: 1; min-width: 120px; }
    .mod-row { grid-template-columns: minmax(0, 1fr); }
    .mod-row-actions { justify-content: flex-start; margin-left: 46px; }
    .remove-confirmation { align-items: flex-start; flex-direction: column; }
  }

  @media (prefers-reduced-motion: reduce) {
    .mod-switch span { transition: none; }
    .mod-details-chevron { transition: none; }
  }
</style>
