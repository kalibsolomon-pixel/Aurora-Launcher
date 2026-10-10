<script lang="ts">
  import { onMount } from "svelte";
  import { ipcSeries } from "$lib/launcher/performance";
  import { launcher } from "$lib/launcher/store.svelte";
  import { updates } from "$lib/launcher/updates.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import AppShell from "$lib/shell/AppShell.svelte";
  import HomePage from "$lib/pages/HomePage.svelte";
  import InstancesPage from "$lib/pages/InstancesPage.svelte";
  import SettingsPage from "$lib/pages/SettingsPage.svelte";
  import CosmeticsPage from "$lib/pages/CosmeticsPage.svelte";
  import DeveloperPage from "$lib/pages/DeveloperPage.svelte";
  import InstanceWorkspace from "$lib/instances/InstanceWorkspace.svelte";

  // Development-only pipeline proofs; the destination is stripped from
  // production builds together with its page.
  const developerDestination = import.meta.env.DEV;

  const state = $derived(navigation.state);

  $effect(() => {
    // Selection changes invalidate only local presentation. A new network
    // check still requires the single startup call or explicit user action.
    if (launcher.launcherState) {
      void launcher.launcherState.config.selectedInstanceId;
      void updates.refresh();
    }
  });

  onMount(() => {
    launcher.initialize();
    ipcSeries();
    void updates.initialize();
    return () => {
      launcher.dispose();
      updates.dispose();
    };
  });
</script>

<svelte:head>
  <title>Aurora Launcher</title>
</svelte:head>

<AppShell>
  {#if state.kind === "instance"}
    <InstanceWorkspace instanceId={state.instanceId} tab={state.tab} />
  {:else if state.page === "home"}
    <HomePage />
  {:else if state.page === "instances"}
    <InstancesPage />
  {:else if state.page === "settings"}
    <SettingsPage />
  {:else if state.page === "cosmetics"}
    <CosmeticsPage />
  {:else if state.page === "developer" && developerDestination}
    <DeveloperPage />
  {/if}
</AppShell>
