<script lang="ts">
  import { updates } from "$lib/launcher/updates.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import Icon from "$lib/shell/Icon.svelte";
  import {
    channelDescriptions,
    channelNote,
    launcherPhaseLabel,
    presentationNotes,
    summarizeAvailability,
  } from "$lib/launcher/updatePresentation";
  import type { UpdateReleaseChannel } from "$lib/backend";

  const channels: UpdateReleaseChannel[] = ["stable", "beta", "nightly"];
  const overview = $derived(updates.overview);
  const launcherSummary = $derived(
    overview ? summarizeAvailability(overview.launcher.availability, "launcher") : null,
  );
  const clientSummary = $derived(overview ? summarizeAvailability(overview.client, "client") : null);
  const launcherOffer = $derived(overview?.launcher.availability.kind === "updateAvailable"
    ? overview.launcher.availability
    : null);
  const clientOffer = $derived(overview?.client.kind === "updateAvailable" ? overview.client : null);
  const launcherNotes = $derived(presentationNotes(launcherOffer, null));
  const clientNotes = $derived(presentationNotes(clientOffer, null));
  const phaseLabel = $derived(
    launcherPhaseLabel(
      updates.launcherPhase ?? overview?.launcher.phase ?? null,
      updates.launcherDownloaded,
      updates.launcherTotal,
    ),
  );
  const canInstall = $derived(updates.launcherPhase === "readyToInstall" || overview?.launcher.phase === "readyToInstall");

  function goToInstance(): void {
    if (overview?.clientInstanceId) navigation.openInstance(overview.clientInstanceId, "settings");
  }
</script>

<section class="group f-surface updates-surface" aria-labelledby="updates-title">
  <div class="section-intro">
    <h3 id="updates-title">Updates</h3>
    <p>Release-driven, never automatic. Aurora checks after startup and when you ask; installing is always your choice.</p>
  </div>

  <fieldset class="choice-section">
    <legend>Release channel</legend>
    <div class="channel-grid">
      {#each channels as channel}
        <label class="channel-tile" class:chosen={overview?.channel === channel}>
          <input
            type="radio"
            name="update-channel"
            value={channel}
            checked={overview?.channel === channel}
            disabled={updates.busy}
            onchange={() => void updates.setChannel(channel)}
          />
          <span class="channel-name">{channel}</span>
          <span class="channel-detail">{channelDescriptions[channel]}</span>
          {#if overview?.channel === channel}<span class="channel-check"><Icon name="check" size={14} /></span>{/if}
        </label>
      {/each}
    </div>
    <p class="choice-note">{channelNote}</p>
  </fieldset>

  <div class="group-row">
    <div class="group-row-main">
      <span class="group-row-title">Check for updates</span>
      <span class="group-row-detail">Checks the Aurora Launcher and the selected instance's Aurora Client together.</span>
    </div>
    <button type="button" class="btn btn-primary" disabled={updates.checking || updates.busy} onclick={() => void updates.check()}>
      {updates.checking ? "Checking…" : "Check for updates"}
    </button>
  </div>

  {#if updates.error}
    <p class="inline-message inline-message-error group-row" role="alert">{updates.error.message}</p>
  {/if}

  <div class="update-domain">
    <div class="domain-heading">
      <span class="group-row-title">Aurora Launcher</span>
      <span class="domain-version">v{overview?.launcher.installedVersion ?? "—"}</span>
    </div>
    {#if launcherSummary}
      <p class="domain-line" class:tone-accent={launcherSummary.tone === "accent"} class:tone-warn={launcherSummary.tone === "warn"}>{launcherSummary.title}</p>
      <p class="group-row-detail">{launcherSummary.detail}</p>
    {/if}
    {#if launcherNotes}
      <details class="notes">
        <summary>Release notes</summary>
        <pre class="notes-body">{launcherNotes}</pre>
      </details>
    {/if}
    {#if phaseLabel}<p class="group-row-detail" role="status">{phaseLabel}</p>{/if}
    {#if launcherOffer}
      <div class="domain-actions">
        {#if canInstall}
          <button type="button" class="btn btn-primary" disabled={updates.busy} onclick={() => void updates.installLauncher(launcherOffer.candidate)}>
            Install and restart
          </button>
        {:else if updates.launcherPhase === "downloading" || overview?.launcher.phase === "downloading"}
          <button type="button" class="btn" disabled>Downloading…</button>
        {:else}
          <button type="button" class="btn btn-primary" disabled={updates.busy} onclick={() => void updates.downloadLauncher(launcherOffer.candidate)}>
            Download update
          </button>
        {/if}
      </div>
      <p class="choice-note">On Windows the verified installer runs and Aurora restarts as part of the update.</p>
    {/if}
  </div>

  <div class="update-domain">
    <div class="domain-heading">
      <span class="group-row-title">Aurora Client</span>
      <span class="domain-version">{overview?.clientInstanceName ?? "no instance selected"}</span>
    </div>
    {#if clientSummary}
      <p class="domain-line" class:tone-accent={clientSummary.tone === "accent"} class:tone-warn={clientSummary.tone === "warn"}>{clientSummary.title}</p>
      <p class="group-row-detail">{clientSummary.detail}</p>
    {/if}
    {#if clientNotes}
      <details class="notes">
        <summary>Release notes</summary>
        <pre class="notes-body">{clientNotes}</pre>
      </details>
    {/if}
    {#if clientOffer && overview?.clientInstanceId}
      <div class="domain-actions">
        <button type="button" class="btn" onclick={goToInstance}>Update in instance</button>
      </div>
    {/if}
  </div>
</section>

<style>
  .updates-surface { padding: 28px; }
  .channel-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  .channel-tile { position: relative; display: grid; gap: 4px; padding: 12px; border: 1px solid var(--f-edge); border-radius: 12px; background: rgb(8 15 25 / 24%); cursor: pointer; min-width: 0; }
  .channel-tile.chosen { border-color: var(--color-accent); }
  .channel-tile:focus-within { outline: 2px solid var(--color-accent); outline-offset: 3px; }
  .channel-tile input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .channel-name { text-transform: capitalize; font-weight: 600; font-size: 13px; }
  .channel-detail { color: var(--color-text-secondary); font-size: 11px; line-height: 1.5; }
  .channel-check { position: absolute; top: 10px; right: 10px; color: var(--color-accent); }
  .update-domain { border-top: 1px solid var(--f-edge); padding: 18px 0; display: grid; gap: 8px; }
  .domain-heading { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; }
  .domain-version { color: var(--color-text-secondary); font-size: 12px; }
  .domain-line { margin: 0; font-size: 14px; font-weight: 600; }
  .domain-line.tone-accent { color: var(--color-accent); }
  .domain-line.tone-warn { color: var(--color-text-secondary); }
  .domain-actions { display: flex; gap: 10px; flex-wrap: wrap; }
  .notes summary { cursor: pointer; font-size: 12px; color: var(--color-text-secondary); }
  .notes-body { margin: 10px 0 0; padding: 12px; border: 1px solid var(--f-edge); border-radius: 10px; background: rgb(8 15 25 / 32%); font-family: inherit; font-size: 12px; line-height: 1.6; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 260px; overflow-y: auto; }
  @media (max-width: 760px) { .updates-surface { padding: 18px; } .channel-grid { grid-template-columns: 1fr; } }
</style>
