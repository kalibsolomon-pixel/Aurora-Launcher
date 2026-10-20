<script lang="ts">
  import Tooltip from '$lib/shell/Tooltip.svelte';
  import { untrack } from "svelte";
  import { launcher } from "$lib/launcher/store.svelte";
  import { configurationRequiresInstall, draftIsDirty, instanceProgressLabel, runtimeProgressLabel } from "$lib/launcher/instanceStatus";
  import { deleteInstance, type InstanceConfiguration, type InstanceSummary } from "$lib/backend";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import { maintainInstallationBackups } from "$lib/backend";
  import { backupRetentionMessage } from "./backupRetention";
  let backupBusy = $state(false);
  let backupMessage = $state("");
  let backupTarget = $state("");
  async function maintainBackups() {
    const id = instance.id;
    backupBusy = true; backupTarget = id; backupMessage = "Checking recovery backups…";
    try { backupMessage = backupRetentionMessage(await maintainInstallationBackups(id)); }
    catch (reason) { backupMessage = reason instanceof Error ? reason.message : "Backup maintenance could not finish. Recovery data remains protected."; }
    finally { backupBusy = false; }
  }
  let confirmation = $state("");
  let deletionError = $state("");
  let deleting = $state(false);
  let includeSnapshots = $state(false);
  let deleteDialog: HTMLDialogElement;
  let deleteTrigger: HTMLButtonElement;
  async function removeInstance() {
    deleting = true; deletionError = "";
    try { await deleteInstance(instance.id, confirmation); deleteDialog.close(); await launcher.refreshState(); navigation.goTo("instances"); }
    catch (reason) { deletionError = reason instanceof Error ? reason.message : "Deletion did not complete."; }
    finally { deleting = false; }
  }
  import AuroraUpdatePanel from "./AuroraUpdatePanel.svelte";
  import PerformanceSettings from "./PerformanceSettings.svelte";
  import InstanceReadinessPanel from "./InstanceReadinessPanel.svelte";
  import IconPicker from "./IconPicker.svelte";
  import { instanceIcons } from "./instanceIcons.svelte";
  import { onMount } from "svelte";
  onMount(() => { void instanceIcons.ensureLoaded(); });

  let { instance }: { instance: InstanceSummary } = $props();
  const instanceId = $derived(instance.id);

  // The draft lives in the store keyed by instance id: seeding is idempotent
  // and unsaved edits survive tab switches, other instances, and leaving the
  // workspace. Dirty state is derived, never tracked.
  $effect(() => {
    const id = instanceId;
    untrack(() => launcher.openDetail(id));
  });

  const draft = $derived(launcher.draftFor(instance.id));
  const dirty = $derived(
    draft ? draftIsDirty(draft, instance.configuration) : false,
  );
  const needsInstall = $derived(configurationRequiresInstall(instance));

  function onDraftChange(): void {
    if (draft && draft.loader.kind !== "vanilla" && draft.minecraftVersion.trim() !== "") {
      void launcher.loadLoaderVersions(draft.minecraftVersion, draft.loader.kind);
    }
  }

  function loaderVersionValue(configuration: InstanceConfiguration): string {
    return "policy" in configuration.loader && configuration.loader.policy.type === "pinned"
      ? (configuration.loader.policy.version ?? "")
      : "";
  }

  function setLoaderPolicy(event: Event): void {
    if (!draft || !("policy" in draft.loader)) return;
    const value = (event.currentTarget as HTMLSelectElement).value;
    draft.loader.policy =
      value === "" ? { type: "automatic" } : { type: "pinned", version: value };
    onDraftChange();
  }

  function javaContext(): string | null {
    const status = launcher.runtimeStatus;
    if (launcher.runtimeBusy && status?.instanceId === instance.id) {
      return launcher.runtimeProgress
        ? runtimeProgressLabel(launcher.runtimeProgress)
        : null;
    }
    if (!status || status.instanceId !== instance.id) return null;
    const base = `${status.component} · Java ${status.requiredMajorVersion}`;
    if (status.status === "ready") {
      return (
        base +
        (status.runtimeVersion ? ` · ${status.runtimeVersion}` : "") +
        (status.reused === true ? " · reused verified runtime" : "")
      );
    }
    if (status.status === "damaged") {
      return status.problems[0] ?? "The managed runtime failed validation.";
    }
    return `${base} — manage it from the Overview tab.`;
  }
</script>

<!--
  The workspace Settings tab: the instance's desired-configuration editor.
  Edits work on an explicit draft of the whole configuration; Save persists
  it atomically, and install-affecting changes are applied through a
  deliberate install step — never by a dropdown changing content. These
  settings belong to this instance only; launcher-wide preferences live in
  the sidebar's Settings destination.
-->
<div class="instance-settings-composition">
<div class="settings-main">
<AuroraUpdatePanel {instance} />
<section class="group" aria-labelledby="instance-settings-title">
  <div class="group-heading">
    <div>
      <h3 class="group-title" id="instance-settings-title">Settings</h3>
    </div>
    <!-- Rename lives once, in the workspace header. -->
    {#if dirty}<div class="group-row-actions"><span class="status-badge status-warning">Unsaved changes</span></div>{/if}
  </div>

  <!-- The icon saves immediately and never waits for the configuration draft. -->
  <div class="group-row icon-field">
    <div class="group-row-main">
      <span class="group-row-title">Icon <Tooltip label="About the instance icon">Shown on this instance’s tile and on Home. Saved immediately.</Tooltip></span>
      {#if instanceIcons.error}<span class="group-row-detail icon-error" role="alert">{instanceIcons.error}</span>{/if}
    </div>
    <IconPicker value={instanceIcons.iconFor(instance.id)} disabled={instanceIcons.busy === instance.id}
      onselect={(icon) => void instanceIcons.set(instance.id, icon)} />
  </div>

  {#if !draft}
    <div class="group-row group-row-loading">
      <span class="spinner" aria-hidden="true"></span>
      <span class="group-row-detail">Loading configuration…</span>
    </div>
  {:else}
    <form
      class="group-form"
      onsubmit={(event) => {
        event.preventDefault();
        void launcher.runSaveConfiguration(instance.id);
      }}
    >
      <div class="settings-grid">
      <section class="settings-section"><h4 class="detail-section-title">General</h4>
      <div class="field-grid">
        <label class="field">
          <span class="field-label">Minecraft version</span>
          <select
            bind:value={draft.minecraftVersion}
            onchange={onDraftChange}
            onfocus={() => launcher.loadMinecraftVersions(includeSnapshots)}
          >
            {#if launcher.minecraftVersions === null}
              <option value={draft.minecraftVersion}>{draft.minecraftVersion}</option>
            {:else}
              {#if !launcher.minecraftVersions.some(version => version.id === draft.minecraftVersion)}<option value={draft.minecraftVersion}>{draft.minecraftVersion} (saved)</option>{/if}
              {#each launcher.minecraftVersions as version (version.id)}
                <option value={version.id}>{version.id}</option>
              {/each}
            {/if}
          </select>
          <span class="field-hint">Changing this requires installing new content.</span>
        </label>
        <label class="check-field"><input type="checkbox" bind:checked={includeSnapshots} onchange={() => launcher.loadMinecraftVersions(includeSnapshots)} /><span>Include snapshots</span></label>
        <label class="field">
          <span class="field-label">Mod loader</span>
          <select value={draft.loader.kind} onchange={event => {
            const kind = event.currentTarget.value as "vanilla" | "fabric" | "neoForge" | "forge";
            draft.loader = kind === "vanilla" ? { kind } : { kind, policy: { type: "automatic" } };
            onDraftChange();
          }} disabled={draft.auroraEnabled}>
            {#each launcher.launcherState?.platformCapabilities ?? [] as capability (capability.kind)}
              {#if capability.canInstall}<option value={capability.kind}>{capability.kind === "fabric" ? "Fabric" : capability.kind === "neoForge" ? "NeoForge" : capability.kind === "forge" ? "Forge" : capability.kind === "vanilla" ? "Vanilla" : capability.kind}</option>{/if}
            {/each}
          </select>
          <span class="field-hint">{draft.auroraEnabled ? "The original Aurora association requires Fabric. Manage Aurora content in Mods." : "Changing the platform requires installing new content."}</span>
        </label>
        {#if draft.loader.kind !== "vanilla"}
        <label class="field">
          <span class="field-label">{draft.loader.kind === "neoForge" ? "NeoForge" : draft.loader.kind === "forge" ? "Forge" : "Fabric Loader"} version</span>
          {#if "policy" in draft.loader && draft.loader.policy.type === "automatic"}
            <select value="" onchange={setLoaderPolicy}>
              <option value="">{draft.auroraEnabled ? "Aurora release version" : "Automatic compatible version"}</option>
              {#if loaderVersionValue(draft) && !(launcher.loaderVersions ?? []).some(loader => loader.version === loaderVersionValue(draft))}<option value={loaderVersionValue(draft)}>{loaderVersionValue(draft)} (saved)</option>{/if}
              {#each launcher.loaderVersions ?? [] as loader (loader.version)}
                <option value={loader.version}>
                  {loader.version}{loader.stable ? "" : " (unstable)"}
                </option>
              {/each}
            </select>
            <span class="field-hint">
              {draft.auroraEnabled ? "Uses the exact loader version required by the Aurora release." : "Resolves a compatible loader when installed; the installed version stays pinned until then."}
            </span>
          {:else}
            <select value={loaderVersionValue(draft)} onchange={setLoaderPolicy}>
              <option value="">{draft.auroraEnabled ? "Aurora release version" : "Automatic compatible version"}</option>
              {#if loaderVersionValue(draft) && !(launcher.loaderVersions ?? []).some(loader => loader.version === loaderVersionValue(draft))}<option value={loaderVersionValue(draft)}>{loaderVersionValue(draft)} (saved)</option>{/if}
              {#each launcher.loaderVersions ?? [] as loader (loader.version)}
                <option value={loader.version}>
                  {loader.version}{loader.stable ? "" : " (unstable)"}
                </option>
              {/each}
            </select>
          {/if}
        </label>
        {/if}
      </div>
      </section><section class="settings-section">
      <PerformanceSettings {instance} {draft} busy={launcher.detailBusy !== null} onModeChange={mode => { draft.performanceMode = mode; }} onMemoryChange={mib => { draft.memoryMib = mib; }} />

      </section><section class="settings-section"><h4 class="detail-section-title">Java</h4>
      <div class="field-grid">
        <div class="field">
          <span class="field-label">Runtime</span>
          <span class="field-value">Automatic — managed by Aurora</span>
          <span class="field-hint">
            {javaContext() ??
              "The required Java version follows the selected Minecraft version."}
          </span>
        </div>
        <label class="field field-span">
          <span class="field-label">Additional JVM arguments</span>
          <input
            type="text"
            aria-label="Additional JVM arguments"
            aria-describedby="custom-jvm-description"
            bind:value={draft.additionalJvmArguments}
            disabled={launcher.detailBusy !== null}
            oninput={onDraftChange}
            placeholder='e.g. -Dexample=value "-Dlabel=two words"'
          />
          <span class="field-hint" id="custom-jvm-description">
            Passed to the Java process as separate arguments. Double quotes group text;
            heap settings (-Xmx, -Xms) and classpath flags are owned by Aurora.
          </span>
        </label>
      </div>

      </section><section class="settings-section"><h4 class="detail-section-title">Display</h4>
      <div class="field-grid">
        <div class="field">
          <span class="field-label">Window size</span>
          <div class="window-row">
            <label class="check-field">
              <input
                type="checkbox"
                checked={draft.window !== null}
                onchange={(event) => {
                  const checked = (event.currentTarget as HTMLInputElement).checked;
                  draft.window = checked ? { width: 854, height: 480 } : null;
                  onDraftChange();
                }}
              />
              <span>Custom size</span>
            </label>
            {#if draft.window}
              <label class="field field-narrow">
                <span class="field-label">Width</span>
                <input
                  type="number"
                  min="100"
                  max="7680"
                  bind:value={draft.window.width}
                  onchange={onDraftChange}
                  required
                />
              </label>
              <label class="field field-narrow">
                <span class="field-label">Height</span>
                <input
                  type="number"
                  min="100"
                  max="7680"
                  bind:value={draft.window.height}
                  onchange={onDraftChange}
                  required
                />
              </label>
            {/if}
          </div>
        </div>
      </div>

      </section></div>
      {#if launcher.loaderVersionsError}<p class="inline-message inline-message-error" role="alert">{launcher.loaderVersionsError.message}</p>{/if}
      {#if launcher.minecraftVersionsError}<p class="inline-message inline-message-error" role="alert">{launcher.minecraftVersionsError.message}</p>{/if}
      {#if launcher.detailError}
        <p class="inline-message inline-message-error group-row" role="alert">
          {launcher.detailError.message}
        </p>
      {/if}

      <div class="form-actions">
        <button
          type="submit"
          class="btn btn-primary"
          disabled={launcher.detailBusy !== null || !dirty}
        >
          {launcher.detailBusy === instance.id ? "Saving…" : "Save changes"}
        </button>
        {#if dirty}
          <button
            type="button"
            class="btn btn-quiet"
            onclick={() => launcher.discardDraft(instance.id)}
            disabled={launcher.detailBusy !== null}
          >
            Discard
          </button>
        {/if}
        {#if needsInstall}
          <button
            type="button"
            class="btn"
            onclick={() => void launcher.runInstallConfiguration(instance.id)}
            disabled={launcher.detailInstallBusy !== null || dirty}
          >
            {launcher.detailInstallBusy === instance.id ? "Installing…" : "Install new configuration"}
          </button>
        {/if}
        {#if launcher.detailInstallBusy === instance.id && launcher.createProgress}
          <span class="group-row-detail" role="status">{instanceProgressLabel(launcher.createProgress)}</span>
        {/if}
      </div>
    </form>
  {/if}
</section>

</div>
<aside class="settings-tools">
  <div class="readiness-tool"><InstanceReadinessPanel {instance} /></div>
  <section class="group maintenance" aria-labelledby="maintenance-title"><div class="group-heading"><h3 class="group-title" id="maintenance-title">Maintenance</h3></div>
    <details class="group-row tool-disclosure"><summary>Reinstall / restore configured content</summary><p class="field-hint">Explicitly reinstalls the saved game configuration and restores its reviewed Aurora Client and Fabric API bootstrap files. Worlds, config and unrelated mods stay untouched. Re-enable disabled bootstrap mods in Mods first; file conflicts require inspection.</p>
      <button class="btn" type="button" disabled={dirty || launcher.detailInstallBusy !== null} onclick={() => launcher.runInstallConfiguration(instance.id)}>Reinstall configured content</button>
    </details>
    <details class="group-row tool-disclosure"><summary>Recovery backup storage</summary>
      <p class="field-hint">Manage older installation backups. The newest backups, the last 30 days, and any uncertain recovery data stay protected. Worlds and mods are unaffected.</p>
      <button class="btn" type="button" disabled={backupBusy || instance.state !== "ready" || launcher.detailInstallBusy !== null} onclick={() => void maintainBackups()}>{backupBusy ? "Checking backups…" : "Manage recovery backups"}</button>
      {#if backupTarget === instance.id && backupMessage}<p class="field-hint" role="status">{backupMessage}</p>{/if}
    </details>
  </section>
  <section class="group danger-zone"><div class="group-heading"><h3 class="group-title">Delete instance</h3></div>
    <div class="group-form"><p class="field-hint">Permanently removes this instance's worlds, mods, packs, config and logs. Shared Java, cache and accounts stay available.</p>
    <button bind:this={deleteTrigger} type="button" class="btn btn-danger" disabled={instance.state !== "ready" || launcher.detailInstallBusy !== null} onclick={() => { confirmation = ""; deletionError = ""; deleteDialog.showModal(); }}>Delete instance…</button></div>
  </section>
</aside>
</div>
<dialog bind:this={deleteDialog} class="delete-dialog aurora-dialog" aria-labelledby="delete-instance-title" oncancel={(event) => { if (deleting) event.preventDefault(); }} onclose={() => deleteTrigger?.focus()}>
  <form onsubmit={(event) => { event.preventDefault(); void removeInstance(); }}>
    <h3 id="delete-instance-title">Delete {instance.displayName}?</h3>
    <p>All worlds, mods, packs, configuration and logs in this isolated instance will be permanently deleted. This cannot be undone.</p>
    <label class="field"><span class="field-label">Type {instance.displayName} to confirm</span><input bind:value={confirmation} autocomplete="off" disabled={deleting} /></label>
    {#if deletionError}<p role="alert" class="inline-message inline-message-error">{deletionError}</p>{/if}
    <div class="tool-actions"><button class="btn btn-quiet" type="button" disabled={deleting} onclick={() => deleteDialog.close()}>Cancel</button><button class="btn btn-danger" type="submit" disabled={deleting || confirmation !== instance.displayName}>{deleting ? "Deleting…" : "Delete permanently"}</button></div>
  </form>
</dialog>

<style>
  .instance-settings-composition { display: grid; grid-template-columns: minmax(0, 1.65fr) minmax(280px, 1fr); gap: var(--space-5); align-items: start; }
  .settings-main, .settings-tools { min-width: 0; }
  .settings-tools { display: grid; gap: var(--space-4); align-content: start; }
  .settings-tools > :global(.group) { margin: 0; }
  .icon-error { color: var(--color-error); }
  .readiness-tool { grid-column: 1 / -1; min-width: 0; }
  .settings-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-4); }
  .settings-section { min-width: 0; }
  .settings-section :global(.field-grid) { grid-template-columns: 1fr; }
  .tool-actions { display: flex; flex-wrap: wrap; gap: var(--space-2); align-items: center; }
  .danger-zone { border-color: color-mix(in srgb, var(--color-error) 45%, var(--f-edge)); }
  .maintenance .group-row { padding: var(--space-4) var(--space-5); }
  .tool-disclosure { display: block; }
  .tool-disclosure summary { cursor: pointer; color: var(--color-text); font-weight: 500; padding-block: 2px; }
  .tool-disclosure[open] summary { margin-bottom: var(--space-2); }
  .tool-disclosure .btn { margin-top: var(--space-3); }
  .delete-dialog { max-width: 520px; width: calc(100vw - 64px); }
  .delete-dialog .tool-actions { margin-top: var(--space-4); justify-content: end; }
  @media (max-width: 1250px) { .instance-settings-composition { grid-template-columns: 1fr; } .settings-tools { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-4); } }
  @media (max-width: 850px) { .settings-grid, .settings-tools { grid-template-columns: 1fr; } }
  .detail-section-title {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--color-text);
    margin: var(--space-5) 0 var(--space-3);
  }

  .group-form .detail-section-title:first-of-type {
    margin-top: var(--space-2);
  }

  .field-narrow {
    max-width: 7rem;
  }

  .field-hint {
    display: block;
    margin-top: var(--space-1);
    font-size: var(--text-metadata);
    color: var(--color-text-muted);
  }

  .field-value {
    display: block;
    padding: var(--space-2) 0;
    font-size: var(--text-body);
    color: var(--color-text);
  }

  .check-field {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--color-text-secondary);
    font-size: var(--text-body);
    cursor: pointer;
  }

  .window-row {
    display: flex;
    align-items: end;
    gap: var(--space-4);
    flex-wrap: wrap;
  }
</style>
