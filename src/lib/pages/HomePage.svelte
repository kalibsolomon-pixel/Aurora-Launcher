<script lang="ts">
  import { onMount } from "svelte";
  import { launcher } from "$lib/launcher/store.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import { accountManager } from "$lib/launcher/accountManager.svelte";
  import { updates } from "$lib/launcher/updates.svelte";
  import { isUpdateAvailable } from "$lib/launcher/updatePresentation";
  import InstanceSwitcher from "$lib/launcher/InstanceSwitcher.svelte";
  import { mark } from "$lib/launcher/performance";
  import { homeLaunchState } from "$lib/launcher/home";
  import HomeWidgets from "$lib/launcher/HomeWidgets.svelte";
  import PlayerPreview from "$lib/launcher/PlayerPreview.svelte";
  import InstanceIcon from "$lib/instances/InstanceIcon.svelte";
  import { instanceIcons } from "$lib/instances/instanceIcons.svelte";
  // One small read after Home mounts; icons are presentation and never gate launch.
  onMount(() => { void instanceIcons.ensureLoaded(); });

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

  // The single next step for a Rust-reported blocker, reusing existing
  // commands and destinations. Unknown codes offer only "Check again".
  function blockerAction(code: string): { label: string; disabled: boolean; run: () => void } | null {
    if (!instance) return null;
    const id = instance.id;
    if (code === "launch_runtime_not_ready") return { label: launcher.runtimeBusy ? "Installing Java…" : "Install Java", disabled: launcher.runtimeBusy || launcher.playBusy, run: () => void launcher.runEnsureRuntime(id) };
    if (code === "launch_authentication_required") return { label: "Accounts", disabled: false, run: () => accountManager.show() };
    if (code === "launch_instance_not_ready" || code === "launch_instance_damaged") return { label: "Open instance", disabled: false, run: () => navigation.openInstance(id) };
    return null;
  }

  const updateNotice = $derived(isUpdateAvailable(updates.overview?.launcher.availability) ||
    (updates.overview?.clientInstanceId === instance?.id && isUpdateAvailable(updates.overview?.client)));
  if (import.meta.env.MODE === "performance-p0-2") {
    onMount(() => {
      let second = 0;
      const first = requestAnimationFrame(() => { second = requestAnimationFrame(() => mark("homePaint")); });
      return () => { cancelAnimationFrame(first); cancelAnimationFrame(second); };
    });
    $effect(() => {
      // Initial Home paint can precede restored state. This separate marker
      // requires both native instance/config and account discovery to finish.
      if (!launcher.launcherState || !launcher.accountsState || launcher.stateError || launcher.accountsError) return;
      let second = 0;
      const first = requestAnimationFrame(() => { second = requestAnimationFrame(() => mark("homeUsable")); });
      return () => { cancelAnimationFrame(first); cancelAnimationFrame(second); };
    });
    let previousLabel: string | undefined;
    $effect(() => {
      if (launch.label === previousLabel) return;
      previousLabel = launch.label;
      if (launch.label === "Ready") {
        mark("ready");
        let second = 0;
        const first = requestAnimationFrame(() => {
          second = requestAnimationFrame(() => mark("readyPaint"));
        });
        return () => { cancelAnimationFrame(first); cancelAnimationFrame(second); };
      }
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
    <!-- The empty launch states keep Home's launch surface, so first run has
         the same composition (one primary panel, widgets below) as daily use. -->
    {#if launcher.launcherState.instances.length === 0}
      <section class="home-composition home-empty" aria-live="polite">
        <div class="empty-state">
          <h3 class="empty-title">No instances yet</h3>
          <p class="empty-detail">
            Create an isolated Minecraft installation to get started.
          </p>
          <button type="button" class="btn btn-primary" onclick={() => navigation.goTo("instances")}>
            Create instance
          </button>
        </div>
      </section>
    {:else}
      <section class="home-composition home-empty" aria-live="polite">
        <div class="empty-state">
          <h3 class="empty-title">No instance selected</h3>
          <p class="empty-detail">Choose an instance to launch from.</p>
          <div class="empty-switcher"><InstanceSwitcher /></div>
        </div>
      </section>
    {/if}
  {:else}
    {#if launcher.instanceError}
      <p class="inline-message inline-message-error" role="alert">{launcher.instanceError.message}<code>{launcher.instanceError.code}</code></p>
    {/if}
    <div class="home-composition" aria-label="Launch workspace">
    <div class="home-launch-column">
    <header class="instance-heading">
      <InstanceIcon icon={instanceIcons.iconFor(instance.id)} size={26} tile />
      <h1 title={instance.displayName}>{instance.displayName}</h1>
    </header>
    <section class="instance-card" aria-label="Selected instance" aria-live="polite">
      <InstanceSwitcher compact status={{ ready: launch.tone === "status-success", label: launch.label }}
        launchAction={{ label: launch.playLabel, disabled: launch.disabled, run: () => launcher.runPlay(instance.id) }} />
    </section>
    <!-- Ready stays the quiet dot inside Launch (owner decision, df5d9da);
         Starting/Running are already the button's own label. Any state that
         blocks or delays Play is also stated in words, never color alone. -->
    {#if launch.label !== "Ready" && launch.playLabel === "Launch"}<p class="launch-readiness {launch.tone}" aria-hidden="true">{launch.label}</p>{/if}
    </div>
    <PlayerPreview />
    {#if launch.blockers.length || launcher.playError || launch.failure || launch.exitDetail}
    <section class="launch-feedback" aria-label="Launch details" aria-live="polite">
      {#if launch.blockers.length || launcher.playError || launch.failure}
        <div class="group-row">
          <div class="group-row-main">
            {#if launch.blockers.length}
              <ul class="blocker-list">
                {#each launch.blockers as blocker, index (`${blocker.code}-${index}`)}
                  {@const action = blockerAction(blocker.code)}
                  <li>
                    <span class="group-row-detail">{blocker.message}</span>
                    {#if action}<button type="button" class="btn btn-quiet" disabled={action.disabled} onclick={action.run}>{action.label}</button>{/if}
                  </li>
                {/each}
              </ul>
              <details><summary>Technical details</summary>{#each launch.blockers as blocker, index (`${blocker.code}-${index}`)}<code>{blocker.code}</code> {/each}</details>
            {/if}
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
  {/if}
  <HomeWidgets />
  {#if launcher.playProcess && launcher.playProcess.instanceId !== instance?.id && (launcher.playProcess.status === "starting" || launcher.playProcess.status === "running")}
    <p class="group-footer" role="status">{launcher.launcherState?.instances.find(item => item.id === launcher.playProcess?.instanceId)?.displayName ?? "Another instance"} is {launcher.playProcess.status === "running" ? "running" : "starting"}.</p>
  {/if}
</div>

<style>
  .home-page { max-width: 1280px; margin-inline: auto; width: 100%; }
  .aurora-update-notice { margin-bottom: 16px; }
  .home-composition { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-5); align-items: center; padding: var(--space-5); border: 1px solid var(--f-edge); border-radius: var(--f-radius-panel); background: radial-gradient(ellipse at 85% 45%, color-mix(in srgb, var(--color-accent) 10%, transparent), transparent 65%), var(--f-panel); box-shadow: var(--f-shadow); position: relative; }
  .home-composition { min-height: clamp(320px, 52vh, 404px); }
  .home-composition:has(.launch-feedback) :global(.player-preview.has-skin) { display: none; }
  .home-launch-column { display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-5); min-width: 0; padding-block: var(--space-3); }
  .instance-heading { display: flex; align-items: center; gap: var(--space-4); min-width: 0; width: 100%; }
  .instance-heading h1 { min-width: 0; }
  .instance-heading h1 { margin: 0; font-size: 2rem; line-height: 1.2; font-weight: 650; letter-spacing: -.035em; overflow-wrap: anywhere; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; overflow: hidden; }
  /* Launch carries only the verb and readiness dot; the name is the heading above. */
  .instance-card { width: min(100%, 300px); height: 80px; min-height: 4rem; min-width: 0; margin: 0; position: relative; z-index: 2; }
  .home-composition.home-empty { grid-template-columns: minmax(0, 1fr); min-height: 0; align-items: start; }
  .home-empty .empty-state { padding: var(--space-3) var(--space-2); }
  .home-empty .empty-title { font-size: 1.375rem; letter-spacing: -.02em; }
  .empty-switcher { width: min(100%, 360px); }
  .launch-readiness { display: inline-flex; align-items: center; gap: var(--space-2); margin: calc(var(--space-2) * -1) 0 0; padding: 0; background: none; font-size: var(--text-secondary); font-weight: 500; }
  .launch-readiness::before { content: ''; width: 8px; height: 8px; border-radius: 50%; background: currentColor; }
  .blocker-list { display: grid; gap: var(--space-2); margin: 0 0 var(--space-2); padding: 0; list-style: none; }
  .blocker-list li { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex-wrap: wrap; min-height: 36px; }
  .blocker-list .btn { padding-block: 6px; }
  .launch-feedback details code { margin-right: var(--space-2); }
  .launch-feedback { grid-column: 1 / -1; width: 100%; min-width: 0; margin: 0; border-top: 1px solid var(--f-edge); overflow-wrap: anywhere; }
  .launch-feedback :global(.group-row), .launch-feedback :global(.group-footer) { padding: var(--space-4) 0 0; }
  @media (max-width: 800px) { .home-composition { grid-template-columns: minmax(0, 1fr); min-height: 0; padding: var(--space-4); } .home-launch-column { gap: var(--space-4); padding-block: var(--space-2); } }
</style>
