<script lang="ts">
  import { launcher } from "$lib/launcher/store.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import { updates } from "$lib/launcher/updates.svelte";
  import { isUpdateAvailable } from "$lib/launcher/updatePresentation";
  import InstanceSwitcher from "$lib/launcher/InstanceSwitcher.svelte";
  import { mark } from "$lib/launcher/performance";
  import { homeLaunchState } from "$lib/launcher/home";
  import HomeWidgets from "$lib/launcher/HomeWidgets.svelte";
  import PlayerPreview from "$lib/launcher/PlayerPreview.svelte";

  const instance = $derived(launcher.selectedInstance);
  const readiness = $derived(
    launcher.playReadiness && launcher.playReadiness.instanceId === instance?.id &&
      launcher.playReadiness.accountId === (launcher.accountsState?.selectedAccountId ?? null)
      ? launcher.playReadiness
      : null,
  );
  const process = $derived(
    launcher.playProcess && launcher.playProcess.instanceId === instance?.id
      ? launcher.playProcess
      : null,
  );

  const launch = $derived(homeLaunchState(instance?.id ?? null, readiness, process,
    launcher.playBusy, launcher.playReadinessBusy, launcher.instanceBusy !== null || launcher.accountBusy !== null));

  const updateNotice = $derived(isUpdateAvailable(updates.overview?.launcher.availability) ||
    (updates.overview?.clientInstanceId === instance?.id && isUpdateAvailable(updates.overview?.client)));
  if (import.meta.env.MODE === "performance-p0-2") {
    let previousLabel: string | undefined;
    $effect(() => {
      if (launch.label === previousLabel) return;
      previousLabel = launch.label;
      if (launch.label === "Ready") mark("ready");
      else if (launch.label === "Checking…") mark("checking");
    });
  }
</script>

<!--
  Aurora's fast launch surface: the selected instance and everything Play
  needs, with the readiness decision Rust owns rendered verbatim. The full
  per-instance workspace lives behind Instances — Home stays a launch
  surface; detailed readiness and maintenance stay in the workspace Overview.
-->
<div class="page home-page f-pilot" aria-label="Home">
  {#if updateNotice}
    <button type="button" class="btn btn-quiet aurora-update-notice" onclick={() => navigation.openUpdates()}>Aurora update available</button>
  {/if}
  {#if launcher.stateError}
    <section class="group" aria-live="polite">
      <div class="group-heading">
        <div>
          <h3 class="group-title">Launcher state</h3>
          <p class="group-subtitle">Persisted launcher state could not be loaded.</p>
        </div>
        <span class="status-badge status-error">Error</span>
      </div>
      <p class="inline-message inline-message-error group-row" role="alert">
        {launcher.stateError.message}
        <code>{launcher.stateError.code}</code>
      </p>
    </section>
  {:else if !launcher.launcherState}
    <section class="group" aria-live="polite">
      <div class="group-row group-row-loading">
        <span class="spinner" aria-hidden="true"></span>
        <p class="group-row-detail">Checking saved installation state and recovering interrupted installations…</p>
      </div>
    </section>
  {:else if !instance}
    {#if launcher.launcherState.instances.length === 0}
      <section class="empty-state" aria-live="polite">
        <h3 class="empty-title">No instances yet</h3>
        <p class="empty-detail">
          Create an isolated Minecraft installation to get started.
        </p>
        <button type="button" class="btn btn-primary" onclick={() => navigation.goTo("instances")}>
          Create instance
        </button>
      </section>
    {:else}
      <section class="empty-state" aria-live="polite">
        <h3 class="empty-title">No instance selected</h3>
        <p class="empty-detail">Choose an instance to launch from.</p>
        <InstanceSwitcher />
      </section>
    {/if}
  {:else}
    {#if launcher.instanceError}
      <p class="inline-message inline-message-error" role="alert">{launcher.instanceError.message}<code>{launcher.instanceError.code}</code></p>
    {/if}
    <div class="home-composition">
    <div class="home-launch-column">
    <section class="instance-card" aria-label="Selected instance" aria-live="polite">
      <InstanceSwitcher compact status={{ ready: launch.tone === "status-success", label: launch.label }}
        launchAction={{ label: launch.playLabel, disabled: launch.disabled, run: () => launcher.runPlay(instance.id) }} />
    </section>
    {#if launch.blockers.length || launcher.playError || launch.failure || launch.exitDetail}
    <section class="group f-surface launch-feedback" aria-label="Launch details" aria-live="polite">
      {#if launch.blockers.length || launcher.playError || launch.failure}
        <div class="group-row">
          <div class="group-row-main">
            {#each launch.blockers as blocker (blocker.code)}
              <p class="group-row-detail">{blocker.message}</p>
              <details><summary>Technical details</summary><code>{blocker.code}</code></details>
            {/each}
            {#if launcher.playError}
              <p class="inline-message inline-message-error" role="alert">{launcher.playError.message}</p>
              <details><summary>Technical details</summary><code>{launcher.playError.code}</code></details>
            {:else if launch.failure}<p class="inline-message inline-message-error" role="alert">{launch.failure}</p>{/if}
          </div>
          <button type="button" class="btn" onclick={() => launcher.refreshPlayReadiness()} disabled={launcher.playBusy || launcher.playReadinessBusy}>Check again</button>
        </div>
      {/if}
      {#if launch.exitDetail}<p class="group-footer">{launch.exitDetail}</p>{/if}
    </section>
    {/if}
    </div>
    <PlayerPreview />
    </div>
  {/if}
  <HomeWidgets />
  {#if launcher.playProcess && launcher.playProcess.instanceId !== instance?.id && (launcher.playProcess.status === "starting" || launcher.playProcess.status === "running")}
    <p class="group-footer" role="status">{launcher.launcherState?.instances.find(item => item.id === launcher.playProcess?.instanceId)?.displayName ?? "Another instance"} is {launcher.playProcess.status === "running" ? "running" : "starting"}.</p>
  {/if}
</div>

<style>
  .home-page { max-width: 1280px; margin-inline: auto; width: 100%; }
  .aurora-update-notice { margin-bottom: 16px; }
  .home-composition { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-5); align-items: center; min-height: min(52vh, 480px); padding: var(--space-4) 0 var(--space-5); }
  .home-launch-column { display: flex; flex-direction: column; align-items: center; gap: var(--space-5); min-width: 0; }
  .instance-card { width: min(100%, 320px); aspect-ratio: 4 / 1; min-height: 4rem; min-width: 0; margin: 0; position: relative; z-index: 2; }
  .launch-feedback { width: 100%; min-width: 0; margin: 0; overflow-wrap: anywhere; }
  @media (max-width: 800px) { .home-composition { grid-template-columns: minmax(0, 1fr); min-height: 0; padding: 0 0 var(--space-4); } .home-launch-column { gap: var(--space-3); } }
</style>
