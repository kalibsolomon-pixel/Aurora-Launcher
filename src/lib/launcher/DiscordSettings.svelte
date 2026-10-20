<script lang="ts">
  import { onMount } from "svelte";
  import { discord, connectionLabels } from "./discord.svelte";
  import type { DiscordPreferences } from "$lib/backend";
  import SettingsToggle from './SettingsToggle.svelte';
  import SettingsFeedback from './SettingsFeedback.svelte';
  import Tooltip from '$lib/shell/Tooltip.svelte';
  let saved = $state(false);
  let retryOperation = $state<(() => Promise<void>) | null>(null);
  async function save(field: keyof DiscordPreferences, value: boolean) {
    if (discord.busy) return;
    retryOperation = () => discord.change(field, value); saved = false;
    await retryOperation(); saved = !discord.error;
  }
  async function retry() {
    if (!retryOperation) { await discord.refresh(); return; }
    saved = false; await retryOperation(); saved = !discord.error;
  }
  const fields: { id: keyof DiscordPreferences; label: string; detail: string }[] = [
    { id: "elapsedTime", label: "Elapsed Play Time", detail: "Time since the supervised game process started." },
    { id: "world", label: "Show World", detail: "Displays the singleplayer world’s name when a compatible Aurora Client reports it." },
    { id: "server", label: "Show Server", detail: "Displays the multiplayer server’s saved/display name when available." },
    { id: "serverAddress", label: "Show Server Address", detail: "Also allows the raw hostname/address to appear. Requires Show Server." },
  ];
  onMount(() => {
    let disposed = false; let stop: (() => void) | undefined;
    void discord.subscribe().then(unlisten => { if (disposed) unlisten(); else stop = unlisten; }).catch(() => { void discord.refresh(); });
    return () => { disposed = true; stop?.(); };
  });
</script>
<section class="group" aria-labelledby="discord-title">
  <div class="group-heading"><h3 id="discord-title" class="group-title">Discord</h3>
    <Tooltip label="Discord connection status">{discord.state ? discord.state.configured && !discord.state.preferences.enabled ? 'Rich Presence is off' : connectionLabels[discord.state.connection] : "Reading connection status…"}. World and server details require a compatible, active Aurora Client. Discord’s activity visibility settings also apply.</Tooltip>
  </div>
  <SettingsToggle id="discord-enabled" title="Discord Rich Presence" description="Publishes In Launcher, Starting Minecraft or Playing Minecraft. Turning off clears Aurora’s activity. Your detail preferences are kept while it is off."
    label="Enable Discord Rich Presence" checked={discord.state?.preferences.enabled ?? false} disabled={discord.busy || !discord.state} change={value => save('enabled', value)} />
  {#each fields as field}
    <SettingsToggle id={`discord-${field.id}`} title={field.label} description={field.detail}
      label={`Display ${field.label}`} checked={discord.state?.preferences[field.id] ?? false}
      disabled={discord.busy || !discord.state || (field.id === 'serverAddress' && !discord.state.preferences.server)} change={value => save(field.id, value)} />
  {/each}
</section>
<SettingsFeedback busy={discord.busy} error={discord.error} {saved} retryLabel={retryOperation ? 'Retry save' : 'Retry status'} retry={() => void retry()} />
