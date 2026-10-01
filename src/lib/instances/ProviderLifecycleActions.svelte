<script lang="ts">
  import {
    applyModrinthUpdate, applyProviderRemoval, checkModrinthUpdate,
    previewModrinthUpdate, previewProviderRemoval, providerOriginLabel,
    setProviderUpdatePolicy, updateChannelDescription, updateChannelLabel,
    UPDATE_CHANNELS, type ContentType, type ModrinthVersionChoice,
    type ProviderLifecycleEntry, type ProviderRemovalPreview,
    type ProviderUpdatePreview, type UpdateChannel,
  } from "$lib/backend";

  let { instanceId, kind, title, lifecycle, onChanged, showRemoval = true }: {
    showRemoval?: boolean;
    instanceId: string;
    kind: ContentType;
    title: string;
    lifecycle: ProviderLifecycleEntry;
    onChanged: () => Promise<void>;
  } = $props();

  let busy = $state<"check" | "preview" | "update" | "remove" | "policy" | null>(null);
  let candidate = $state<ModrinthVersionChoice | null>(null);
  let checked = $state(false);
  let updatePreview = $state<ProviderUpdatePreview | null>(null);
  let removalPreview = $state<ProviderRemovalPreview | null>(null);
  let error = $state("");
  let success = $state("");
  let showChangelog = $state(false);
  const record = $derived(lifecycle.record);
  const requiredBy = $derived(lifecycle.requiredBy);

  async function check(): Promise<void> {
    busy = "check"; error = ""; success = ""; candidate = null; checked = false; updatePreview = null; showChangelog = false;
    try {
      candidate = await checkModrinthUpdate(instanceId, kind, record.projectId);
      checked = true;
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Update check failed.";
    } finally { busy = null; }
  }

  async function previewUpdate(): Promise<void> {
    busy = "preview"; error = ""; success = ""; removalPreview = null; showChangelog = false;
    try {
      updatePreview = await previewModrinthUpdate(instanceId, kind, record.projectId);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Update preview failed.";
    } finally { busy = null; }
  }

  async function update(): Promise<void> {
    if (!updatePreview) return;
    busy = "update"; error = "";
    try {
      await applyModrinthUpdate(instanceId, kind, record.projectId, updatePreview.previewFingerprint);
      updatePreview = null; candidate = null; checked = false; showChangelog = false;
      await onChanged();
      success = `${title} updated.`;
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Update failed. The previous installation was retained.";
    } finally { busy = null; }
  }

  async function previewRemove(): Promise<void> {
    busy = "preview"; error = ""; success = ""; updatePreview = null;
    try {
      removalPreview = await previewProviderRemoval(instanceId, kind, record.projectId);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Removal preview failed.";
    } finally { busy = null; }
  }

  async function remove(): Promise<void> {
    if (!removalPreview) return;
    busy = "remove"; error = "";
    try {
      await applyProviderRemoval(instanceId, kind, record.projectId, removalPreview.previewFingerprint);
      removalPreview = null; updatePreview = null;
      await onChanged();
      success = `${title} removal completed.`;
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Removal failed. Installed content was retained.";
    } finally { busy = null; }
  }

  async function changePolicy(change: { pinned?: boolean; channel?: UpdateChannel }): Promise<void> {
    busy = "policy"; error = "";
    try {
      const updated = await setProviderUpdatePolicy(instanceId, kind, record.projectId, change);
      lifecycle.record.pinned = updated.pinned;
      lifecycle.record.updateChannel = updated.updateChannel;
      candidate = null; checked = false; updatePreview = null;
      success = change.channel !== undefined
        ? `Release policy set to ${updateChannelLabel(updated.updateChannel)}. Check for updates again to apply it.`
        : updated.pinned
          ? `${title} pinned to its current version.`
          : `${title} unpinned.`;
      await onChanged();
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Policy change failed.";
    } finally { busy = null; }
  }
</script>

<div class="provider-lifecycle" aria-label={`Modrinth details for ${title}`}>
  <p class="lifecycle-summary">
    Managed via Modrinth · {record.displayVersion ?? record.versionId}
    {#if record.pinned}<span class="pin-marker">Pinned</span>{/if}
    {#if record.updateChannel !== "stable"}<span class="channel-marker">{updateChannelLabel(record.updateChannel)} releases</span>{/if}
  </p>
  <details class="relationships">
    <summary>Provider details</summary>
    <p>{providerOriginLabel(record.origin)}. {record.explicitlyRetained ? "Explicitly retained" : "Installed as a required dependency"}.</p>
    <p>Requires: {lifecycle.requires.length ? lifecycle.requires.map((item) => item.fileName).join(", ") : "No provider-managed dependencies"}</p>
    <p>Required by: {requiredBy.length ? requiredBy.map((parent) => parent.fileName).join(", ") : "No installed provider content"}</p>
    {#if record.explicitlyRetained}
      <div class="policy-row">
        <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={() => changePolicy({ pinned: !record.pinned })}>
          {busy === "policy" ? "Saving…" : record.pinned ? "Unpin" : "Pin this version"}
        </button>
        <label class="policy-channel">
          <span>Release policy</span>
          <select
            disabled={busy !== null}
            value={record.updateChannel}
            aria-label={`Release policy for ${title}`}
            onchange={(event) => changePolicy({ channel: event.currentTarget.value as UpdateChannel })}
          >
            {#each UPDATE_CHANNELS as channel (channel)}
              <option value={channel}>{updateChannelLabel(channel)} — {updateChannelDescription(channel)}</option>
            {/each}
          </select>
        </label>
      </div>
      <p class="policy-note">{record.pinned ? "Pinned content never advances through update actions; unpin first to update it here." : "Pinning keeps this exact version until you unpin it."}</p>
      <div class="lifecycle-actions">
        <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={check}>{busy === "check" ? "Checking…" : "Check for updates"}</button>
        {#if checked}
          <span role="status">
            {#if candidate}
              {record.pinned ? `Pinned — update available: ${candidate.versionNumber} (${candidate.versionType})` : `Update available: ${candidate.versionNumber} (${candidate.versionType})`}
            {:else}
              No newer compatible version under the {updateChannelLabel(record.updateChannel)} policy.
            {/if}
          </span>
        {/if}
        {#if candidate && !record.pinned}<button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={previewUpdate}>Update…</button>{/if}
      </div>
    {/if}
    {#if showRemoval && (record.explicitlyRetained || !requiredBy.length)}
      <div class="lifecycle-actions">
        <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={previewRemove}>Remove…</button>
      </div>
    {/if}
  </details>
  {#if error}<p class="lifecycle-error" role="alert">{error}</p>{/if}
  {#if success}<p role="status">{success}</p>{/if}

  {#if updatePreview}
    <div class="lifecycle-preview" role="group" aria-label={`Update ${title} preview`}>
      <strong>{title}: {updatePreview.current.displayVersion ?? "installed"} → {updatePreview.candidate.versionNumber}</strong>
      <p>Target release type: {updatePreview.candidate.versionType} · policy {updateChannelLabel(updatePreview.channel)}</p>
      {#if updatePreview.changelog}
        <button type="button" class="changelog-toggle" aria-expanded={showChangelog} onclick={() => { showChangelog = !showChangelog; }}>Release notes</button>
        {#if showChangelog}<pre class="changelog">{updatePreview.changelog}</pre>{/if}
      {/if}
      {#if updatePreview.delta.willInstall.length}<p>Will install: {updatePreview.delta.willInstall.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if updatePreview.delta.newRequirements.length}<p>New dependencies: {updatePreview.delta.newRequirements.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if updatePreview.delta.removedRequirements.length}<p>No longer required: {updatePreview.delta.removedRequirements.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if !updatePreview.delta.newRequirements.length && !updatePreview.delta.removedRequirements.length}<p>Required dependencies: unchanged.</p>{/if}
      {#if updatePreview.delta.willRemove.length}<p>Will remove: {updatePreview.delta.willRemove.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if updatePreview.delta.willRetain.length}<p>Retained: {updatePreview.delta.willRetain.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if updatePreview.warnings.length}<p>Notes: {updatePreview.warnings.join(" ")}</p>{/if}
      <div class="preview-actions">
        <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={() => updatePreview = null}>Cancel</button>
        <button type="button" class="btn" disabled={busy !== null} onclick={update}>{busy === "update" ? "Updating…" : "Approve update"}</button>
      </div>
    </div>
  {/if}
  {#if removalPreview}
    <div class="lifecycle-preview" role="group" aria-label={`Remove ${title} preview`}>
      <strong>{removalPreview.delta.willRemove.length ? `Remove ${title}?` : `Stop explicitly retaining ${title}?`}</strong>
      {#if !removalPreview.delta.willRemove.length && requiredBy.length}<p>The file stays installed because {requiredBy.map((parent) => parent.fileName).join(", ")} still requires it.</p>{/if}
      {#if removalPreview.delta.willRemove.length}<p>Will remove: {removalPreview.delta.willRemove.map((item) => item.fileName).join(", ")}</p>{/if}
      {#if removalPreview.delta.willRetain.length}<p>Retained because still required or explicitly installed: {removalPreview.delta.willRetain.map((item) => item.fileName).join(", ")}</p>{/if}
      <div class="preview-actions">
        <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={() => removalPreview = null}>Cancel</button>
        <button type="button" class="btn btn-danger" disabled={busy !== null} onclick={remove}>{busy === "remove" ? "Removing…" : "Approve removal"}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .provider-lifecycle { display: grid; gap: var(--space-2); margin-top: var(--space-2); font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .lifecycle-summary { margin: 0; }
  .pin-marker { display: inline-block; margin-left: var(--space-2); padding: 1px var(--space-2); border-radius: var(--radius-sm); background: var(--color-accent-soft); color: var(--color-accent); font-weight: 600; }
  .channel-marker { display: inline-block; margin-left: var(--space-2); padding: 1px var(--space-2); border-radius: var(--radius-sm); background: var(--color-surface-raised); color: var(--color-text-secondary); }
  .lifecycle-actions { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; margin-top: var(--space-2); }
  .relationships summary { cursor: pointer; color: var(--color-text-secondary); }
  .relationships p { margin: var(--space-1) 0; }
  .policy-row { display: flex; align-items: center; gap: var(--space-3); flex-wrap: wrap; margin-top: var(--space-2); }
  .policy-channel { display: grid; gap: 2px; }
  .policy-channel span { color: var(--color-text-muted); }
  .policy-channel select { padding: var(--space-1) var(--space-2); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text); font: inherit; }
  .policy-note { color: var(--color-text-muted); }
  .changelog-toggle { padding: 0; border: none; background: transparent; color: var(--color-accent); font: inherit; font-size: var(--text-metadata); cursor: pointer; text-decoration: underline; }
  .changelog { max-height: 220px; overflow: auto; margin: var(--space-1) 0 0; padding: var(--space-2); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text-secondary); font-size: var(--text-metadata); white-space: pre-wrap; overflow-wrap: anywhere; }
  .lifecycle-preview { padding: var(--space-3); border: 1px solid var(--color-border); border-radius: var(--radius-sm); background: var(--color-surface-raised); }
  .lifecycle-preview p { margin: var(--space-1) 0; }
  .preview-actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-2); }
  .lifecycle-error { color: var(--color-error); }
</style>
