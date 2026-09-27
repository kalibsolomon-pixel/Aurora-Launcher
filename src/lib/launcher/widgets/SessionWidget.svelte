<script lang="ts">
  import type { InstanceSummary } from "$lib/backend";
  import { launcher } from "../store.svelte";
  let { instance }: { instance: InstanceSummary | null } = $props();
  const process = $derived(launcher.playProcess?.instanceId === instance?.id ? launcher.playProcess : null);
</script>
{#if process && process.status !== "stopped"}
  <p class="session-state">{process.status === "running" ? "Minecraft is running" : process.status === "starting" ? "Minecraft is starting" : process.status === "exited" ? "Minecraft exited" : "Launch failed"}</p>
  {#if process.startedAtUnixSeconds}<p>Started {new Date(process.startedAtUnixSeconds * 1000).toLocaleString()}</p>{/if}
  {#if process.exitCode !== null}<p>Exit code {process.exitCode}</p>{/if}
{:else}<p>No launch observed for this instance.</p>{/if}
<p class="note">Current launcher session only.</p>
<style>p { margin: var(--space-1) 0; color: var(--color-text-secondary); font-size: var(--text-secondary); } .session-state { color: var(--color-text); } .note { font-size: var(--text-metadata); margin-top: var(--space-3); }</style>
