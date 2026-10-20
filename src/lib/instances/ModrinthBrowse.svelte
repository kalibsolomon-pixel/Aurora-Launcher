<script module lang="ts">
  import { DefaultBrowseCache } from "./modrinthBrowse";
  const browseDefaultPages = new DefaultBrowseCache();
</script>

<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { launcher } from "$lib/launcher/store.svelte";
  import { appendBrowsePage, formatCategoryLabel, TransientNotice } from "./modrinthBrowse";
  import {
    BROWSE_KINDS, BROWSE_SORTS, browseKindLabel, browseKindNoun, browseModrinth, browseModrinthTags,
    browseSortLabel, getModrinthProjectBrowse, installModrinth, previewModrinthInstall, quickInstallModrinth,
    installModrinthPack, previewModrinthPack, previewModrinthPackMerge, installModrinthPackMerge,
    LauncherBackendError,
    type BrowseKind, type BrowseSort, type ContentType, type ModrinthPreviewResponse,
    type ModrinthProjectDetails, type ModrinthSearchPage, type ProviderCategory,
    type ProviderConflict, type ModrinthPackPreview, type ModrinthPackMergePreview,
  } from "$lib/backend";
  import InstallConflicts from "./InstallConflicts.svelte";
  import InstalledArtwork from './InstalledArtwork.svelte';
  import Icon from '$lib/shell/Icon.svelte';

  let {
    instanceId, instanceName, minecraftVersion, loaderLabel = "Fabric", kind, installedProjectIds, dependencyOnlyProjectIds, onInstalled, standalonePackBrowse = false,
  }: {
    instanceId: string;
    instanceName: string;
    minecraftVersion: string;
    loaderLabel?: string;
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
  let resultQuery = $state("");
  let categories = $state<string[]>([]);
  let sort = $state<BrowseSort>("relevance");
  let page = $state<ModrinthSearchPage | null>(null);
  let project = $state<ModrinthProjectDetails | null>(null);
  let versionId = $state("");
  let preview = $state<ModrinthPreviewResponse | null>(null);
  let packPreview = $state<ModrinthPackPreview | null>(null);
  // Inside an instance workspace a pack is added to that instance; only the
  // standalone Instances browser creates a new instance from a pack.
  const packIntoInstance = $derived(!standalonePackBrowse && instanceId !== "");
  let mergePreview = $state<ModrinthPackMergePreview | null>(null);
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
  let blockedProjects = $state<Record<string, { message: string; conflict: ProviderConflict | null }>>({});
  let categoryPickerOpen = $state(false);
  let categoryTrigger: HTMLButtonElement | undefined = $state();
  let searchInput: HTMLInputElement | undefined = $state();
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
      resultQuery = term;
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

  function clearSearch(): void {
    query = "";
    searchInput?.focus();
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
    if (!installable || busy !== null || quickBusyProjectId || (installedProjectIds.includes(projectId) && !dependencyOnlyProjectIds.includes(projectId))) return;
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
    mergePreview = null;
    try {
      if (packIntoInstance) {
        mergePreview = await previewModrinthPackMerge(instanceId, project.projectId, versionId, selectedOptional);
        packPreview = mergePreview.pack;
      } else {
        packPreview = await previewModrinthPack(project.projectId, versionId, selectedOptional);
      }
      previewOptional = [...selectedOptional];
    } catch (reason) { showError(reason); }
    finally { busy = null; }
  }

  async function confirmPack(): Promise<void> {
    if (!packPreview || optionalChanged || busy !== null) return;
    const selected = packPreview;
    busy = "install";
    error = null;
    packPhase = packIntoInstance ? "resolvingPack" : "preparingInstance";
    try {
      if (packIntoInstance) {
        if (!mergePreview || mergePreview.blockers.length) return;
        const result = await installModrinthPackMerge(instanceId, selected.projectId, selected.versionId, selectedOptional, mergePreview.fingerprint);
        packPreview = null;
        mergePreview = null;
        project = null;
        await onInstalled(instanceId, "mod");
        notifications.show(`Added ${result.added} file${result.added === 1 ? "" : "s"} from ${selected.name} to ${instanceName}${result.keptExisting.length ? ` · kept ${result.keptExisting.length} of your existing settings files` : ""}.`);
        return;
      }
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
    if (!project || !preview || busy !== null) return;
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

<section class="browse" class:browse-mods={browseKind === "mod"} class:browse-modpacks={browseKind === "modpack"} class:browse-resource-packs={browseKind === "resourcePack"} aria-label="Browse Modrinth">
  <div class="browse-heading">
    <div>
      <h3 class="group-title">Discover {browseKindLabel(browseKind)}</h3>
      <p class="group-subtitle">{browseKind === "modpack" ? (packIntoInstance ? `Review an exact Fabric pack version to add its mods and settings to ${instanceName}. Your existing files are never overwritten.` : "Review an exact Fabric pack version to create a new independent instance.") : `Install into ${instanceName}. Compatibility and required dependencies are checked before installation.`}</p>
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
    <div class="search-control">
      <label class="field-label" for="browse-search-input">Find {browseKindNoun(browseKind)}</label>
      <span class="search-field">
        <input id="browse-search-input" type="search" bind:this={searchInput} bind:value={query} oninput={(event) => { if (!event.currentTarget.value.trim()) void search(0, ""); }} maxlength="160" placeholder={`Search ${browseKindNoun(browseKind)} on Modrinth`} />
        {#if query}<button type="button" class="clear-search" aria-label="Clear search" onclick={clearSearch}>×</button>{/if}
      </span>
    </div>
    <button type="submit" class="btn" disabled={busy !== null}>{busy === "search" ? "Searching…" : "Search"}</button>
  </form>

  <div class="browse-filters">
    <div class="category-filter">
      <span class="field-label">Categories</span>
      <button
        type="button"
        class="filter-input"
        bind:this={categoryTrigger}
        aria-label={`Categories: ${categories.length ? `${categories.length} selected` : "All categories"}`}
        aria-haspopup="true"
        aria-expanded={categoryPickerOpen}
        onclick={() => { categoryPickerOpen = !categoryPickerOpen; if (categoryPickerOpen) void loadCategories(); }}
      >
        <span>{categories.length ? `${categories.length} selected` : "All categories"}</span>
        <Icon name="chevron" size={16} />
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
      <span class="select-wrap">
      <select class="filter-input" bind:value={sort} onchange={() => void search()} aria-label="Sort results">
        {#each BROWSE_SORTS as option (option)}
          <option value={option}>{browseSortLabel(option)}</option>
        {/each}
      </select>
      <span class="filter-chevron"><Icon name="chevron" size={16} /></span>
      </span>
    </label>
    {#if categories.length || sort !== "relevance" || query.trim()}
      <button type="button" class="btn btn-quiet" onclick={clearFilters}>Clear filters</button>
    {/if}
    <span class="filter-context">{browseKind === "modpack" ? (packIntoInstance ? `Fabric modpacks · into ${instanceName}` : "Fabric modpacks · new instance") : `Minecraft ${minecraftVersion}${browseKind === "mod" ? ` · ${loaderLabel}` : ""}`}</span>
  </div>

  {#if categories.length}
    <div class="selected-categories" role="group" aria-label="Selected categories">
      {#each categories as category (category)}
        <button type="button" class="category-chip" aria-label={`Remove ${formatCategoryLabel(category)} filter`} onclick={() => toggleCategory(category)}>{formatCategoryLabel(category)} ×</button>
      {/each}
    </div>
  {/if}

  {#if error}
    <div class="browse-error" role="alert">
      <p class="inline-message inline-message-error">{error.message} <code>{error.code}</code></p>
      {#if !project}<button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={() => search(page ? nextOffset : 0)}>Retry search</button>{/if}
    </div>
  {/if}
  {#if busy && busy !== "install"}<p class="browse-status" class:browse-loading={busy === "search" && !page} role="status"><span class="spinner" aria-hidden="true"></span> {busy === "search" ? "Loading Modrinth projects…" : busy === "details" ? "Loading versions…" : "Resolving dependencies…"}</p>{/if}
  {#if busy === "install"}<p class="browse-status" role="status"><span class="spinner" aria-hidden="true"></span> {browseKind === "modpack" ? packPhaseLabel : "Downloading and verifying content…"}</p>{/if}
  {#if quickBusyProjectId}<p class="browse-status" role="status"><span class="spinner" aria-hidden="true"></span> Installing {page?.hits.find(hit => hit.projectId === quickBusyProjectId)?.title ?? "content"} into {instanceName}…</p>{/if}

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
      {#if browseKind === "modpack"}<p class="browse-note">{packIntoInstance
        ? `This pack's mods and settings are added to ${instanceName}. Its Minecraft version must match this instance and its Fabric Loader can't be newer; your existing files are never overwritten.`
        : "This pack will create its own Fabric instance with the Minecraft and loader versions declared by the selected pack. Aurora Client is not added."}</p>{/if}
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
        <p class="browse-note">{browseKind === "modpack" ? "No supported Fabric pack version is available." : "No compatible version is available for this instance."}</p>
      {/if}
    </div>
  {:else if page}
    <p class="result-count" role="status">{page.hits.length.toLocaleString()} of {page.totalHits.toLocaleString()} {browseKindNoun(browseKind)}{resultQuery.trim() ? ` matching “${resultQuery.trim()}”` : ""}</p>
    {#if page.hits.length}
      <div class="browse-list">
        {#each page.hits as hit (hit.projectId)}
          <article class="browse-row">
            <div class="browse-glyph" aria-hidden="true">
              <InstalledArtwork projectId={hit.projectId} size={browseKind === "resourcePack" ? 64 : 44} fallback={browseKind === "mod" ? "M" : browseKind === "resourcePack" ? "R" : browseKind === "modpack" ? "P" : "S"} />
            </div>
            <div class="browse-copy">
              <h4>{hit.title}</h4>
              <p>{hit.summary}</p>
              <span class="browse-meta">By {hit.author} · {hit.downloads.toLocaleString()} downloads</span>
              {#if hit.categories.length}<span class="browse-meta"> · {hit.categories.slice(0, 3).map((category) => formatCategoryLabel(category)).join(" · ")}</span>{/if}
            </div>
            <div class="browse-actions">
              {#if installable}
                <button type="button" class="btn install-action" title={blockedProjects[hit.projectId]?.message ?? (installedProjectIds.includes(hit.projectId) ? `${hit.title} is installed` : `Install newest eligible version of ${hit.title}`)} aria-label={blockedProjects[hit.projectId] ? `${hit.title} installation blocked; review Details` : quickBusyProjectId === hit.projectId ? `Installing ${hit.title}` : dependencyOnlyProjectIds.includes(hit.projectId) ? `Keep ${hit.title} installed directly` : installedProjectIds.includes(hit.projectId) ? `${hit.title} is installed` : `Install newest eligible version of ${hit.title}`} disabled={busy !== null || quickBusyProjectId !== null || !!blockedProjects[hit.projectId] || (installedProjectIds.includes(hit.projectId) && !dependencyOnlyProjectIds.includes(hit.projectId))} onclick={() => quickInstall(hit.projectId, hit.title)}>
                  {#if quickBusyProjectId === hit.projectId}<span class="spinner" aria-hidden="true"></span><span class="action-state">Installing…</span>{:else if installedProjectIds.includes(hit.projectId) && !dependencyOnlyProjectIds.includes(hit.projectId)}<span class="action-state">Installed</span>{:else if blockedProjects[hit.projectId]}<span class="action-state">Blocked</span>{:else if dependencyOnlyProjectIds.includes(hit.projectId)}<span class="action-state">Keep</span>{:else}<Icon name="download" size={18} /><span class="action-state">Install</span>{/if}
                </button>
              {:else}
                <span class="browse-only-badge">{packIntoInstance ? "Adds to instance" : "New instance"}</span>
              {/if}
              <button type="button" class="btn" class:btn-quiet={browseKind !== "modpack"} aria-label={browseKind === "modpack" ? `Review ${hit.title} pack` : `Details for ${hit.title}`} disabled={busy !== null || quickBusyProjectId !== null} onclick={() => openProject(hit.projectId)}>{browseKind === "modpack" ? "Review pack" : "Details"}</button>
            </div>
          </article>
        {/each}
      </div>
    {:else}
      <div class="browse-empty" role="status">
        <h4>{page.totalHits > 0 ? "No projects on this page" : query.trim() || categories.length ? "No matching results" : browseKind === "modpack" ? "No Fabric modpacks found" : "No compatible content found"}</h4>
        <p>{query.trim() || categories.length ? "Try another search or clear your filters." : browseKind === "modpack" ? "Try again later or search for a specific pack." : `No ${browseKindNoun(browseKind)} were found for this instance's current compatibility filters.`}</p>
        {#if query.trim() || categories.length}<button type="button" class="btn btn-quiet" onclick={clearFilters}>Clear filters</button>{/if}
      </div>
    {/if}
    {#if nextOffset < page.totalHits}
      <button type="button" class="btn btn-quiet more" disabled={busy !== null} onclick={() => search(nextOffset)}>{busy === "search" ? "Loading…" : "Load more"}</button>
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
      <p>Minecraft {packPreview.minecraftVersion} · Fabric Loader {packPreview.fabricLoaderVersion} · {packIntoInstance ? `Adds to ${instanceName}` : "New independent instance"}</p>
      {#if mergePreview}
        {#if mergePreview.blockers.length}
          <div class="merge-blockers" role="alert">
            <strong>This pack can't be added to {instanceName}</strong>
            <ul>{#each mergePreview.blockers as blocker}<li>{blocker}</li>{/each}</ul>
          </div>
        {:else}
          <p class="merge-summary">Adds {mergePreview.added.length} file{mergePreview.added.length === 1 ? "" : "s"} and {mergePreview.overridesWritten.length} pack setting{mergePreview.overridesWritten.length === 1 ? "" : "s"}{mergePreview.alreadyPresent.length ? ` · ${mergePreview.alreadyPresent.length} already in this instance` : ""}. The pack's content becomes ordinary content of this instance; it won't follow pack updates.</p>
        {/if}
        {#if mergePreview.keptExisting.length}<details><summary>Your files kept as they are ({mergePreview.keptExisting.length})</summary><ul>{#each mergePreview.keptExisting as path}<li>{path}</li>{/each}</ul></details>{/if}
        {#if mergePreview.alreadyPresent.length}<details><summary>Already in this instance ({mergePreview.alreadyPresent.length})</summary><ul>{#each mergePreview.alreadyPresent as path}<li>{path}</li>{/each}</ul></details>{/if}
      {/if}
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
        <button type="button" class="btn btn-quiet" disabled={busy !== null} onclick={() => { packPreview = null; mergePreview = null; }}>Cancel</button>
        <button type="button" class="btn" class:btn-primary={packIntoInstance} disabled={busy !== null || optionalChanged || !!mergePreview?.blockers.length} onclick={confirmPack}>{packIntoInstance ? `Add to ${instanceName}` : "Install exact version"}</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .merge-blockers { padding: var(--space-3) var(--space-4); border-radius: var(--radius-md); background: var(--color-warning-soft); color: var(--color-warning); }
  .merge-blockers ul { margin: var(--space-2) 0 0; padding-left: 1.2em; color: var(--color-text); }
  .merge-summary { color: var(--color-text); }
  .browse { min-width: 0; container-type: inline-size; }
  .browse-heading, .browse-search, .browse-row, .preview-actions, .browse-filters { display: flex; gap: var(--space-3); align-items: center; }
  .browse-heading { justify-content: space-between; margin-bottom: var(--space-3); }
  .browse-heading { align-items: flex-start; }
  .browse-heading > div { min-width: 0; flex: 1; }
  .browse-heading .group-subtitle { max-width: 72ch; color: var(--color-text-secondary); }
  .source, .browse-meta, .filter-context { font-size: var(--text-metadata); color: var(--color-text-muted); }
  .source { white-space: nowrap; color: var(--color-text-secondary); }
  .search-field { position: relative; display: flex; min-width: 0; }
  .search-field input { padding-right: 44px !important; }
  .search-field input::-webkit-search-cancel-button { appearance: none; }
  .clear-search { position: absolute; right: 0; top: 0; bottom: 0; width: 40px; border: none; border-radius: var(--f-radius-control); background: transparent; color: var(--color-text-secondary); font: inherit; cursor: pointer; }
  .clear-search:hover { color: var(--color-text); background: var(--color-surface-hover); }
  .search-control { min-width: 0; }
  .browse-search input, .browse-search > .btn { min-height: 40px; border-radius: var(--f-radius-control); }
  .browse-search > .btn { flex: none; }
  .result-count { margin: 0 0 var(--space-3); font-size: var(--text-metadata); color: var(--color-text-secondary); overflow-wrap: anywhere; }
  .browse-error { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-3); margin-bottom: var(--space-3); }
  .browse-error p { flex: 1 1 20em; margin: 0; overflow-wrap: anywhere; }
  .browse-empty { display: grid; justify-items: start; align-content: center; gap: var(--space-2); min-height: 120px; padding: var(--space-4); background: var(--f-content-panel); border: 1px solid var(--f-edge); border-radius: var(--f-radius-control); }
  .browse-empty h4, .browse-empty p { margin: 0; }
  .browse-empty h4 { font-size: var(--text-body); }
  .browse-empty p { font-size: var(--text-secondary); color: var(--color-text-secondary); }
  .browse-kind-tabs { display: flex; gap: var(--space-2); margin-bottom: var(--space-3); flex-wrap: wrap; }
  .browse-kind-tabs [aria-pressed="true"] { color: var(--color-text); background: var(--color-surface-raised); }
  .browse-search { align-items: end; margin-bottom: var(--space-3); }
  .search-control, .version-choice { display: grid; gap: var(--space-1); flex: 1; }
  /* Text-field styling is scoped to the search input specifically: a bare
     `.browse input` selector also matched the category checkboxes and blew
     each one up into a full-width padded box, tearing the picker apart. */
  .browse input[type="search"], .browse select { width: 100%; padding: var(--space-2) var(--space-3); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text); font: inherit; }
  .browse-filters { flex-wrap: wrap; align-items: end; gap: var(--space-3); margin-bottom: var(--space-4); }
  .sort-control, .category-filter { display: grid; gap: var(--space-1); width: max(180px, 12em); max-width: 100%; }
  .filter-context { margin-left: auto; padding-bottom: var(--space-2); overflow-wrap: anywhere; }
  .category-filter { position: relative; }
  .browse .filter-input { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); width: 100%; min-height: 40px; height: auto; padding: var(--space-2) var(--space-3); border: 1px solid var(--color-border-strong); border-radius: var(--f-radius-control); background: var(--color-surface-sunken); color: var(--color-text); font: inherit; font-size: var(--text-body); cursor: pointer; }
  .browse .filter-input:hover { border-color: var(--color-text-muted); background: var(--color-surface-hover); }
  .browse .filter-input:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .select-wrap { position: relative; display: block; }
  .browse select.filter-input { appearance: none; padding-right: 36px; }
  .filter-chevron { position: absolute; right: var(--space-3); inset-block: 0; display: flex; align-items: center; pointer-events: none; }
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
  .browse-status { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-3) var(--space-4); border: 1px solid var(--f-edge); border-radius: var(--f-radius-control); background: var(--f-content-panel); color: var(--color-text-secondary); overflow-wrap: anywhere; }
  .browse-loading { min-height: 96px; }
  .browse-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-3); }
  .browse-row { display: grid; grid-template-columns: 44px minmax(0, 1fr); align-items: start; gap: var(--space-2) var(--space-3); padding: var(--space-4); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-md); background: var(--f-panel); }
  .browse-glyph { display: grid; place-items: center; width: 44px; height: 44px; grid-row: 1 / 3; }
  .browse-actions { display: flex; flex-wrap: wrap; align-items: center; justify-content: flex-end; gap: var(--space-2); grid-column: 2; align-self: end; }
  .install-action { display: flex; align-items: center; justify-content: center; gap: var(--space-1); min-width: 34px; min-height: 32px; font-size: 19px; line-height: 1; }
  .action-state { font-size: var(--text-metadata); }
  .browse-only-badge { padding: 4px var(--space-2); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text-muted); font-size: var(--text-metadata); white-space: nowrap; }
  .browse-notice { position: fixed; z-index: 20; right: var(--space-4); bottom: var(--space-4); max-width: min(360px, calc(100vw - 32px)); padding: var(--space-3) var(--space-4); border: 1px solid var(--color-surface-edge); border-radius: var(--radius-md); background: var(--color-surface-raised); box-shadow: var(--shadow-group); color: var(--color-text); font-size: var(--text-secondary); opacity: 1; transition: opacity 300ms ease-out; }
  .browse-notice.exiting { opacity: 0; }
  .browse-copy { min-width: 0; flex: 1; }
  .browse-copy h4, .project h4, .preview h4 { margin: 0; }
  .browse-copy h4 { line-height: 1.35; overflow-wrap: anywhere; }
  .browse-copy { overflow-wrap: anywhere; }
  .browse-copy p { display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 3; line-clamp: 3; overflow: hidden; line-height: 1.4; }
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
  /* Mods prioritize scanning: a single compact list with aligned actions. */
  .browse-mods .browse-heading { align-items: flex-start; }
  .browse-mods .browse-heading > div { flex: 1 1 24em; min-width: 0; }
  .browse-mods .browse-heading .group-subtitle { max-width: 72ch; color: var(--color-text-secondary); }
  .browse-mods .source { color: var(--color-text-secondary); }
  .browse-mods .browse-search input,
  .browse-mods .filter-input,
  .browse-mods .browse-search .btn { min-height: 40px; border-radius: var(--f-radius-control); }
  .browse-mods .filter-input { height: auto; }
  .browse-mods .sort-control, .browse-mods .category-filter { width: max(180px, 12em); }
  .browse-mods .filter-chevron { bottom: 0; height: auto; }
  .browse-mods .browse-search .btn { flex: none; }
  /* Modpacks reuse the approved Mods rows; their review workflow stays distinct. */
  :is(.browse-mods, .browse-modpacks) .browse-list { grid-template-columns: minmax(0, 1fr); gap: 0; border: 1px solid var(--f-edge); border-radius: var(--f-radius-control); }
  :is(.browse-mods, .browse-modpacks) .browse-row { padding: var(--space-3); border: none; border-bottom: 1px solid var(--f-edge); border-radius: 0; }
  :is(.browse-mods, .browse-modpacks) .browse-row:first-child { border-radius: var(--f-radius-control) var(--f-radius-control) 0 0; }
  :is(.browse-mods, .browse-modpacks) .browse-row:last-child { border-bottom: none; border-radius: 0 0 var(--f-radius-control) var(--f-radius-control); }
  :is(.browse-mods, .browse-modpacks) .browse-copy h4 { font-size: var(--text-body); font-weight: 600; }
  :is(.browse-mods, .browse-modpacks) .browse-copy p { margin-block: var(--space-1); font-size: var(--text-secondary); -webkit-line-clamp: 2; line-clamp: 2; }
  :is(.browse-mods, .browse-modpacks) .browse-meta { color: var(--color-text-secondary); }
  :is(.browse-mods, .browse-modpacks) .browse-actions .btn { min-height: 40px; }
  .browse-mods > .browse-status { padding: var(--space-4); border: 1px solid var(--f-edge); border-radius: var(--f-radius-control); background: var(--f-content-panel); color: var(--color-text-secondary); }
  .browse-resource-packs .browse-list { grid-template-columns: minmax(0, 1fr); }
  .browse-resource-packs .browse-row { grid-template-columns: 64px minmax(0, 1fr); padding: var(--space-3); }
  .browse-resource-packs .browse-glyph { width: 64px; height: 64px; }
  .browse-resource-packs .browse-copy h4 { font-size: var(--text-body); }
  .browse-resource-packs .browse-copy p { margin-block: var(--space-1); font-size: var(--text-secondary); -webkit-line-clamp: 2; line-clamp: 2; }
  .browse-resource-packs .browse-meta { color: var(--color-text-secondary); }
  .browse-resource-packs .browse-actions .btn { min-height: 40px; }
  @container (min-width: 761px) { .browse-resource-packs .browse-list { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @container (min-width: 561px) {
    :is(.browse-mods, .browse-modpacks) .browse-row { grid-template-columns: 44px minmax(0, 1fr) auto; align-items: center; }
    :is(.browse-mods, .browse-modpacks) .browse-glyph { grid-row: 1; }
    :is(.browse-mods, .browse-modpacks) .browse-actions { grid-column: 3; grid-row: 1; align-self: center; }
  }
  @media (max-width: 760px) { .browse-heading { align-items: flex-start; flex-wrap: wrap; } .filter-context { margin-left: 0; } }
  @container (max-width: 900px) { .browse-list { grid-template-columns: minmax(0, 1fr); } }
  @container (max-width: 440px) { .sort-control, .category-filter { width: calc((100% - var(--space-3)) / 2); min-width: 140px; } .filter-context { margin-left: 0; width: 100%; padding-bottom: 0; } .browse-row { padding: var(--space-3); } }
  @media (prefers-reduced-motion: reduce) { .browse-notice { transition: none; } }
</style>
