<script lang="ts">
  import {
    providerOriginLabel,
    registerRecoveredContent,
    scanInstanceContent,
    type ContentType,
    type InstanceSummary,
    type ProviderRecord,
    type RecognitionScan,
    type ScanCandidate,
  } from "$lib/backend";
  import { formatModSize } from "./mods";

  let { instance, kind, onChanged }: {
    instance: InstanceSummary;
    kind: ContentType;
    onChanged: () => Promise<void>;
  } = $props();

  let open = $state(false);
  let scanning = $state(false);
  let adopting = $state(false);
  let scan = $state<RecognitionScan | null>(null);
  let selected = $state<Set<string>>(new Set());
  let error = $state("");
  let success = $state("");
  let adoptedRecords = $state<ProviderRecord[]>([]);

  const directoryLabel = $derived(kind === "mod" ? "mods" : kind === "resourcePack" ? "resource packs" : "shaders");

  async function runScan(): Promise<void> {
    scanning = true; error = ""; success = ""; scan = null; adoptedRecords = [];
    const targetId = instance.id;
    try {
      const result = await scanInstanceContent(targetId, kind);
      if (instance.id !== targetId) return;
      scan = result;
      selected = new Set(result.candidates.filter((candidate) => candidate.status === "recognized").map((candidate) => candidate.fileName));
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "The recognition scan failed.";
    } finally { scanning = false; }
  }

  function toggle(candidate: ScanCandidate, checked: boolean): void {
    const next = new Set(selected);
    if (checked) next.add(candidate.fileName); else next.delete(candidate.fileName);
    selected = next;
  }

  async function adopt(): Promise<void> {
    if (!scan || selected.size === 0) return;
    adopting = true; error = ""; success = "";
    const approval = {
      instanceId: scan.instanceId,
      contentType: scan.contentType,
      inventoryRevision: scan.inventoryRevision,
      files: scan.candidates
        .filter((candidate) => selected.has(candidate.fileName) && candidate.sha512)
        .map((candidate) => ({ fileName: candidate.fileName, sha512: candidate.sha512! })),
    };
    try {
      adoptedRecords = await registerRecoveredContent(approval);
      success = adoptedRecords.length === 1
        ? "One local file is now provider-managed. The file was not modified."
        : `${adoptedRecords.length} local files are now provider-managed. No file was modified.`;
      selected = new Set();
      await onChanged();
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Adoption failed. No files were modified.";
    } finally { adopting = false; }
  }

  function close(): void {
    open = false; scan = null; error = ""; success = ""; adoptedRecords = [];
  }

  function statusLabel(candidate: ScanCandidate): string {
    if (candidate.status === "recognized") return "Recognized";
    if (candidate.status === "unrecognized") return "Not on Modrinth";
    return "Skipped";
  }
</script>

<div class="recognition">
  {#if !open}
    <button type="button" class="btn btn-quiet" onclick={() => { open = true; void runScan(); }}>
      Recognize local files
    </button>
  {:else}
    <div class="recognition-panel f-surface" role="group" aria-label="Recognize local files on Modrinth">
      <div class="recognition-heading">
        <div>
          <h4>Recognize local files</h4>
          <p class="recognition-subtitle">Match existing {directoryLabel} against Modrinth by cryptographic hash. Recognized files can be adopted into provider management without changing their bytes.</p>
        </div>
        <div class="recognition-heading-actions">
          <button type="button" class="btn btn-quiet" disabled={scanning || adopting} onclick={() => void runScan()}>{scanning ? "Scanning…" : "Rescan"}</button>
          <button type="button" class="btn btn-quiet" disabled={scanning || adopting} onclick={close}>Close</button>
        </div>
      </div>

      {#if error}<p class="inline-message inline-message-error" role="alert">{error}</p>{/if}
      {#if success}<p class="recognition-success" role="status">{success}</p>{/if}

      {#if scanning}
        <div class="recognition-loading"><span class="spinner" aria-hidden="true"></span> Hashing local files and checking Modrinth…</div>
      {:else if scan}
        {@const recognized = scan.candidates.filter((candidate) => candidate.status === "recognized")}
        {@const unrecognized = scan.candidates.filter((candidate) => candidate.status === "unrecognized")}
        {@const skipped = scan.candidates.filter((candidate) => candidate.status === "skipped")}
        <p class="recognition-summary" role="status">
          {recognized.length} recognized · {unrecognized.length} not on Modrinth · {skipped.length} skipped
        </p>

        {#if scan.candidates.length === 0}
          <div class="empty-state"><h4 class="empty-title">Nothing to recognize</h4><p class="empty-detail">No eligible local files were found in this content folder.</p></div>
        {:else}
          <ul class="recognition-list">
            {#each scan.candidates as candidate (candidate.fileName)}
              <li class="recognition-row" class:row-recognized={candidate.status === "recognized"} class:row-unrecognized={candidate.status === "unrecognized"} class:row-skipped={candidate.status === "skipped"}>
                <div class="recognition-main">
                  {#if candidate.status === "recognized"}
                    <input
                      id={`recognize-${instance.id}-${kind}-${candidate.fileName}`}
                      type="checkbox"
                      checked={selected.has(candidate.fileName)}
                      disabled={adopting}
                      onchange={(event) => toggle(candidate, event.currentTarget.checked)}
                    />
                  {:else}
                    <span class="recognition-marker" aria-hidden="true">—</span>
                  {/if}
                  <div class="recognition-identity">
                    <label for={`recognize-${instance.id}-${kind}-${candidate.fileName}`}>
                      <span class="recognition-name">{candidate.fileName}</span>
                      <span class="recognition-status">{statusLabel(candidate)}</span>
                    </label>
                    {#if candidate.status === "recognized" && candidate.recognition}
                      <p class="recognition-meta">
                        {candidate.recognition.versionNumber}
                        {#if candidate.filenameMatches === false}<span> · filename differs from the published {candidate.recognition.fileName}</span>{/if}
                        {#if candidate.disabled}<span> · currently disabled</span>{/if}
                        {#if formatModSize(candidate.sizeBytes)}<span> · {formatModSize(candidate.sizeBytes)}</span>{/if}
                      </p>
                    {:else if candidate.reason}
                      <p class="recognition-meta">{candidate.reason}</p>
                    {/if}
                  </div>
                </div>
                {#if candidate.recognition}
                  <details class="recognition-details"><summary>Details</summary>
                    <dl>
                      <div><dt>Project</dt><dd>{candidate.recognition.projectId}</dd></div>
                      <div><dt>Version</dt><dd>{candidate.recognition.versionNumber} ({candidate.recognition.versionType})</dd></div>
                      <div><dt>Published filename</dt><dd>{candidate.recognition.fileName}</dd></div>
                      {#if candidate.sha512}<div class="hash-row"><dt>SHA-512</dt><dd>{candidate.sha512}</dd></div>{/if}
                    </dl>
                  </details>
                {/if}
              </li>
            {/each}
          </ul>

          {#if recognized.length}
            <div class="recognition-actions">
              <button type="button" class="btn" disabled={adopting || selected.size === 0} onclick={() => void adopt()}>
                {adopting ? "Adopting…" : `Adopt ${selected.size} file${selected.size === 1 ? "" : "s"}`}
              </button>
              <span class="recognition-note">Adoption records provider identity only. Files are never renamed, moved, replaced, or downloaded.</span>
            </div>
          {/if}
          {#if adoptedRecords.length}
            <details class="recognition-adopted"><summary>Adopted files</summary>
              <ul>{#each adoptedRecords as record}<li>{record.fileName} · {providerOriginLabel(record.origin)}</li>{/each}</ul>
            </details>
          {/if}
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .recognition-panel { display: grid; gap: var(--space-3); padding: var(--space-4); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-lg); background: var(--f-panel); box-shadow: var(--f-shadow); margin-bottom: var(--space-4); }
  .recognition-heading { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--space-3); flex-wrap: wrap; }
  .recognition-heading h4 { margin: 0; font-size: var(--text-body); }
  .recognition-subtitle { margin: var(--space-1) 0 0; color: var(--color-text-secondary); font-size: var(--text-metadata); max-width: 60ch; }
  .recognition-heading-actions { display: flex; gap: var(--space-2); flex-wrap: wrap; }
  .recognition-loading { display: flex; align-items: center; gap: var(--space-3); min-height: 64px; color: var(--color-text-secondary); }
  .recognition-summary { margin: 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .recognition-list { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--space-1); }
  .recognition-row { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--space-1) var(--space-3); padding: var(--space-2) var(--space-3); border: 1px solid var(--color-border); border-radius: var(--radius-sm); }
  .row-recognized { border-color: var(--color-border); background: var(--color-surface-sunken); }
  .row-unrecognized, .row-skipped { opacity: 0.85; }
  .recognition-main { display: flex; align-items: flex-start; gap: var(--space-3); min-width: 0; }
  .recognition-marker { width: 13px; text-align: center; color: var(--color-text-muted); }
  .recognition-identity { min-width: 0; flex: 1; }
  .recognition-identity label { display: flex; align-items: baseline; gap: var(--space-2); flex-wrap: wrap; cursor: pointer; }
  .row-unrecognized .recognition-identity label, .row-skipped .recognition-identity label { cursor: default; }
  .recognition-name { color: var(--color-text); font-size: var(--text-body); overflow-wrap: anywhere; }
  .recognition-status { color: var(--color-text-muted); font-size: var(--text-metadata); }
  .row-recognized .recognition-status { color: var(--color-working); }
  .recognition-meta { margin: 2px 0 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .recognition-details { font-size: var(--text-metadata); }
  .recognition-details summary { cursor: pointer; color: var(--color-text-secondary); }
  .recognition-details dl { margin: var(--space-2) 0; display: grid; gap: var(--space-1); }
  .recognition-details dl > div { display: grid; grid-template-columns: 130px minmax(0, 1fr); gap: var(--space-2); }
  .recognition-details dt { color: var(--color-text-muted); }
  .recognition-details dd { margin: 0; overflow-wrap: anywhere; }
  .hash-row dd { font-family: var(--font-mono, monospace); font-size: var(--text-metadata); }
  .recognition-actions { display: flex; align-items: center; gap: var(--space-3); flex-wrap: wrap; }
  .recognition-note { color: var(--color-text-muted); font-size: var(--text-metadata); max-width: 52ch; }
  .recognition-success { margin: 0; padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: var(--color-working-soft); color: var(--color-working); font-size: var(--text-metadata); }
  .recognition-adopted { font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .recognition-adopted summary { cursor: pointer; }
  .recognition-adopted ul { margin: var(--space-1) 0 0; padding-left: var(--space-4); }
</style>
