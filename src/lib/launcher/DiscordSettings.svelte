<script lang="ts">
  import { onMount } from "svelte";
  import { discord, connectionLabels } from "./discord.svelte";
  import type { DiscordPreferences } from "$lib/backend";
  const fields: { id: keyof DiscordPreferences; label: string; detail: string }[] = [
    { id: "minecraftVersion", label: "Minecraft version", detail: "The exact installed game version." },
    { id: "platform", label: "Platform / loader", detail: "Vanilla or Fabric." },
    { id: "auroraActive", label: "Aurora Client active state", detail: "Verified Aurora bootstrap active at launch; not a live client health signal." },
    { id: "elapsedTime", label: "Elapsed play time", detail: "Time since the supervised game process started." },
    { id: "instanceName", label: "Instance name", detail: "Shares your chosen instance name with Discord." },
  ];
  onMount(() => {
    let disposed = false; let stop: (() => void) | undefined;
    void discord.subscribe().then(unlisten => { if (disposed) unlisten(); else stop = unlisten; }).catch(() => { void discord.refresh(); });
    return () => { disposed = true; stop?.(); };
  });
</script>
<section class="group" aria-labelledby="discord-title">
  <div class="group-heading"><div><h3 id="discord-title" class="group-title">Discord</h3><p class="group-subtitle">Optional activity on your desktop Discord profile. No account link required.</p></div></div>
  <div class="group-row"><div class="group-row-main"><span class="group-row-title" role="status">{discord.state ? connectionLabels[discord.state.connection] : "Reading connection status…"}</span>
    <span class="group-row-detail">{discord.state?.configured ? "Open the Discord desktop app, then connect. Connection alone does not enable activity." : "This build needs Aurora’s Discord application registration before it can connect. Your preferences can still be saved."}</span></div>
    <button type="button" class="btn" disabled={discord.busy || !discord.state?.configured} onclick={() => discord.connect()}>{discord.busy ? "Connecting…" : discord.state?.connection === "connected" || discord.state?.connection === "closed" || discord.state?.connection === "failed" ? "Reconnect to Discord" : "Connect to Discord"}</button>
  </div>
  <label class="group-row"><div class="group-row-main"><span class="group-row-title">Discord Rich Presence</span><span class="group-row-detail">Publishes In Launcher, Starting Minecraft or Playing Minecraft. Turning off clears Aurora’s activity.</span></div>
    <input type="checkbox" aria-label="Enable Discord Rich Presence" checked={discord.state?.preferences.enabled ?? false} disabled={discord.busy || !discord.state} onchange={event => discord.change("enabled", event.currentTarget.checked)} />
  </label>
  {#each fields as field}
    <label class="group-row"><div class="group-row-main"><span class="group-row-title">{field.label}</span><span class="group-row-detail">{field.detail}</span></div>
      <input type="checkbox" aria-label={`Display ${field.label}`} checked={discord.state?.preferences[field.id] ?? false} disabled={discord.busy || !discord.state} onchange={event => discord.change(field.id, event.currentTarget.checked)} />
    </label>
  {/each}
  <div class="group-row"><div class="group-row-main"><span class="group-row-title">World and server · unavailable</span><span class="group-row-detail">Aurora Client does not currently report authoritative gameplay activity to this launcher. World names and server names/addresses are never published.</span></div></div>
  {#if discord.error}<p class="group-row inline-message inline-message-error" role="alert">{discord.error}</p>{/if}
  <p class="group-footer">Off by default. All extra details require opt-in. Discord’s own activity visibility settings also apply. Connection problems never block Play.</p>
</section>
