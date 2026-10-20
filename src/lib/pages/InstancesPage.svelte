<script lang="ts">
  import { launcher } from "$lib/launcher/store.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import {
    instanceProgressLabel,
  } from "$lib/launcher/instanceStatus";
  import { getAuroraCompatibility, type AuroraCompatibility, type InstanceLoader, type InstanceSummary } from "$lib/backend";
  import { creationPlatforms, creationBlocked } from "$lib/instances/configurationChoices";
  import ModrinthBrowse from "$lib/instances/ModrinthBrowse.svelte";
  import InstanceIcon from "$lib/instances/InstanceIcon.svelte";
  import IconPicker from "$lib/instances/IconPicker.svelte";
  import { instanceIcons } from "$lib/instances/instanceIcons.svelte";
  import { onMount } from "svelte";
  import Tooltip from "$lib/shell/Tooltip.svelte";

  onMount(() => { void instanceIcons.ensureLoaded(); });
  import { auroraCreationCopy } from '$lib/instances/creationCopy';

  let browsingPacks = $state(false);

  const platforms = $derived(creationPlatforms(launcher.launcherState?.platformCapabilities ?? []));
  const selectedCapability = $derived(platforms.find(capability => capability.kind === launcher.createPlatform));
  let compatibility = $state<AuroraCompatibility | null>(null);
  let compatibilityError = $state("");
  const platformLabel = (kind: string): string =>
    kind === "vanilla"
      ? "Vanilla"
      : kind === "neoForge"
        ? "NeoForge"
        : kind === "forge"
          ? "Forge"
          : "Fabric";
  const loader = $derived<InstanceLoader>(
    launcher.createPlatform === "vanilla"
      ? {kind: "vanilla"}
      : launcher.createPlatform === "neoForge"
        ? {kind: "neoForge", policy: launcher.createLoaderPolicy}
        : launcher.createPlatform === "forge"
          ? {kind: "forge", policy: launcher.createLoaderPolicy}
          : {kind: "fabric", policy: launcher.createLoaderPolicy},
  );
  $effect(() => {
    const version = launcher.createMinecraftVersion;
    const selection = $state.snapshot(loader);
    let active = true;
    compatibility = null;
    launcher.createAuroraEnabled = false;
    compatibilityError = "";
    void getAuroraCompatibility(version, selection).then(result => { if (active) { compatibility = result; } }).catch(error => { if (active) compatibilityError = error.message; });
    return () => { active = false; };
  });
  $effect(() => { launcher.createAuroraEnabled = compatibility?.available === true && launcher.createAuroraPreference !== false; });
  const instances = $derived(launcher.launcherState?.instances ?? []);
  const selectedId = $derived(launcher.launcherState?.config.selectedInstanceId ?? null);
  // Existing instances are the primary content of this page once any exist.
  // While state is loading (or failed to load) the count is unknown, so the
  // existing-first arrangement stays in place and never flashes to the
  // empty-state layout and back.
  const existingFirst = $derived(instances.length > 0 || launcher.launcherState === null);

  // The create form defaults its Minecraft version to the newest release
  // the Aurora release source supports, so a fresh instance is compatible
  // by construction.
  $effect(() => {
    if (launcher.createMinecraftVersion === "" && launcher.releases.length > 0) {
      launcher.createMinecraftVersion = launcher.releases[0].minecraftVersion;
    }
  });

  function statusDetail(instance: InstanceSummary): string | null {
    if (instance.state === "installing" && launcher.createBusy) return instanceProgressLabel(launcher.createProgress);
    if (instance.recovery) return instance.recovery.message;
    const validation = launcher.instanceValidations[instance.id];
    if (instance.state === "installing") {
      return instance.pack
          ? "This modpack did not finish installing and is unavailable. Review its files before recovery."
          : "This instance did not finish installing — retry below.";
    }
    if (validation?.status === "damaged") {
      return validation.problems[0]
        ? `${validation.problems[0].component}: ${validation.problems[0].reason}`
        : "Deep validation found problems.";
    }
    if (validation?.status === "stale") {
      return "The saved configuration differs from the installed content — open the instance and install the new configuration.";
    }
    return null;
  }
</script>

<!--
  Instance identity and creation. Cards open the instance workspace;
  only interrupted installations need an inline recovery action.
  The full configuration editor and Java lifecycle live in each instance's
  workspace, so this page never duplicates the Settings editor.
-->
<div class="page instances-page f-pilot">
  {#if launcher.launcherState === null && !launcher.stateError}
    <p role="status">Checking saved installation state and recovering interrupted installations…</p>
  {/if}
  <header class="page-header">
    <div>
      <h2 class="page-title">Instances</h2>
    </div>
  </header>

  {#snippet createSection()}
  <section class="group" aria-labelledby="create-title">
    <div class="group-heading">
      <div>
        <h3 class="group-title" id="create-title">Create instance
          {#if launcher.releases.some((release) => release.source === "development-fixture")}
            <Tooltip label="Development release source" tone="warning">Development releases are available in this debug build. Serve <code>src-tauri/development</code> on 127.0.0.1:8765 to install those fixtures.</Tooltip>
          {/if}
        </h3>
      </div>
      {#if launcher.createBusy}
        <span class="status-badge status-working">
          Installing
        </span>
      {/if}
    </div>

    <form
      class="group-form"
      onsubmit={(event) => {
        event.preventDefault();
        void launcher.runCreateInstance();
      }}
    >
      <div class="field-grid">
        <div class="field field-span name-with-icon">
          <label class="field-label" for="create-instance-name">Name</label>
          <div class="name-row">
            <IconPicker value={instanceIcons.pendingCreateIcon} disabled={launcher.createBusy}
              onselect={(icon) => (instanceIcons.pendingCreateIcon = icon)} />
            <input
              id="create-instance-name"
              type="text"
              bind:value={launcher.createDisplayName}
              placeholder="e.g. My Aurora Setup"
              required
              maxlength="80"
            />
          </div>
        </div>
        <label class="field">
          <span class="field-label">Minecraft version</span>
          <select
            bind:value={launcher.createMinecraftVersion}
            onfocus={() => launcher.loadMinecraftVersions(launcher.createIncludeSnapshots)}
            onclick={() => launcher.loadMinecraftVersions(launcher.createIncludeSnapshots)}
            required
          >
            {#if launcher.minecraftVersions === null}
              <option value={launcher.createMinecraftVersion}>
                {launcher.createMinecraftVersion || "Loading versions…"}
              </option>
            {:else}
              {#each launcher.minecraftVersions as version (version.id)}
                <option value={version.id}>
                  {version.id}{version.versionType === "snapshot" ? " (snapshot)" : ""}
                </option>
              {/each}
            {/if}
          </select>
        </label>
        <label class="field">
          <span class="field-label">Minecraft platform</span>
          <select bind:value={launcher.createPlatform}>
            {#each platforms as capability (capability.kind)}
              <option value={capability.kind}>{platformLabel(capability.kind)}</option>
            {/each}
          </select>
        </label>
        {#if launcher.createPlatform !== "vanilla"}
        <label class="field">
          <span class="field-label">{platformLabel(launcher.createPlatform)} version</span>
          <select
            value={launcher.createLoaderPolicy.type === "automatic" ? "" : "pinned"}
            onchange={(event) => {
              const value = (event.currentTarget as HTMLSelectElement).value;
              launcher.createLoaderPolicy =
                value === ""
                  ? { type: "automatic" }
                  : {
                      type: "pinned",
                      version:
                        (launcher.loaderVersions?.find((loader) => loader.stable)?.version ??
                        ""),
                    };
              if (value === "pinned" && launcher.createMinecraftVersion) {
                void launcher.loadLoaderVersions(launcher.createMinecraftVersion);
              }
            }}
          >
              <option value="">Automatic (stable)</option>
              <option value="pinned">{platformLabel(launcher.createPlatform)} — choose version</option>
          </select>
          {#if launcher.createLoaderPolicy.type === "pinned"}
            <select
              class="field-nested"
              bind:value={launcher.createLoaderPolicy.version}
              required
            >
              {#if launcher.loaderVersions === null}
                <option value="">Loading loader versions…</option>
              {:else}
                {#each launcher.loaderVersions as loader (loader.version)}
                  <option value={loader.version}>
                    {loader.version}{loader.stable ? "" : " (unstable)"}
                  </option>
                {/each}
              {/if}
            </select>
          {/if}
        </label>
        {/if}
      </div>
      <div class="check-with-info"><label class="check-field">
        <input type="checkbox" bind:checked={launcher.createAuroraEnabled} onchange={() => { launcher.createAuroraPreference = launcher.createAuroraEnabled; }} disabled={!selectedCapability?.auroraSupported || !compatibility?.available || launcher.createBusy} />
        <span>Aurora Client</span>
      </label>
      <Tooltip label="Aurora Client compatibility">{auroraCreationCopy(compatibility)} {launcher.createAuroraEnabled && compatibility?.loaderVersion ? `Required Fabric Loader: ${compatibility.loaderVersion}.` : ""}</Tooltip></div>
      {#if compatibilityError}<p class="inline-message inline-message-error" role="alert">{compatibilityError}</p>{/if}
      <label class="check-field">
        <input
          type="checkbox"
          bind:checked={launcher.createIncludeSnapshots}
          onchange={() => launcher.loadMinecraftVersions(launcher.createIncludeSnapshots)}
        />
        <span>Show snapshot versions</span>
      </label>
      <div class="form-actions">
        <button
          type="submit"
          class="btn btn-primary"
          disabled={!selectedCapability || (compatibility === null || creationBlocked(launcher.createAuroraEnabled, compatibility)) || launcher.createBusy ||
            launcher.createDisplayName.trim() === "" ||
            launcher.createMinecraftVersion === ""}
        >
          {launcher.createBusy ? "Creating…" : "Create instance"}
        </button>
        {#if launcher.createBusy}
          <span class="group-row-detail" role="status">{instanceProgressLabel(launcher.createProgress)}</span>
        {/if}
      </div>
    </form>

    {#if launcher.createError}
      <p class="inline-message inline-message-error group-row" role="alert">
        {launcher.createError.message}
      </p>
    {/if}
    {#if launcher.minecraftVersionsError}
      <p class="inline-message inline-message-error group-row" role="alert">
        {launcher.minecraftVersionsError.message}
      </p>
    {/if}
    {#if launcher.loaderVersionsError}
      <p class="inline-message inline-message-error group-row" role="alert">
        {launcher.loaderVersionsError.message}
      </p>
    {/if}

  </section>
  {/snippet}

  {#snippet instancesSection()}
  <section class="instances-library" aria-labelledby="list-title" aria-live="polite">
    <header class="library-heading">
      <h3 class="group-title" id="list-title">Your instances</h3>
    </header>

    {#if launcher.stateError}
      <p class="inline-message inline-message-error" role="alert">
        {launcher.stateError.message}
      </p>
    {:else if launcher.launcherState === null}
      <p class="library-loading" role="status"><span class="spinner" aria-hidden="true"></span> Loading instances…</p>
    {:else}
      <ul class="instance-tiles">
      {#each instances as instance (instance.id)}
        {@const selected = selectedId === instance.id}
        {@const busy = launcher.instanceBusy === instance.id}
        {@const detail = statusDetail(instance)}
        {@const problemTone =
          launcher.instanceValidations[instance.id]?.status === "damaged"
            ? "problem-line-error"
            : "problem-line-warning"}
        <li class="instance-tile" class:selected>
          <div class="tile-top">
            <InstanceIcon icon={instanceIcons.iconFor(instance.id)} size={42} tile />
          </div>
          <!-- The name is the card's primary action; its hit area covers the tile. -->
          <button type="button" class="tile-open" aria-label="Open {instance.displayName}" aria-current={selected ? 'true' : undefined}
            onclick={() => navigation.openInstance(instance.id)} disabled={busy || launcher.createBusy}>
            <span class="tile-name" title={instance.displayName}>{instance.displayName}</span>
          </button>
          <span class="tile-config">Minecraft {instance.minecraftVersion}</span>
          {#if detail}<span class="tile-detail {problemTone}">{detail}</span>{/if}
          {#if instance.state === "installing" && !instance.pack && !launcher.createBusy}
            <div class="tile-actions">
              <button type="button" class="btn" onclick={() => launcher.runRetry(instance.id)} disabled={busy || launcher.createBusy}>Retry install</button>
            </div>
          {/if}
        </li>
      {/each}
      </ul>

      {#if launcher.instanceError}
        <p class="inline-message inline-message-error" role="alert">
          {launcher.instanceError.message}
        </p>
      {/if}
    {/if}

  </section>
  {/snippet}

  <!-- DOM order, reading order and keyboard order all follow the rendered
       branch: the instance list precedes creation whenever instances exist,
       and creation leads only in the intentional zero-instance empty state. -->
  <div class="instances-composition" class:existing-first={existingFirst} class:creation-first={!existingFirst}>
    <section class="group modpack-group" aria-label="Modrinth modpacks">
      <div class="group-heading">
        <div><h3 class="group-title">Modrinth modpacks</h3></div>
        <button type="button" class="btn" aria-expanded={browsingPacks} onclick={() => browsingPacks = !browsingPacks}>{browsingPacks ? "Close Modpacks" : "Browse Modpacks"}</button>
      </div>
      {#if browsingPacks}
        <div class="modpack-browser">
        <ModrinthBrowse instanceId="" instanceName="" minecraftVersion="" kind="mod" installedProjectIds={[]} dependencyOnlyProjectIds={[]} onInstalled={async () => {}} standalonePackBrowse />
        </div>
      {/if}
    </section>
  {#if existingFirst}
      {@render instancesSection()}
      {@render createSection()}
  {:else}
      {@render createSection()}
      <section class="group" aria-labelledby="list-title">
        <div class="group-heading">
          <div>
            <h3 class="group-title" id="list-title">Your instances</h3>
          </div>
        </div>
        <p class="group-footer">No instances yet.</p>
      </section>
  {/if}
  </div>
</div>

<style>
  .modpack-browser { padding: 0 24px 24px; }
  .modpack-group { grid-column: 1 / -1; }
  /* Instance library: one tile per instance. Tiles are the surfaces (no
     enclosing panel), so there is no card-on-card nesting. */
  .instances-library { display: grid; gap: var(--space-4); min-width: 0; align-content: start; }
  .library-heading { display: grid; gap: var(--space-1); }
  .library-loading { display: flex; align-items: center; gap: var(--space-3); margin: 0; color: var(--color-text-secondary); }
  .instance-tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(232px, 1fr)); gap: var(--space-4); margin: 0; padding: 0; list-style: none; }
  .instance-tile { position: relative; isolation: isolate; display: flex; flex-direction: column; gap: 6px; min-width: 0; min-height: 188px; padding: 18px; border: 1px solid var(--f-edge); border-radius: var(--f-radius-panel); background: var(--f-content-panel); box-shadow: var(--f-shadow); transition: border-color var(--f-fast) ease, box-shadow var(--f-fast) ease, transform 160ms var(--f-ease); }
  .instance-tile::before { content: ""; position: absolute; inset: 0; z-index: -1; border-radius: inherit; background: rgb(255 255 255 / 3.5%); opacity: 0; transition: opacity var(--f-fast) ease; pointer-events: none; }
  @media (hover: hover) and (pointer: fine) {
    .instance-tile:has(.tile-open:hover:not(:disabled))::before { opacity: 1; }
    .instance-tile:has(.tile-open:hover:not(:disabled)) { border-color: rgb(255 255 255 / 16%); }
  }
  .instance-tile:has(.tile-open:active:not(:disabled)) { transform: scale(.985); }
  .instance-tile:has(.tile-open:focus-visible) { outline: 2px solid var(--color-accent); outline-offset: 3px; }
  .instance-tile.selected { border-color: var(--color-accent-outline); box-shadow: inset 2px 0 0 var(--color-accent), var(--f-shadow); }
  .tile-top { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-2); margin-bottom: var(--space-3); }
  .tile-open { position: static; padding: 0; border: 0; background: none; color: var(--color-text); font: inherit; text-align: left; cursor: pointer; }
  .tile-open::after { content: ""; position: absolute; inset: 0; border-radius: inherit; }
  .tile-open:focus-visible { outline: none; }
  .tile-open:disabled { cursor: progress; }
  .tile-name { display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; overflow: hidden; font-size: 1.0625rem; font-weight: 600; letter-spacing: -.015em; line-height: 1.3; overflow-wrap: anywhere; }
  .tile-config { color: var(--color-text-secondary); font-size: var(--text-metadata); overflow-wrap: anywhere; }
  .tile-detail { font-size: var(--text-metadata); line-height: 1.45; overflow-wrap: anywhere; }
  .tile-actions { position: relative; z-index: 1; display: flex; flex-wrap: wrap; gap: var(--space-2); margin-top: auto; padding-top: var(--space-3); }
  .tile-actions .btn { padding-block: 6px; }
  @media (prefers-reduced-motion: reduce) { .instance-tile, .instance-tile::before { transition: none; } .instance-tile:has(.tile-open:active:not(:disabled)) { transform: none; } }
  .name-row { display: flex; align-items: center; gap: var(--space-2); }
  .name-row input { flex: 1; min-width: 0; }
  .instances-page { --instance-panel-gap: var(--space-5); max-width: 1500px; width: 100%; margin-inline: auto; display: grid; gap: var(--instance-panel-gap); }
  .instances-page > .page-header { margin: 0; }
  .instances-composition { display: grid; gap: var(--instance-panel-gap); align-items: start; }
  /* The existing-instance collection is the primary region: left column and
     at least as wide as the creation form. Stacking keeps DOM order, so the
     list stays above creation at narrow widths without CSS reordering. */
  .instances-composition.existing-first { grid-template-columns: minmax(480px, 1.15fr) minmax(360px, 0.85fr); }
  /* Zero instances: creation is the primary task, centered as the page's
     single form, with only a compact instances note below it. */
  .instances-composition.creation-first { grid-template-columns: minmax(0, min(100%, 880px)); justify-content: center; }
  /* The page header shares the centered creation column's edge. */
  .instances-page:has(.creation-first) { max-width: 880px; }
  .instances-composition > .group { min-width: 0; margin: 0; }
  .instances-composition :global(.field-grid) { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .group-form { gap: var(--space-4); padding: var(--space-4) var(--space-5) var(--space-5); }
  @media (max-width: 1250px) { .instances-composition.existing-first { grid-template-columns: minmax(0, 1fr); } }
  .field-nested {
    margin-top: var(--space-2);
  }

  .check-field {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--color-text-secondary);
    font-size: var(--text-body);
    cursor: pointer;
  }
  .check-with-info { display: flex; align-items: center; gap: var(--space-1); }

  .problem-line-warning {
    color: var(--color-warning);
  }

  .problem-line-error {
    color: var(--color-error);
  }

</style>
