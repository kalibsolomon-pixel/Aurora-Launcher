<script lang="ts">
  import { onMount } from "svelte";
  import { getRecentWorlds, getRecentServers, type RecentGameplayTarget, type WidgetSize } from "$lib/backend";
  import { launcher } from "../store.svelte";
  import { recentLabel } from "../homeHistory";
  let { mode, size }: { mode: "world" | "server"; size: WidgetSize } = $props();
  let entries = $state<RecentGameplayTarget[]>([]);
  let loading = $state(true);
  let error = $state("");
  let pending = $state<string | null>(null);
  const shown = $derived(entries.slice(0, size === "small" ? 3 : 5));
  const instances = $derived(launcher.launcherState?.instances ?? []);
  function instanceLabel(id: string): string {
    const found = instances.find(item => item.id === id);
    if (!found) return "Missing instance";
    const repeated = instances.filter(item => item.displayName === found.displayName).length > 1;
    return repeated ? `${found.displayName} · ${id.slice(0, 8)}` : found.displayName;
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
    catch (cause) { error = cause instanceof Error ? cause.message : "Recent history is unavailable."; }
    finally { loading = false; }
  }
  let lastCompleted = launcher.playProcess;
  $effect(() => {
    const process = launcher.playProcess;
    if (process && process !== lastCompleted && (process.status === "exited" || process.status === "failed")) {
      lastCompleted = process;
      void load();
    }
  });
  onMount(() => { void load(); });
</script>

{#if loading}<p class="note">Loading recent {mode === "world" ? "worlds" : "servers"}…</p>
{:else if !entries.length}<p class="note">No recently played {mode === "world" ? "worlds" : "servers"}.</p>
{:else}
  <ul>
    {#each shown as entry (entry.id)}
      <li>
        <div class="identity">
          <strong title={entry.displayName}>{entry.displayName}</strong>
          <span class="meta" title={instanceLabel(entry.instanceId)}>{instanceLabel(entry.instanceId)} · {recentLabel(entry.lastPlayedAt, Date.now())}{entry.available ? "" : " · Unavailable"}</span>
        </div>
        <button type="button" class="launch" aria-label={`Quick Launch ${mode === "world" ? "world" : "server"} ${entry.displayName} in ${instanceLabel(entry.instanceId)}`} disabled={!entry.available || !launcher.accountsState?.selectedAccountId || launcher.playBusy || pending !== null || (launcher.playProcess?.instanceId === entry.instanceId && ["starting", "running"].includes(launcher.playProcess.status))} onclick={() => launch(entry)} title={entry.available ? "Quick Launch" : "Target unavailable"}>
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
  .identity { min-width: 0; flex: 1; display: grid; gap: 2px; }
  strong, .meta { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  strong { font-size: var(--text-secondary); font-weight: 600; }
  .meta, .note { font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .note { margin: var(--space-2) 0; }
  .launch { flex: none; width: 32px; height: 32px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text); cursor: pointer; }
  .launch:hover:not(:disabled) { background: var(--color-accent-soft); color: var(--color-accent); }
  .launch:active:not(:disabled) { transform: scale(.96); }
  .launch:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .launch:disabled { opacity: .45; cursor: not-allowed; }
  .error { font-size: var(--text-metadata); color: var(--color-error); overflow-wrap: anywhere; }
</style>
