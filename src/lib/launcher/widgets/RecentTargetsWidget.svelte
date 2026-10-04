<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { getRecentWorlds, getRecentServers, refreshRecentServerStatus, type RecentGameplayTarget, type RecentServerMotdSegment, type RecentServerPresentation, type WidgetSize } from "$lib/backend";
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
  const shown = $derived(entries.slice(0, size === "small" ? 3 : 5));
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
    if (mode !== "server" || initialFixture || enriching || !entries.length) return;
    enriching = true;
    try {
      presentations = presentationsById(await refreshRecentServerStatus(entries.map(entry => entry.id)));
    } catch {
      /* Unavailable enrichment never degrades history rows. */
    } finally {
      enriching = false;
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
    try { entries = await (mode === "world" ? getRecentWorlds(null, 5) : getRecentServers(null, 5)); error = ""; }
    catch (cause) { error = cause instanceof Error ? cause.message : "Recent history is unavailable."; return; }
    finally { loading = false; }
    await enrich();
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

{#if loading}<p class="note">Loading recent {mode === "world" ? "worlds" : "servers"}…</p>
{:else if !entries.length}<p class="note">No recently played {mode === "world" ? "worlds" : "servers"}.</p>
{:else}
  <ul>
    {#each shown as entry (entry.id)}
      {@const detail = presentation(entry)}
      {@const name = rowName(entry, detail)}
      {@const firstMotdLine = detail ? detail.motd.find(line => line.some(segment => segment.text.trim())) : undefined}
      <li class:enriched={mode === "server"}>
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
        <button type="button" class="launch" aria-label={`Quick Launch ${mode === "world" ? "world" : "server"} ${name} in ${instanceLabel(entry.instanceId)}`} disabled={!entry.available || !launcher.accountsState?.selectedAccountId || launcher.playBusy || pending !== null || (launcher.playProcess?.instanceId === entry.instanceId && ["starting", "running"].includes(launcher.playProcess.status))} onclick={() => launch(entry)} title={entry.available ? "Quick Launch" : "Target unavailable"}>
          {pending === entry.id ? "…" : "▶"}
        </button>
      </li>
    {/each}
  </ul>
{/if}
{#if error}<p class="error" role="alert">{error}</p>{/if}
{#if launcher.playProcess?.status === "failed" && shown.some(entry => entry.instanceId === launcher.playProcess?.instanceId)}
  <p class="error" role="alert">{launcher.playProcess.message ?? "Minecraft could not be started."}</p>
{/if}
<style>
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
