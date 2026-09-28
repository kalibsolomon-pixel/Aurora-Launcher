<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { getDailyPlaytime, getPlaytimeSummary, type DailyPlaytime, type PlaytimeSummary, type WidgetSize } from "$lib/backend";
  import { formatPlaytime } from "../homeHistory";
  import { launcher } from "../store.svelte";
  let { size, testFixture }: { size: WidgetSize; testFixture?: { summary: PlaytimeSummary; daily: DailyPlaytime[] } } = $props();
  const initialFixture = untrack(() => testFixture);
  let range = $state<"7d" | "30d" | "all">("7d");
  let summary = $state<PlaytimeSummary | null>(initialFixture?.summary ?? null);
  let daily = $state<DailyPlaytime[]>(initialFixture?.daily ?? []);
  let error = $state("");
  let loading = $state(!initialFixture);
  const total = $derived(range === "7d" ? summary?.last7DaysMs : range === "30d" ? summary?.last30DaysMs : summary?.allTimeMs);
  const shown = $derived(range === "7d" ? daily.slice(-7) : daily);
  const peak = $derived(Math.max(1, ...shown.map(day => day.durationMs)));
  function dayLabel(day: number): string {
    return new Date(day * 86400000).toLocaleDateString(undefined, { month: "short", day: "numeric", timeZone: "UTC" });
  }
  async function load(): Promise<void> {
    try {
      [summary, daily] = await Promise.all([getPlaytimeSummary(), getDailyPlaytime(30)]);
      error = "";
    } catch (cause) { error = cause instanceof Error ? cause.message : "Playtime is unavailable."; }
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
  onMount(() => { if (!initialFixture) void load(); });
</script>

<div class="range" role="group" aria-label="Playtime range">
  <button type="button" class:active={range === "7d"} aria-pressed={range === "7d"} onclick={() => range = "7d"}>7D</button>
  <button type="button" class:active={range === "30d"} aria-pressed={range === "30d"} onclick={() => range = "30d"}>30D</button>
  <button type="button" class:active={range === "all"} aria-pressed={range === "all"} onclick={() => range = "all"}>All</button>
</div>
{#if error}<p role="alert" class="error">{error}</p>
{:else if loading}<p class="note">Loading playtime…</p>
{:else if !summary || summary.allTimeMs === 0}<p class="note">No playtime recorded yet.</p>
{:else}
  <p class="total">{formatPlaytime(total ?? 0)}</p>
  <p class="note">{range === "7d" ? "Past 7 days" : range === "30d" ? "Past 30 days" : "All recorded time"} · UTC</p>
  {#if range !== "all"}
    <div class="chart" class:compact={size === "small"} role="list" aria-label={`Daily playtime for the past ${shown.length} days`}>
      {#each shown as day (day.day)}
        <div class="day" role="listitem" title={`${dayLabel(day.day)}: ${formatPlaytime(day.durationMs)}`} aria-label={`${dayLabel(day.day)}: ${formatPlaytime(day.durationMs)}`}>
          <span class="bar" style={`height: ${Math.max(day.durationMs ? 7 : 2, day.durationMs / peak * 100)}%`}></span>
          {#if range === "7d"}<span class="label">{new Date(day.day * 86400000).toLocaleDateString(undefined, { weekday: "narrow", timeZone: "UTC" })}</span>{/if}
        </div>
      {/each}
    </div>
  {:else}
    <div class="all-summary"><span>Past 30 days <strong>{formatPlaytime(summary.last30DaysMs)}</strong></span><span>Past 7 days <strong>{formatPlaytime(summary.last7DaysMs)}</strong></span></div>
    <p class="note all-note">Daily detail is available for the past 30 days.</p>
  {/if}
{/if}

<style>
  .range { display: flex; justify-content: flex-end; gap: var(--space-1); margin-bottom: var(--space-3); }
  .range button { font: inherit; font-size: var(--text-metadata); border: 0; border-radius: var(--radius-sm); padding: var(--space-1) var(--space-2); color: var(--color-text-secondary); background: transparent; cursor: pointer; }
  .range button:hover, .range button.active { color: var(--color-text); background: var(--color-surface-sunken); }
  button:focus-visible, .day:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .total { margin: 0; font-size: clamp(1.5rem, 3vw, 2.2rem); font-weight: 650; line-height: 1.1; }
  .note { margin: var(--space-1) 0; color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .error { color: var(--color-error); overflow-wrap: anywhere; }
  .chart { height: 86px; display: grid; grid-template-columns: repeat(30, minmax(0, 1fr)); gap: 2px; margin-top: var(--space-3); min-width: 0; }
  .chart:has(.label) { grid-template-columns: repeat(7, minmax(0, 1fr)); gap: var(--space-2); }
  .chart.compact { height: 64px; }
  .day { position: relative; display: flex; align-items: flex-end; min-width: 0; height: calc(100% - 18px); border-bottom: 1px solid var(--color-border); }
  .bar { display: block; width: 100%; border-radius: var(--radius-sm) var(--radius-sm) 0 0; background: var(--color-accent); min-height: 2px; opacity: .8; }
  .day:hover .bar { opacity: 1; }
  .label { position: absolute; top: calc(100% + 3px); left: 0; right: 0; text-align: center; font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .all-note { margin-top: var(--space-3); }
  .all-summary { display: flex; flex-wrap: wrap; gap: var(--space-2) var(--space-4); margin-top: var(--space-3); font-size: var(--text-metadata); color: var(--color-text-secondary); }
  .all-summary strong { color: var(--color-text); font-weight: 600; }
</style>
