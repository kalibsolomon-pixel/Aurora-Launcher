<script lang="ts">
  import { onMount } from "svelte";
  import { discord, connectionLabels } from "./discord.svelte";
  import type { DiscordPreferences } from "$lib/backend";
  import SettingsToggle from './SettingsToggle.svelte';
  import SettingsFeedback from './SettingsFeedback.svelte';
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
    { id: "minecraftVersion", label: "Minecraft version", detail: "The exact installed game version." },
    { id: "platform", label: "Platform / loader", detail: "The installed platform and loader reported by Aurora." },
    { id: "auroraActive", label: "Aurora Client active state", detail: "Verified Aurora bootstrap active at launch; not a live client health signal." },
    { id: "elapsedTime", label: "Elapsed play time", detail: "Time since the supervised game process started." },
    { id: "instanceName", label: "Instance name", detail: "Shares your chosen instance name with Discord." },
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
  <div class="group-heading"><div><h3 id="discord-title" class="group-title">Discord</h3><p class="group-subtitle">Optional activity on your desktop Discord profile. No account link required.</p></div></div>
  <div class="group-row"><div class="group-row-main"><span class="group-row-title" role="status">{discord.state ? discord.state.configured && !discord.state.preferences.enabled ? 'Rich Presence is off' : connectionLabels[discord.state.connection] : "Reading connection status…"}</span>
    <span class="group-row-detail">{!discord.state ? discord.error ? "Connection status is unavailable. Retry to read it again." : "Reading the native Discord integration. No account link is required." : discord.state.configured ? discord.state.preferences.enabled ? "Aurora connects automatically while Rich Presence is enabled. If Discord is closed, it will retry when the desktop app becomes available." : "Enable Rich Presence to connect automatically. Turning it off clears activity and closes the connection." : "This build needs Aurora’s Discord application registration before it can connect. Your preferences can still be saved."}</span></div>
  </div>
  <SettingsToggle id="discord-enabled" title="Discord Rich Presence" description="Publishes In Launcher, Starting Minecraft or Playing Minecraft. Turning off clears Aurora’s activity. Your detail preferences are kept while it is off."
    label="Enable Discord Rich Presence" checked={discord.state?.preferences.enabled ?? false} disabled={discord.busy || !discord.state} change={value => save('enabled', value)} />
  {#each fields as field}
    {#if field.id === "minecraftVersion"}<h4 class="privacy-heading">Game details</h4>{/if}
    {#if field.id === "world"}<h4 class="privacy-heading">World &amp; server privacy</h4>{/if}
    <SettingsToggle id={`discord-${field.id}`} title={field.label} description={field.detail}
      label={`Display ${field.label}`} checked={discord.state?.preferences[field.id] ?? false}
      disabled={discord.busy || !discord.state || (field.id === 'serverAddress' && !discord.state.preferences.server)} change={value => save(field.id, value)} />
  {/each}
  <p class="group-footer">World and server details require a compatible, active Aurora Client. Without its authenticated activity, Discord shows generic process activity.</p>
  <p class="group-footer">Off by default. All extra details require opt-in. Discord’s own activity visibility settings also apply. Connection problems never block Play.</p>
</section>
<SettingsFeedback busy={discord.busy} error={discord.error} {saved} retryLabel={retryOperation ? 'Retry save' : 'Retry status'} retry={() => void retry()} />

<style>
  .privacy-heading { margin: 0; padding: 20px 24px 12px; font-size: 12px; color: var(--color-text-secondary); font-weight: 600; border-top: 1px solid var(--color-border); }
</style>
