<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { getRecentWorlds, getRecentServers, setRecentServerFavorite, refreshRecentServerStatus, type RecentGameplayTarget, type RecentServerPresentation, type WidgetSize } from "$lib/backend";
  import { launcher } from "../store.svelte";
  import { recentLabel } from "../homeHistory";
  import { motdPlainText, presentationsById, rowDetail, rowName, segmentStyle, statusLabel } from "../serverPresentation";
  import Artwork from '$lib/shell/Artwork.svelte';
  let { mode, size, testFixture, testPresentations }: { mode: "world" | "server"; size: WidgetSize; testFixture?: RecentGameplayTarget[]; testPresentations?: RecentServerPresentation[] } = $props();
  const initialFixture = untrack(() => testFixture);
  let entries = $state<RecentGameplayTarget[]>(initialFixture ?? []);
  let presentations = $state(presentationsById(untrack(() => testPresentations ?? [])));
  let loading = $state(!initialFixture);
  let error = $state("");
  let pending = $state<string | null>(null);
  let enriching = $state(false);
  let favoritesOnly = $state(false);
  let favoriteBusy = $state<string | null>(null);
  const shown = $derived(mode === "server" ? (favoritesOnly ? entries.filter(entry => entry.favorite) : entries.slice(0, 100)) : entries.slice(0, size === "small" ? 3 : 5));
  const requested = new Set<string>();
  const waiting = new Set<string>();
  let revision = 0;
  function visible(node: HTMLLIElement, id: string) {
    if (mode !== "server" || initialFixture) return {};
    const observer = new IntersectionObserver(rows => {
      if (rows.some(row => row.isIntersecting) && !requested.has(id)) {
        waiting.add(id); queueMicrotask(() => void enrich());
      }
    }, { root: node.closest(".history-scroll"), rootMargin: "50px" });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }
  async function favorite(entry: RecentGameplayTarget) {
    if (favoriteBusy) return;
    favoriteBusy = entry.id; error = "";
    try { await setRecentServerFavorite(entry.id, !entry.favorite); await load(); }
    catch (cause) { error = cause instanceof Error ? cause.message : "The favorite could not be saved."; }
    finally { favoriteBusy = null; }
  }
  const instances = $derived(launcher.launcherState?.instances ?? []);
  function instanceLabel(id: string): string {
    const found = instances.find(item => item.id === id);
    if (!found) return "Missing instance";
    const repeated = instances.filter(item => item.displayName === found.displayName).length > 1;
    return repeated ? `${found.displayName} · ${id.slice(0, 8)}` : found.displayName;
  }
  function presentation(entry: RecentGameplayTarget): RecentServerPresentation | undefined {
    return mode === "server" ? presentations.get(entry.id) : undefined;
  }
  /** History renders first; enrichment overlays presentation facts when the
   * bounded native refresh answers. Failure is silent: rows stay usable. */
  async function enrich(): Promise<void> {
    if (mode !== "server" || initialFixture || enriching || !waiting.size) return;
    const ids = [...waiting].filter(id => !requested.has(id)).slice(0, 20);
    if (!ids.length) return;
    ids.forEach(id => { waiting.delete(id); requested.add(id); });
    const current = revision;
    enriching = true;
    try {
      const result = await refreshRecentServerStatus(ids);
      if (current === revision) presentations = new Map([...presentations, ...presentationsById(result)]);
    } catch {
      /* Unavailable enrichment never degrades history rows. */
    } finally {
      enriching = false;
      if (waiting.size) void enrich();
    }
  }
  async function launch(entry: RecentGameplayTarget): Promise<void> {
    if (!entry.available || pending || launcher.playBusy || (launcher.playProcess?.instanceId === entry.instanceId && ["starting", "running"].includes(launcher.playProcess.status))) return;
    pending = entry.id;
    error = "";
    await launcher.runQuickPlay(entry.id, mode);
    if (launcher.playError) error = launcher.playError.message;
    pending = null;
  }
  async function load(): Promise<void> {
    try { entries = await (mode === "world" ? getRecentWorlds(null, 5) : getRecentServers(null, 200)); error = ""; }
    catch (cause) { error = cause instanceof Error ? cause.message : "Recent history is unavailable."; return; }
    finally { loading = false; }
    revision++;
  }
  let lastCompleted = launcher.playProcess;
  $effect(() => {
    const process = launcher.playProcess;
    if (process && process !== lastCompleted && (process.status === "exited" || process.status === "failed")) {
      lastCompleted = process;
      void load();
    }
  });
  onMount(() => { if (!initialFixture) void load(); });
</script>

{#if mode === "server" && entries.length}
  <div class="history-tabs" aria-label="Server history view">
    <button type="button" aria-pressed={!favoritesOnly} onclick={() => favoritesOnly = false}>Recent</button>
    <button type="button" aria-pressed={favoritesOnly} onclick={() => favoritesOnly = true}>Favorites · {entries.filter(entry => entry.favorite).length}</button>
  </div>
{/if}
{#if loading}<p class="note">Loading recent {mode === "world" ? "worlds" : "servers"}…</p>
{:else if !entries.length}<p class="note">No recently played {mode === "world" ? "worlds" : "servers"}.</p>
{:else}
  {#if favoritesOnly && !shown.length}<p class="note">Star a server to keep it in Favorites.</p>{/if}
  <!-- A scroll region needs its own focus stop so PageDown/Arrow keys can scroll without activating a row. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="history-scroll" class:server-history={mode === "server"} role="region" tabindex={mode === "server" ? 0 : undefined} aria-label={mode === "server" ? "Scrollable server history" : "Recent worlds"}>
  <ul>
    {#each shown as entry (entry.id)}
      {@const detail = presentation(entry)}
      {@const name = rowName(entry, detail)}
      {@const firstMotdLine = detail ? detail.motd.find(line => line.some(segment => segment.text.trim())) : undefined}
      <li class:enriched={mode === "server"} use:visible={entry.id}>
        {#if mode === "server"}
          <span class="favicon"><Artwork source={detail?.favicon} fallback="server" size={36} /></span>
        {/if}
        <div class="identity">
          <strong title={mode === "server" && rowDetail(detail) ? `${name} — ${rowDetail(detail)}` : name}>{name}{#if mode === "server" && detail}<span class="dot" class:online={detail.status === "online"} class:offline={detail.status === "offline"} title={statusLabel(detail)}></span>{/if}</strong>
          {#if mode === "server" && firstMotdLine}
            <span class="motd" title={motdPlainText(detail?.motd ?? [])}>
              {#each firstMotdLine as segment, index (index)}<span style={segmentStyle(segment)}>{segment.text}</span>{/each}
            </span>
          {/if}
          <span class="meta" title={instanceLabel(entry.instanceId)}>{instanceLabel(entry.instanceId)} · {recentLabel(entry.lastPlayedAt, Date.now())}{mode === "server" && statusLabel(detail) ? ` · ${statusLabel(detail)}` : ""}{entry.available ? "" : " · Unavailable"}</span>
        </div>
        {#if mode === "server"}<button type="button" class="favorite" class:pinned={entry.favorite} aria-pressed={!!entry.favorite} aria-label={`${entry.favorite ? "Unfavorite" : "Favorite"} ${name}`} title={entry.favorite ? "Remove favorite" : "Keep as favorite"} disabled={favoriteBusy !== null} onclick={() => favorite(entry)}>{entry.favorite ? "★" : "☆"}</button>{/if}
        <button type="button" class="launch" aria-label={`Quick Launch ${mode === "world" ? "world" : "server"} ${name} in ${instanceLabel(entry.instanceId)}`} disabled={!entry.available || !launcher.accountsState?.selectedAccountId || launcher.playBusy || pending !== null || (launcher.playProcess?.instanceId === entry.instanceId && ["starting", "running"].includes(launcher.playProcess.status))} onclick={() => launch(entry)} title={entry.available ? "Quick Launch" : "Target unavailable"}>
          {pending === entry.id ? "…" : "▶"}
        </button>
      </li>
    {/each}
  </ul>
  </div>
{/if}
{#if error}<p class="error" role="alert">{error}</p>{/if}
{#if launcher.playProcess?.status === "failed" && shown.some(entry => entry.instanceId === launcher.playProcess?.instanceId)}
  <p class="error" role="alert">{launcher.playProcess.message ?? "Minecraft could not be started."}</p>
{/if}
<style>
  .server-history { max-height: 246px; overflow-y: auto; overflow-x: hidden; scrollbar-gutter: stable; overscroll-behavior: contain; }
  .server-history:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 3px; border-radius: var(--radius-sm); }
  .history-tabs { display: flex; gap: var(--space-1); margin-bottom: var(--space-2); }
  .history-tabs button, .favorite { border: 0; background: transparent; color: var(--color-text-secondary); border-radius: var(--radius-sm); cursor: pointer; }
  .history-tabs button { padding: 4px 8px; font: inherit; font-size: var(--text-metadata); }
  .history-tabs button[aria-pressed="true"] { background: var(--color-accent-soft); color: var(--color-accent); }
  .favorite { width: 28px; height: 32px; flex: none; font-size: 21px; }
  .favorite:hover, .favorite.pinned { color: var(--color-accent); }
  .favorite:focus-visible, .history-tabs button:focus-visible { outline: 2px solid var(--color-accent); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; align-items: center; gap: var(--space-2); min-width: 0; padding: var(--space-2) 0; border-top: 1px solid var(--color-border); }
  li:first-child { border-top: 0; }
  li.enriched { align-items: flex-start; }
  .identity { min-width: 0; flex: 1; display: grid; gap: 2px; }
  strong, .meta { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  strong { font-size: var(--text-secondary); font-weight: 600; display: flex; align-items: center; gap: 6px; }
  .meta, .note { font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .note { margin: var(--space-2) 0; }
  .favicon { flex: none; display: inline-grid; width: 36px; height: 36px; margin-top: 1px; }
  .motd { font-size: var(--text-metadata); color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dot { flex: none; width: 7px; height: 7px; border-radius: 50%; background: var(--color-text-muted); }
  .dot.online { background: var(--color-success); }
  .dot.offline { background: var(--color-text-muted); }
  .launch { flex: none; width: 32px; height: 32px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text); cursor: pointer; }
  .launch:hover:not(:disabled) { background: var(--color-accent-soft); color: var(--color-accent); }
  .launch:active:not(:disabled) { transform: scale(.96); }
  .launch:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .launch:disabled { opacity: .45; cursor: not-allowed; }
  .error { font-size: var(--text-metadata); color: var(--color-error); overflow-wrap: anywhere; }
</style>
