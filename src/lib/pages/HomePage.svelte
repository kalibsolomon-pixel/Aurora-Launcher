<script lang="ts">
  import { installedConfigurationLabel } from "$lib/launcher/instanceStatus";
  import { launcher } from "$lib/launcher/store.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import InstanceSwitcher from "$lib/launcher/InstanceSwitcher.svelte";
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
</script>

<!--
  Aurora's fast launch surface: the selected instance and everything Play
  needs, with the readiness decision Rust owns rendered verbatim. The full
  per-instance workspace lives behind Instances — Home stays a launch
  surface; detailed readiness and maintenance stay in the workspace Overview.
-->
<div class="page home-page f-pilot" aria-label="Home">
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
        <p class="group-row-detail">Loading persisted launcher state…</p>
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
    <img class="hero-logo" src="/aurora-icon.png" alt="Aurora" draggable="false" />
    <section class="group f-surface instance-card" aria-label="Selected instance" aria-live="polite">
      <div class="launch-card">
        <div class="launch-main">
        <button type="button" class="btn btn-primary play-hero" onclick={() => launcher.runPlay(instance.id)} disabled={launch.disabled}>{launch.playLabel}</button>
        <div class="instance-heading">
          <InstanceSwitcher compact />
          <p class="instance-versions">{installedConfigurationLabel(instance)}</p>
          <span class="status-badge {launch.tone}">{launch.label}</span>
        </div>
        </div>
      </div>
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
  .home-composition { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-4); align-items: center; min-height: min(56vh, 660px); padding: 8px 0 28px; }
  .home-launch-column { display: flex; flex-direction: column; align-items: center; gap: 22px; min-width: 0; }
  .hero-logo { width: clamp(200px, 22vw, 280px); height: auto; }
  .instance-card { width: 100%; max-width: none; min-width: 0; overflow: visible; margin: 0; position: relative; z-index: 2; }
  .launch-main { padding: 12px; display: grid; gap: 10px; min-width: 0; }
  .play-hero { width: 100%; min-height: 78px; font-size: 30px; font-weight: 700; letter-spacing: -.03em; border-radius: 15px; box-shadow: inset 0 1px 0 rgb(255 255 255 / 16%), 0 5px 16px rgb(0 0 0 / 15%); }
  .instance-heading { min-width: 0; padding: 4px 14px 12px; text-align: center; }
  .instance-heading :global(.picker-trigger) { font-size: 16px; font-weight: 550; margin: 0; padding: 12px; }
  .instance-heading :global(.picker-menu) { text-align: left; }
  .instance-versions { margin: 2px 0 12px; color: var(--color-text-secondary); font-size: 12px; overflow-wrap: anywhere; }
  @media (min-width: 1500px) { .home-composition { min-height: 660px; } }
  @media (max-width: 900px) { .home-composition { grid-template-columns: minmax(0, 1fr); min-height: 0; padding: 0 0 16px; } .hero-logo { width: 168px; } .home-launch-column { gap: 12px; } .instance-card { max-width: none; } .play-hero { min-height: 68px; font-size: 28px; } }
</style>
