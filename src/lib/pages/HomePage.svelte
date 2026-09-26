<script lang="ts">
  import { installedConfigurationLabel } from "$lib/launcher/instanceStatus";
  import { launcher } from "$lib/launcher/store.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import InstanceSwitcher from "$lib/launcher/InstanceSwitcher.svelte";
  import { homeLaunchState } from "$lib/launcher/home";
  import AccountIdentity from "$lib/launcher/AccountIdentity.svelte";

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
<div class="page">
  <header class="page-header">
    <div>
      <h2 class="page-title">Home</h2>
      <p class="page-subtitle">Choose your instance and play Minecraft.</p>
    </div>
  </header>

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
    <InstanceSwitcher />
    {#if launcher.instanceError}
      <p class="inline-message inline-message-error" role="alert">{launcher.instanceError.message}<code>{launcher.instanceError.code}</code></p>
    {/if}
    <section class="group" aria-label="Selected instance" aria-live="polite">
      <div class="group-heading">
        <div class="instance-heading">
          <h3 class="instance-name">{instance.displayName}</h3>
          <p class="instance-versions">
            {installedConfigurationLabel(instance)}
          </p>
          <span class="status-badge {launch.tone}">{launch.label}</span>
        </div>
        <div class="play-actions">
          <button
            type="button"
            class="btn btn-primary"
            onclick={() => launcher.runPlay(instance.id)}
            disabled={launch.disabled}
          >
            {launch.playLabel}
          </button>
          <button
            type="button"
            class="btn btn-quiet"
            onclick={() => navigation.openInstance(instance.id)}
          >
            Manage Instance
          </button>
        </div>
      </div>

      <div class="group-row">
        <AccountIdentity account={launcher.selectedAccount} />
        <button type="button" class="btn btn-quiet" onclick={() => navigation.goTo("accounts")}>{launcher.selectedAccount ? "Manage accounts" : "Sign in"}</button>
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
  {/if}
</div>

<style>
  .instance-heading {
    min-width: 0;
    flex: 1 1 240px;
  }

  .instance-name {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 600;
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
  }

  .instance-versions {
    margin: var(--space-1) 0 0;
    color: var(--color-text-secondary);
    font-size: var(--text-metadata);
    overflow-wrap: anywhere;
    margin-bottom: var(--space-3);
  }

  .play-actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-wrap: wrap;
    justify-content: flex-start;
  }
  .play-actions .btn-primary { min-width: 104px; }
  .group-heading { flex-wrap: wrap; }
</style>
