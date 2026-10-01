<script module lang="ts">
  import { DefaultBrowseCache } from "./modrinthBrowse";
  const browseDefaultPages = new DefaultBrowseCache();
  const failedIconUrls = new Set<string>();
</script>

<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { launcher } from "$lib/launcher/store.svelte";
  import { appendBrowsePage, formatCategoryLabel, TransientNotice } from "./modrinthBrowse";
  import {
    BROWSE_KINDS, BROWSE_SORTS, browseKindLabel, browseKindNoun, browseModrinth, browseModrinthTags,
    browseSortLabel, getModrinthProjectBrowse, installModrinth, previewModrinthInstall, quickInstallModrinth,
    installModrinthPack, previewModrinthPack,
    LauncherBackendError,
    type BrowseKind, type BrowseSort, type ContentType, type ModrinthPreviewResponse,
    type ModrinthProjectDetails, type ModrinthSearchPage, type ProviderCategory,
    type ProviderConflict, type ModrinthPackPreview,
  } from "$lib/backend";
  import InstallConflicts from "./InstallConflicts.svelte";

  let {
    instanceId, instanceName, minecraftVersion, kind, installedProjectIds, dependencyOnlyProjectIds, onInstalled, standalonePackBrowse = false,
  }: {
    instanceId: string;
    instanceName: string;
    minecraftVersion: string;
    kind: ContentType;
    installedProjectIds: string[];
    dependencyOnlyProjectIds: string[];
    onInstalled: (targetInstanceId: string, targetKind: ContentType) => Promise<void>;
    standalonePackBrowse?: boolean;
  } = $props();

  // browseKind is seeded once from the host's kind; hosts swap panels
  // through {#if} so a mounted browser never changes kind. A future host
  // passing a dynamic kind must key its remount explicitly.
  // svelte-ignore state_referenced_locally
  let browseKind = $state<BrowseKind>(standalonePackBrowse ? "modpack" : kind);
  let query = $state("");
  let categories = $state<string[]>([]);
  let sort = $state<BrowseSort>("relevance");
  let page = $state<ModrinthSearchPage | null>(null);
  let project = $state<ModrinthProjectDetails | null>(null);
  let versionId = $state("");
  let preview = $state<ModrinthPreviewResponse | null>(null);
  let packPreview = $state<ModrinthPackPreview | null>(null);
  let selectedOptional = $state<string[]>([]);
  let previewOptional = $state<string[]>([]);
  let packPhase = $state("");
  let stopPackProgress: (() => void) | null = null;
  let busy = $state<"search" | "details" | "preview" | "install" | "tags" | null>(null);
  let error = $state<{ code: string; message: string } | null>(null);
  let nextOffset = $state(0);
  let quickBusyProjectId = $state<string | null>(null);
  let notice = $state("");
  let noticeExiting = $state(false);
  let failedIcons = $state<Record<string, true>>({});
  let blockedProjects = $state<Record<string, { message: string; conflict: ProviderConflict | null }>>({});
  let categoryPickerOpen = $state(false);
  let categoryTrigger: HTMLButtonElement | undefined = $state();
  let allCategories = $state<ProviderCategory[]>([]);
  let categoriesError = $state("");
  let requestSerial = 0;

  // A brief in-memory reuse avoids fetching the same default page on a quick
  // Installed → Browse round trip. Instance and domain are part of the key.
  const defaultPages = browseDefaultPages;
  const notifications = new TransientNotice((message, exiting) => { notice = message; noticeExiting = exiting; });

  const installable = $derived(browseKind !== "modpack");
  const optionalChanged = $derived(selectedOptional.join("\u0000") !== previewOptional.join("\u0000"));
  const packFileCounts = $derived.by(() => {
    const paths = [...(packPreview?.recognized.map((item) => item.path) ?? []), ...(packPreview?.unresolved ?? [])];
    return {
      mods: paths.filter((path) => path.startsWith("mods/")).length,
      resourcePacks: paths.filter((path) => path.startsWith("resourcepacks/")).length,
      shaders: paths.filter((path) => path.startsWith("shaderpacks/")).length,
      other: paths.filter((path) => !/^(mods|resourcepacks|shaderpacks)\//.test(path)).length,
    };
  });
  const availableCategories = $derived.by(() => {
    const scope = browseKind === "mod" ? ["mod", "modpack"] : [browseKindProjectType(browseKind)];
    const seen = new Set<string>();
    return allCategories
      .filter((category) => scope.includes(category.projectType) && !seen.has(category.name) && seen.add(category.name))
      .map((category) => category.name)
      .sort();
  });

  function browseKindProjectType(kind: BrowseKind): string {
    if (kind === "mod") return "mod";
    if (kind === "modpack") return "modpack";
    if (kind === "resourcePack") return "resourcepack";
    return "shader";
  }

  /** Installable kinds only; modpacks never reach the install commands. */
  function installContentType(): ContentType {
    return browseKind === "mod" ? "mod" : browseKind === "resourcePack" ? "resourcePack" : "shaderPack";
  }

  function showError(reason: unknown): void {
    error = reason instanceof LauncherBackendError
      ? { code: reason.code, message: reason.message }
      : { code: "provider_network_error", message: "Modrinth is unavailable. Try again later." };
  }

  onDestroy(() => {
    requestSerial++;
    notifications.dispose();
    stopPackProgress?.();
  });

  async function search(offset = 0, term = query): Promise<void> {
    const serial = ++requestSerial;
    busy = "search";
    error = null;
    if (offset === 0) { page = null; nextOffset = 0; project = null; preview = null; packPreview = null; }
    try {
      const result = await browseModrinth({
        instanceId, provider: "modrinth", contentType: browseKind,
        search: term, categories, sort, offset,
      });
      if (serial !== requestSerial) return;
      page = appendBrowsePage(page, result, offset);
      nextOffset = result.offset + 20;
      if (offset === 0 && !term.trim() && !categories.length && sort === "relevance") {
        defaultPages.put(instanceId, browseKind, minecraftVersion, result);
      }
    } catch (reason) {
      if (serial === requestSerial) showError(reason);
    } finally {
      if (serial === requestSerial) busy = null;
    }
  }

  async function loadCategories(): Promise<void> {
    if (allCategories.length || busy === "tags") return;
    busy = "tags";
    categoriesError = "";
    try {
      allCategories = await browseModrinthTags();
    } catch (reason) {
      categoriesError = reason instanceof Error ? reason.message : "Categories are unavailable right now.";
    } finally { busy = null; }
  }

  function toggleCategory(name: string): void {
    categories = categories.includes(name) ? categories.filter((item) => item !== name) : [...categories, name];
    // Applying or clearing a facet keeps the typed search term and the chosen
    // sort; only the category set changes.
    void search();
  }

  function closeCategoryPicker(): void {
    categoryPickerOpen = false;
    categoryTrigger?.focus();
  }

  function onWindowKeydown(event: KeyboardEvent): void {
    if (categoryPickerOpen && event.key === "Escape") closeCategoryPicker();
  }

  function clearFilters(): void {
    categories = [];
    sort = "relevance";
    query = "";
    void search(0, "");
  }

  function switchKind(next: BrowseKind): void {
    if (standalonePackBrowse && next !== "modpack") return;
    if (browseKind === next) return;
    browseKind = next;
    query = "";
    categories = [];
    sort = "relevance";
    page = null;
    project = null;
    preview = null;
    packPreview = null;
    error = null;
    blockedProjects = {};
    const cached = defaultPages.get(instanceId, browseKind, minecraftVersion);
    if (cached) { page = cached; nextOffset = 20; }
    else void search(0, "");
  }

  onMount(() => {
    let destroyed = false;
    void listen<{ phase: string }>("modpack-progress", (event) => {
      if (busy === "install" && browseKind === "modpack") packPhase = event.payload.phase;
    }).then((stop) => { if (destroyed) stop(); else stopPackProgress = stop; });
    // The host tab mounts browse with a fixed kind; tab changes remount.
    // Initial load only — switches go through switchKind below.
    const cached = defaultPages.get(instanceId, browseKind, minecraftVersion);
    if (cached) {
      page = cached;
      nextOffset = 20;
    } else {
      void search(0, "");
    }
    return () => { destroyed = true; };
  });

  function quickMessage(reason: unknown): string {
    if (!(reason instanceof LauncherBackendError)) return "Modrinth is unavailable. Try again later.";
    switch (reason.code) {
      case "provider_no_compatible_version": return "No compatible version is available for this instance.";
      case "provider_content_collision": return reason.message;
      case "provider_rate_limited": return "Modrinth's rate limit was reached. Try again shortly.";
      case "provider_network_error": return "Modrinth is unavailable. Try again later.";
      case "provider_dependency_unresolved": return "A required dependency has no compatible version.";
      case "provider_dependency_cycle": return "This project's required dependencies contain a cycle.";
      case "provider_integrity_failure": return "The download failed verification. Nothing was installed.";
      case "content_acquisition_failed": return "The download could not be completed. Try again shortly.";
      default: return reason.message;
    }
  }

  async function quickInstall(projectId: string, title: string): Promise<void> {
    if (!installable || quickBusyProjectId || (installedProjectIds.includes(projectId) && !dependencyOnlyProjectIds.includes(projectId))) return;
    const targetInstanceId = instanceId;
    const targetKind = installContentType();
    quickBusyProjectId = projectId;
    try {
      const installed = await quickInstallModrinth(targetInstanceId, targetKind, projectId);
      if (installed.length) {
        await onInstalled(targetInstanceId, targetKind);
        notifications.show(`${title} installed. View it under Installed.`);
      } else {
        notifications.show(`${title} is already installed.`);
      }
    } catch (reason) {
      notifications.show(quickMessage(reason));
      if(reason instanceof LauncherBackendError && reason.code === "provider_content_collision") {
        blockedProjects = { ...blockedProjects, [projectId]: { message: reason.message, conflict: reason.conflict } };
      }
    } finally {
      quickBusyProjectId = null;
    }
  }

  async function openProject(id: string): Promise<void> {
    const serial = ++requestSerial;
    busy = "details";
    error = null;
    preview = null;
    packPreview = null;
    selectedOptional = [];
    try {
      const result = await getModrinthProjectBrowse(instanceId, browseKind, id);
      if (serial !== requestSerial) return;
      project = result;
      versionId = result.defaultVersionId ?? "";
    } catch (reason) {
      if (serial === requestSerial) showError(reason);
    } finally {
      if (serial === requestSerial) busy = null;
    }
  }

  async function inspectInstall(): Promise<void> {
    if (!project || !versionId) return;
    busy = "preview";
    error = null;
    preview = null;
    try {
      preview = await previewModrinthInstall(instanceId, installContentType(), project.projectId, versionId);
    } catch (reason) {
      showError(reason);
    } finally {
      busy = null;
    }
  }

  async function inspectPack(): Promise<void> {
    if (!project || !versionId) return;
    busy = "preview";
    error = null;
    packPreview = null;
    try {
      packPreview = await previewModrinthPack(project.projectId, versionId, selectedOptional);
      previewOptional = [...selectedOptional];
    } catch (reason) { showError(reason); }
    finally { busy = null; }
  }

  async function confirmPack(): Promise<void> {
    if (!packPreview || optionalChanged) return;
    const selected = packPreview;
    busy = "install";
    error = null;
    packPhase = "preparingInstance";
    try {
      const created = await installModrinthPack(selected.projectId, selected.versionId, selectedOptional, selected.fingerprint);
      await launcher.refreshState();
      await launcher.runSelect(created.id);
      packPreview = null;
      project = null;
      notifications.show(`${selected.name} ${selected.packVersion} installed as a new instance.`);
    } catch (reason) { showError(reason); }
    finally { busy = null; packPhase = ""; }
  }

  function togglePackOptional(path: string): void {
    selectedOptional = selectedOptional.includes(path) ? selectedOptional.filter((value) => value !== path) : [...selectedOptional, path];
  }

  const packPhaseLabel = $derived(({
    resolvingPack: "Resolving the exact pack version…",
    downloading: "Downloading and verifying pack files…",
    preparingInstance: "Preparing a new Fabric instance…",
    installingGame: "Installing Minecraft and Fabric…",
    installingOverrides: "Installing pack files and overrides…",
    installingComponents: "Registering managed content…",
    validating: "Validating the completed instance…",
    complete: "Installation complete.",
  } as Record<string, string>)[packPhase] ?? "Installing modpack…");

  async function confirmInstall(): Promise<void> {
    if (!project || !preview) return;
    const targetInstanceId = instanceId;
    const targetKind = installContentType();
    busy = "install";
    error = null;
    try {
      await installModrinth(
        targetInstanceId, targetKind, project.projectId,
        preview.preview.versionId, preview.previewFingerprint,
      );
      await onInstalled(targetInstanceId, targetKind);
      preview = null;
      project = null;
    } catch (reason) {
      showError(reason);
    } finally {
      busy = null;
    }
  }

  const previewItems = $derived(preview?.preview.items ?? []);
  const installCount = $derived(previewItems.filter((item) => !item.alreadyInstalled).length);
</script>

<svelte:window onkeydown={onWindowKeydown} />

<section class="browse" aria-label="Browse Modrinth">
  <div class="browse-heading">
    <div>
      <h3 class="group-title">Browse Modrinth</h3>
      <p class="group-subtitle">{browseKind === "modpack" ? "Choose an exact Fabric pack version. Its declared Minecraft and loader versions determine the new instance." : `Results are filtered for this instance's Minecraft ${minecraftVersion}${browseKind === "mod" ? " and Fabric" : ""}. Compatibility is checked before installation.`}</p>
    </div>
    <span class="source">Source: Modrinth</span>
  </div>

  {#if !standalonePackBrowse}
    <div class="browse-kind-tabs" role="group" aria-label="Browse content type">
      {#each BROWSE_KINDS as option (option)}
        <button type="button" class="btn btn-quiet" aria-pressed={browseKind === option} onclick={() => switchKind(option)}>{browseKindLabel(option)}</button>
      {/each}
    </div>
  {/if}

  <form class="browse-search" onsubmit={(event) => { event.preventDefault(); void search(); }}>
    <label>
      <span class="field-label">Find {browseKindNoun(browseKind)}</span>
      <input type="search" bind:value={query} oninput={(event) => { if (!event.currentTarget.value.trim()) void search(0, ""); }} maxlength="160" placeholder="Search Modrinth" />
    </label>
    <button type="submit" class="btn" disabled={busy !== null}>{busy === "search" ? "Searching…" : "Search"}</button>
  </form>

  <div class="browse-filters">
    <div class="category-filter">
      <button
        type="button"
        class="btn btn-quiet"
        bind:this={categoryTrigger}
        aria-haspopup="true"
        aria-expanded={categoryPickerOpen}
        onclick={() => { categoryPickerOpen = !categoryPickerOpen; if (categoryPickerOpen) void loadCategories(); }}
      >
        Categories{categories.length ? ` (${categories.length})` : ""} ▾
      </button>
      {#if categoryPickerOpen}
        <div class="category-popover" role="dialog" aria-label="Category filter">
          {#if busy === "tags"}<p class="browse-note">Loading categories…</p>{/if}
          {#if categoriesError}<p class="browse-note">{categoriesError}</p>{/if}
          {#if availableCategories.length}
            <div class="category-list">
              {#each availableCategories as category (category)}
                <label class="category-option">
                  <input
                    type="checkbox"
                    checked={categories.includes(category)}
                    onchange={() => toggleCategory(category)}
                  />
                  <span>{formatCategoryLabel(category)}</span>
                </label>
              {/each}
            </div>
          {:else if !busy && !categoriesError}
            <p class="browse-note">No categories are listed for this content type.</p>
          {/if}
        </div>
      {/if}
    </div>
    <label class="sort-control">
      <span class="field-label">Sort</span>
      <select bind:value={sort} onchange={() => void search(0, "")} aria-label="Sort results">
        {#each BROWSE_SORTS as option (option)}
          <option value={option}>{browseSortLabel(option)}</option>
        {/each}
      </select>
    </label>
    {#if categories.length || sort !== "relevance" || query.trim()}
      <button type="button" class="btn btn-quiet" onclick={clearFilters}>Clear filters</button>
    {/if}
    <span class="filter-context">{browseKind === "modpack" ? "Fabric modpacks · new instance" : `Minecraft ${minecraftVersion}${browseKind === "mod" ? " · Fabric" : ""}`}</span>
  </div>

  {#if categories.length}
    <div class="selected-categories" role="group" aria-label="Selected categories">
      {#each categories as category (category)}
        <button type="button" class="category-chip" aria-label={`Remove ${formatCategoryLabel(category)} filter`} onclick={() => toggleCategory(category)}>{formatCategoryLabel(category)} ×</button>
      {/each}
    </div>
  {/if}

  {#if error}
    <p class="inline-message inline-message-error" role="alert">{error.message} <code>{error.code}</code></p>
  {/if}
  {#if busy && busy !== "install"}<p class="browse-status" class:browse-loading={busy === "search" && !page} role="status"><span class="spinner" aria-hidden="true"></span> {busy === "search" ? "Loading Modrinth projects…" : busy === "details" ? "Loading versions…" : "Resolving dependencies…"}</p>{/if}
  {#if busy === "install"}<p class="browse-status" role="status"><span class="spinner" aria-hidden="true"></span> {browseKind === "modpack" ? packPhaseLabel : "Downloading and verifying content…"}</p>{/if}

  {#if project}
    <div class="project">
      <button type="button" class="btn btn-quiet" onclick={() => { project = null; preview = null; packPreview = null; }}>← Results</button>
      <h4>{project.title}</h4>
      {#if blockedProjects[project.projectId]}
        {@const blocker = blockedProjects[project.projectId]}
        {#if blocker.conflict}<InstallConflicts conflicts={[blocker.conflict]} />{:else}<p class="inline-message inline-message-error">{blocker.message}</p>{/if}
      {/if}
      <p>{project.summary}</p>
      <p class="browse-meta">License {project.license} · Modrinth project {project.projectId} · {project.loaders.join(", ") || "No loader listed"}</p>
      {#if browseKind === "shaderPack"}<p class="browse-note">The file can be installed. This instance may need a compatible shader loader before Minecraft can use it.</p>{/if}
      {#if browseKind === "modpack"}<p class="browse-note">This pack will create its own Fabric instance with the Minecraft and loader versions declared by the selected pack. Aurora Client is not added.</p>{/if}
      {#if project.versions.length}
        <label class="version-choice"><span class="field-label">Version</span>
          <select bind:value={versionId} onchange={() => { preview = null; packPreview = null; selectedOptional = []; }}>
            {#each project.versions as version (version.id)}
              <option value={version.id}>{version.versionNumber} · {version.versionType} · {version.name}</option>
            {/each}
          </select>
        </label>
        {#if installable}
          <button type="button" class="btn" disabled={busy !== null || !versionId} onclick={inspectInstall}>Review installation</button>
        {:else}
          <button type="button" class="btn" disabled={busy !== null || !versionId} onclick={inspectPack}>Review pack installation</button>
        {/if}
      {:else}
        <p class="browse-note">No Fabric pack version is available.</p>
      {/if}
    </div>
  {:else if page}
    {#if page.hits.length}
      <div class="browse-list">
        {#each page.hits as hit (hit.projectId)}
          <article class="browse-row">
            <div class="browse-glyph" aria-hidden="true">
              {#if hit.iconUrl && !failedIconUrls.has(hit.iconUrl) && !failedIcons[hit.projectId]}
                <img src={hit.iconUrl} alt="" loading="lazy" onerror={() => { if (hit.iconUrl) failedIconUrls.add(hit.iconUrl); failedIcons = { ...failedIcons, [hit.projectId]: true }; }} />
              {:else}
                {browseKind === "mod" ? "M" : browseKind === "resourcePack" ? "R" : browseKind === "modpack" ? "P" : "S"}
              {/if}
            </div>
            <div class="browse-copy">
              <h4>{hit.title}</h4>
              <p>{hit.summary}</p>
              <span class="browse-meta">By {hit.author} · {hit.downloads.toLocaleString()} downloads</span>
              {#if hit.categories.length}<span class="browse-meta"> · {hit.categories.slice(0, 3).map((category) => formatCategoryLabel(category)).join(" · ")}</span>{/if}
            </div>
            <div class="browse-actions">
              {#if installable}
                <button type="button" class="btn btn-quiet install-action" title={blockedProjects[hit.projectId]?.message ?? (installedProjectIds.includes(hit.projectId) ? `${hit.title} is installed` : `Install newest eligible version of ${hit.title}`)} aria-label={blockedProjects[hit.projectId] ? `${hit.title} installation blocked; review Details` : dependencyOnlyProjectIds.includes(hit.projectId) ? `Keep ${hit.title} installed directly` : installedProjectIds.includes(hit.projectId) ? `${hit.title} is installed` : quickBusyProjectId === hit.projectId ? `Installing ${hit.title}` : `Install newest eligible version of ${hit.title}`} disabled={quickBusyProjectId !== null || !!blockedProjects[hit.projectId] || (installedProjectIds.includes(hit.projectId) && !dependencyOnlyProjectIds.includes(hit.projectId))} onclick={() => quickInstall(hit.projectId, hit.title)}>
                  {#if quickBusyProjectId === hit.projectId}<span class="spinner" aria-hidden="true"></span><span class="action-state">Installing…</span>{:else if installedProjectIds.includes(hit.projectId) && !dependencyOnlyProjectIds.includes(hit.projectId)}<span class="action-state">Installed</span>{:else if blockedProjects[hit.projectId]}<span class="action-state">Blocked</span>{:else if dependencyOnlyProjectIds.includes(hit.projectId)}<span class="action-state">Keep</span>{:else}<span aria-hidden="true">↓</span>{/if}
                </button>
              {:else}
                <span class="browse-only-badge">New instance</span>
              {/if}
              <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={() => openProject(hit.projectId)}>Details</button>
            </div>
          </article>
        {/each}
      </div>
    {:else}
      <p class="browse-note">{page.totalHits > 0 ? "No matching projects on this page." : "No matching projects for this instance."}</p>
    {/if}
    {#if nextOffset < page.totalHits}
      <button type="button" class="btn btn-quiet more" disabled={busy !== null} onclick={() => search(nextOffset)}>Load more</button>
    {/if}
  {/if}

  {#if notice}<div class="browse-notice" class:exiting={noticeExiting} role="status" aria-live="polite">{notice}</div>{/if}

  {#if preview}
    <div class="preview" role="group" aria-label="Installation preview">
      <h4>Install into {instanceName}</h4>
      <InstallConflicts conflicts={preview.conflicts ?? []} />
      <p>{installCount} file{installCount === 1 ? "" : "s"} will be installed. Required dependencies appear below.</p>
      <ul>
        {#each previewItems as item}
          <li><strong>{item.title}</strong>
            {#if item.satisfiedBy}
              {item.satisfiedBy.version} · Already satisfied by {item.satisfiedBy.fileName} ({item.satisfiedBy.ownership}) · requires {item.satisfiedBy.requirement}. Ownership and bytes stay unchanged.
            {:else}
              {item.versionNumber} · {item.fileName}{item.alreadyInstalled ? " · already installed" : " · will install"}
            {/if}
          </li>
        {/each}
      </ul>
      {#each preview.preview.warnings as warning}<p class="browse-note">⚠ {warning}</p>{/each}
      <div class="preview-actions">
        <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={() => preview = null}>Cancel</button>
        <button type="button" class="btn" disabled={busy !== null || installCount === 0 || !!preview.conflicts?.length} onclick={confirmInstall}>Install {installCount} file{installCount === 1 ? "" : "s"}</button>
      </div>
    </div>
  {/if}
  {#if packPreview}
    <div class="preview" role="group" aria-label="Modpack installation preview">
      <h4>{packPreview.name} · {packPreview.packVersion}</h4>
      <p>Minecraft {packPreview.minecraftVersion} · Fabric Loader {packPreview.fabricLoaderVersion} · New independent instance</p>
      <p>{packFileCounts.mods} mods · {packFileCounts.resourcePacks} resource packs · {packFileCounts.shaders} shader packs · {packFileCounts.other} other files · {packPreview.overrides.length} overrides.</p>
      <p>{packPreview.recognized.length} Modrinth-managed components · {packPreview.unresolved.length} external verified files.</p>
      {#if packPreview.recognized.length}
        <details><summary>Modrinth component identities ({packPreview.recognized.length})</summary><ul>{#each packPreview.recognized as item}<li>{item.path} · Modrinth {item.projectId} / {item.versionId}</li>{/each}</ul></details>
      {/if}
      {#if packPreview.unresolved.length}<p class="browse-note">{packPreview.unresolved.length} external files have no Modrinth project identity. Their pack-declared hashes are verified.</p><details><summary>External file paths</summary><ul>{#each packPreview.unresolved as path}<li>{path}</li>{/each}</ul></details>{/if}
      {#if packPreview.optional.length}
        <fieldset class="pack-optional"><legend>Optional client files</legend>
          {#each packPreview.optional as path}<label><input type="checkbox" checked={selectedOptional.includes(path)} onchange={() => togglePackOptional(path)} /> {path}</label>{/each}
        </fieldset>
        {#if optionalChanged}<p class="browse-note">Optional selection changed. Review the updated plan before installing.</p><button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={inspectPack}>Review selection</button>{/if}
      {/if}
      {#if packPreview.excluded.length}<details><summary>Excluded for this client ({packPreview.excluded.length})</summary><ul>{#each packPreview.excluded as path}<li>{path}</li>{/each}</ul></details>{/if}
      {#if packPreview.overrides.length}<details><summary>Pack defaults ({packPreview.overrides.length})</summary><ul>{#each packPreview.overrides as path}<li>{path}</li>{/each}</ul></details>{/if}
      <div class="preview-actions">
        <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={() => packPreview = null}>Cancel</button>
        <button type="button" class="btn" disabled={busy !== null || optionalChanged} onclick={confirmPack}>Install exact version</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .browse { min-width: 0; }
  .browse-heading, .browse-search, .browse-row, .preview-actions, .browse-filters { display: flex; gap: var(--space-3); align-items: center; }
  .browse-heading { justify-content: space-between; margin-bottom: var(--space-3); }
  .browse-heading .group-subtitle { max-width: 50ch; }
  .source, .browse-meta, .filter-context { font-size: var(--text-metadata); color: var(--color-text-muted); }
  .source { white-space: nowrap; }
  .browse-kind-tabs { display: flex; gap: var(--space-2); margin-bottom: var(--space-3); flex-wrap: wrap; }
  .browse-kind-tabs [aria-pressed="true"] { color: var(--color-text); background: var(--color-surface-raised); }
  .browse-search { align-items: end; margin-bottom: var(--space-3); }
  .browse-search label, .version-choice { display: grid; gap: var(--space-1); flex: 1; }
  /* Text-field styling is scoped to the search input specifically: a bare
     `.browse input` selector also matched the category checkboxes and blew
     each one up into a full-width padded box, tearing the picker apart. */
  .browse input[type="search"], .browse select { width: 100%; padding: var(--space-2) var(--space-3); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text); font: inherit; }
  .browse-filters { flex-wrap: wrap; margin-bottom: var(--space-2); }
  .sort-control { display: grid; gap: var(--space-1); width: 168px; }
  .filter-context { margin-left: auto; white-space: nowrap; }
  .category-filter { position: relative; }
  /* Vertical scrolling only: rows wrap internally, so no horizontal
     scrollbar can appear in normal use. */
  .category-popover { position: absolute; z-index: 3; top: calc(100% + var(--space-2)); left: 0; width: 300px; max-width: calc(100vw - 48px); max-height: 320px; overflow-y: auto; overflow-x: hidden; padding: var(--space-2); border: 1px solid var(--color-border-strong); border-radius: var(--radius-md); background: var(--color-surface-raised); box-shadow: var(--shadow-group); }
  .category-list { display: flex; flex-direction: column; gap: 2px; }
  /* One coherent row per category: the checkbox owns a fixed footprint and
     the label takes the remaining width, so a pair can never be split. */
  .category-option { display: flex; align-items: center; gap: var(--space-3); min-height: 34px; padding: var(--space-1) var(--space-2); border-radius: var(--radius-sm); font-size: var(--text-body); color: var(--color-text-secondary); cursor: pointer; }
  .category-option:hover { background: var(--color-surface-hover); color: var(--color-text); }
  .category-option:has(input:checked) { background: var(--color-accent-soft); color: var(--color-accent); }
  .category-option input[type="checkbox"] { width: 18px; height: 18px; flex: none; margin: 0; }
  .category-option span { flex: 1; min-width: 0; line-height: 1.3; overflow-wrap: anywhere; }
  .selected-categories { display: flex; flex-wrap: wrap; gap: var(--space-2); margin: 0 0 var(--space-3); }
  .category-chip { padding: 2px var(--space-2); border: none; border-radius: var(--radius-sm); background: var(--color-accent-soft); color: var(--color-accent); font-size: var(--text-metadata); cursor: pointer; }
  .browse-status { display: flex; align-items: center; gap: var(--space-2); }
  .browse-loading { min-height: 96px; }
  .browse-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-2); }
  .browse-row { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3) var(--space-4); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-md); background: var(--f-panel); }
  .browse-glyph { display: grid; place-items: center; width: 34px; height: 34px; flex: none; overflow: hidden; border-radius: var(--radius-sm); background: var(--color-surface-raised); color: var(--color-text-secondary); font-weight: 700; }
  .browse-glyph img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .browse-actions { display: flex; align-items: center; gap: var(--space-2); flex: none; }
  .install-action { display: flex; align-items: center; justify-content: center; gap: var(--space-1); min-width: 34px; min-height: 32px; font-size: 19px; line-height: 1; }
  .action-state { font-size: var(--text-metadata); }
  .browse-only-badge { padding: 4px var(--space-2); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text-muted); font-size: var(--text-metadata); white-space: nowrap; }
  .browse-notice { position: fixed; z-index: 20; right: var(--space-4); bottom: var(--space-4); max-width: min(360px, calc(100vw - 32px)); padding: var(--space-3) var(--space-4); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-md); background: var(--color-surface-raised); box-shadow: var(--shadow-group); color: var(--color-text); font-size: var(--text-secondary); opacity: 1; transition: opacity 300ms ease-out; }
  .browse-notice.exiting { opacity: 0; }
  .browse-copy { min-width: 0; flex: 1; }
  .browse-copy h4, .project h4, .preview h4 { margin: 0; }
  .browse-copy p, .project p { margin: 2px 0; font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .browse-note { color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .project, .preview { display: grid; gap: var(--space-3); padding: var(--space-4); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-lg); background: var(--f-panel); }
  .project > .btn, .preview-actions .btn { justify-self: start; }
  .preview { margin-top: var(--space-4); }
  .preview ul { margin: 0; padding-left: var(--space-4); }
  .pack-optional { display: grid; gap: var(--space-2); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-md); }
  .pack-optional label { display: block; }
  .preview li { margin: var(--space-1) 0; }
  .more { margin-top: var(--space-3); }
  @media (max-width: 760px) { .browse-heading, .browse-row { align-items: flex-start; } .browse-heading { flex-wrap: wrap; } .filter-context { margin-left: 0; } }
  @media (max-width: 1150px) { .browse-list { grid-template-columns: minmax(0, 1fr); } }
  @media (prefers-reduced-motion: reduce) { .browse-notice { transition: none; } }
</style>
