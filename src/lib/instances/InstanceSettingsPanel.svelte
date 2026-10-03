<script lang="ts">
  import { launcher } from "$lib/launcher/store.svelte";
  import { configurationRequiresInstall, draftIsDirty } from "$lib/launcher/instanceStatus";
  import { deleteInstance, type InstanceConfiguration, type InstanceSummary } from "$lib/backend";
  import { navigation } from "$lib/launcher/navigation.svelte";
  let confirmation = $state("");
  let deletionError = $state("");
  let deleting = $state(false);
  let deleteDialog: HTMLDialogElement;
  let deleteTrigger: HTMLButtonElement;
  async function removeInstance() {
    deleting = true; deletionError = "";
    try { await deleteInstance(instance.id, confirmation); deleteDialog.close(); await launcher.refreshState(); navigation.goTo("instances"); }
    catch (reason) { deletionError = reason instanceof Error ? reason.message : "Deletion did not complete."; }
    finally { deleting = false; }
  }
  import AuroraTransitionPanel from "./AuroraTransitionPanel.svelte";
  import AuroraUpdatePanel from "./AuroraUpdatePanel.svelte";

  let { instance }: { instance: InstanceSummary } = $props();

  // The draft lives in the store keyed by instance id: seeding is idempotent
  // and unsaved edits survive tab switches, other instances, and leaving the
  // workspace. Dirty state is derived, never tracked.
  $effect(() => {
    launcher.openDetail(instance.id);
  });

  const draft = $derived(launcher.draftFor(instance.id));
  const dirty = $derived(
    draft ? draftIsDirty(draft, instance.configuration) : false,
  );
  const needsInstall = $derived(configurationRequiresInstall(instance));

  function onDraftChange(): void {
    if (draft && draft.loader.kind !== "vanilla" && draft.minecraftVersion.trim() !== "") {
      void launcher.loadLoaderVersions(draft.minecraftVersion);
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

  function onLoaderVersionChange(event: Event): void {
    if (!draft || !("policy" in draft.loader)) return;
    const value = (event.currentTarget as HTMLSelectElement).value;
    draft.loader.policy = { type: "pinned", version: value };
    onDraftChange();
  }

  function javaContext(): string | null {
    const status = launcher.runtimeStatus;
    if (launcher.runtimeBusy && status?.instanceId === instance.id) {
      return launcher.runtimeProgress
        ? `${launcher.runtimeProgress.phase} ${launcher.runtimeProgress.completedItems}/${launcher.runtimeProgress.totalItems}`
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
<div class="settings-main"><AuroraTransitionPanel {instance} />
<AuroraUpdatePanel {instance} />
<section class="group" aria-labelledby="instance-settings-title">
  <div class="group-heading">
    <div>
      <h3 class="group-title" id="instance-settings-title">Settings</h3>
      <p class="group-subtitle">
        The saved configuration this instance launches with.
      </p>
    </div>
    <div class="group-row-actions">
      {#if dirty}<span class="status-badge status-warning">Unsaved changes</span>{/if}
      <button
        type="button"
        class="btn btn-quiet"
        onclick={() => {
          launcher.renaming = { id: instance.id, name: instance.displayName };
        }}
        disabled={launcher.detailBusy !== null}
      >
        Rename
      </button>
    </div>
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
      {#if launcher.renaming?.id === instance.id}
        <div class="field-grid">
          <label class="field">
            <span class="field-label">New name</span>
            <input
              type="text"
              bind:value={launcher.renaming.name}
              required
              maxlength="80"
            />
          </label>
          <div class="form-actions">
            <button
              type="button"
              class="btn"
              onclick={() => void launcher.runRename()}
              disabled={launcher.detailBusy !== null}
            >
              Save name
            </button>
            <button
              type="button"
              class="btn btn-quiet"
              onclick={() => (launcher.renaming = null)}
              disabled={launcher.detailBusy !== null}
            >
              Cancel
            </button>
          </div>
        </div>
      {/if}

      <div class="settings-grid">
      <section class="settings-section"><h4 class="detail-section-title">General</h4>
      <div class="field-grid">
        <label class="field">
          <span class="field-label">Minecraft version</span>
          <select
            bind:value={draft.minecraftVersion}
            onchange={onDraftChange}
            onfocus={() => launcher.loadMinecraftVersions(false)}
            onclick={() => launcher.loadMinecraftVersions(false)}
          >
            {#if launcher.minecraftVersions === null}
              <option value={draft.minecraftVersion}>{draft.minecraftVersion}</option>
            {:else}
              {#each launcher.minecraftVersions as version (version.id)}
                <option value={version.id}>{version.id}</option>
              {/each}
            {/if}
          </select>
          <span class="field-hint">Changing this requires installing new content.</span>
        </label>
        <label class="field">
          <span class="field-label">Mod loader</span>
          <select value={draft.loader.kind} disabled>
            {#each launcher.launcherState?.platformCapabilities ?? [] as capability (capability.kind)}
              {#if capability.canInstall}<option value={capability.kind}>{capability.kind === "fabric" ? "Fabric" : capability.kind === "neoForge" ? "NeoForge" : capability.kind}</option>{/if}
            {/each}
          </select>
        </label>
        {#if draft.loader.kind !== "vanilla"}
        <label class="field">
          <span class="field-label">Fabric Loader version</span>
          {#if "policy" in draft.loader && draft.loader.policy.type === "automatic"}
            <select value="" onchange={setLoaderPolicy}>
              <option value="">Release version</option>
              {#each launcher.loaderVersions ?? [] as loader (loader.version)}
                <option value={loader.version}>
                  {loader.version}{loader.stable ? "" : " (unstable)"}
                </option>
              {/each}
            </select>
            <span class="field-hint">
              {draft.auroraEnabled ? "Uses the exact loader version required by the Aurora release." : "Resolves the newest stable compatible Fabric Loader when installed."}
            </span>
          {:else}
            <select value={loaderVersionValue(draft)} onchange={onLoaderVersionChange}>
              <option value="">Release version</option>
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
      </section><section class="settings-section"><h4 class="detail-section-title">Performance</h4>
      <div class="field-grid">
        <label class="field">
          <span class="field-label">Memory (MB)</span>
          <input
            type="number"
            min="512"
            max="32768"
            step="256"
            bind:value={draft.memoryMib}
            onchange={onDraftChange}
            required
          />
          <span class="field-hint">
            The memory allocated to the game, between 512 and 32768 MB.
          </span>
        </label>
      </div>

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
            bind:value={draft.additionalJvmArguments}
            oninput={onDraftChange}
            placeholder='e.g. -Dexample=value "-Dlabel=two words"'
          />
          <span class="field-hint">
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
          <span class="group-row-detail">
            {launcher.createProgress.phase}
            {#if launcher.createProgress.game}
              · {launcher.createProgress.game.completedItems}/{launcher.createProgress.game.totalItems}
            {/if}
          </span>
        {/if}
      </div>
    </form>
  {/if}

  <p class="group-footer">
    These settings belong to this instance. Launcher-wide preferences — appearance and
    desktop integration — live in the sidebar's Settings destination.
  </p>
</section>

</div>
<aside class="settings-tools">
  <section class="group"><div class="group-heading"><h3 class="group-title">Content</h3></div>
    <div class="group-form"><p>Aurora Client: {instance.auroraContentState ?? (instance.aurora ? "Not detected" : "Not configured")}.</p>
    <p class="field-hint">The original release pin records installation intent. Mods shows the current files and their enabled state.</p>
    <div class="tool-actions"><button class="btn" type="button" onclick={() => navigation.openInstance(instance.id, "mods")}>Manage mods</button>
    <button class="btn" type="button" onclick={() => navigation.openInstance(instance.id, "resourcePacks")}>Resource packs</button>
    <button class="btn" type="button" onclick={() => navigation.openInstance(instance.id, "shaders")}>Shaders</button>
    <button class="btn btn-quiet" type="button" onclick={() => launcher.runOpenModsFolder(instance.id)}>Open mods folder</button></div></div>
  </section>
  <section class="group"><div class="group-heading"><h3 class="group-title">Maintenance</h3></div>
    <div class="group-form tool-actions"><button class="btn" type="button" onclick={() => launcher.runValidate(instance.id)}>Validate instance</button>
    <button class="btn" type="button" onclick={() => launcher.runRuntimeStatus(instance.id)}>Check managed Java</button>
    <button class="btn" type="button" onclick={() => launcher.runEnsureRuntime(instance.id)}>Install / repair Java</button>
    <details><summary>Reinstall / restore configured content</summary><p class="field-hint">Explicitly reinstalls the saved game configuration and restores its reviewed Aurora Client and Fabric API bootstrap files. Worlds, config and unrelated mods stay untouched. Re-enable disabled bootstrap mods in Mods first; file conflicts require inspection.</p>
      <button class="btn" type="button" disabled={dirty || launcher.detailInstallBusy !== null} onclick={() => launcher.runInstallConfiguration(instance.id)}>Reinstall configured content</button>
    </details></div>
  </section>
  <section class="group danger-zone"><div class="group-heading"><h3 class="group-title">Delete instance</h3></div>
    <div class="group-form"><p class="field-hint">Permanently removes this instance's worlds, mods, packs, config and logs. Shared Java, cache and accounts stay available.</p>
    <button bind:this={deleteTrigger} type="button" class="btn btn-danger" disabled={instance.state !== "ready" || launcher.detailInstallBusy !== null} onclick={() => { confirmation = ""; deletionError = ""; deleteDialog.showModal(); }}>Delete instance…</button></div>
  </section>
</aside>
</div>
<dialog bind:this={deleteDialog} class="delete-dialog" oncancel={(event) => { if (deleting) event.preventDefault(); }} onclose={() => deleteTrigger?.focus()}>
  <form onsubmit={(event) => { event.preventDefault(); void removeInstance(); }}>
    <h3>Delete {instance.displayName}?</h3>
    <p>All worlds, mods, packs, configuration and logs in this isolated instance will be permanently deleted. This cannot be undone.</p>
    <label class="field"><span class="field-label">Type {instance.displayName} to confirm</span><input bind:value={confirmation} autocomplete="off" disabled={deleting} /></label>
    {#if deletionError}<p role="alert" class="inline-message inline-message-error">{deletionError}</p>{/if}
    <div class="tool-actions"><button class="btn btn-quiet" type="button" disabled={deleting} onclick={() => deleteDialog.close()}>Cancel</button><button class="btn btn-danger" type="submit" disabled={deleting || confirmation !== instance.displayName}>{deleting ? "Deleting…" : "Delete permanently"}</button></div>
  </form>
</dialog>

<style>
  .instance-settings-composition { display: grid; grid-template-columns: minmax(0, 1.65fr) minmax(280px, 1fr); gap: var(--space-5); align-items: start; }
  .settings-main, .settings-tools { min-width: 0; }
  .settings-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-4); }
  .settings-section { min-width: 0; }
  .settings-section :global(.field-grid) { grid-template-columns: 1fr; }
  .tool-actions { display: flex; flex-wrap: wrap; gap: var(--space-2); align-items: center; }
  .danger-zone { border-color: var(--color-error); }
  .delete-dialog { max-width: 520px; width: calc(100vw - 64px); padding: var(--space-5); border: 1px solid var(--color-border-strong); border-radius: var(--radius-lg); color: var(--color-text); background: var(--color-surface-raised); }
  .delete-dialog::backdrop { background: #0009; }
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
