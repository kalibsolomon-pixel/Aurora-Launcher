<script lang="ts">
  import "./phase-f.css";
  import Icon from "./Icon.svelte";
  import TitleBar from "./TitleBar.svelte";
  import Background from "./Background.svelte";
  import { launcher } from "$lib/launcher/store.svelte";
  import { navigation } from "$lib/launcher/navigation.svelte";
  import { activeGlobalPage, globalDestinations } from "$lib/launcher/navigation";
  import AccountDialog from "$lib/shell/AccountDialog.svelte";
  import { accountManager } from "$lib/launcher/accountManager.svelte";
  import AccountIdentity from "$lib/launcher/AccountIdentity.svelte";

  let { children }: { children: import("svelte").Snippet } = $props();

  // The Developer destination is stripped from production builds together
  // with its page; the list is static per bundle.
  const destinations = globalDestinations(import.meta.env.DEV);

  const state = $derived(navigation.state);
  const currentPage = $derived(state.kind === "instance" ? "instances" : activeGlobalPage(state));
  const account = $derived(launcher.selectedAccount);
</script>

<div class="aurora-shell">
<Background />
<TitleBar />
<div class="app-frame">
  <nav class="sidebar" aria-label="Aurora Launcher">
    <img class="rail-brand" src="/aurora-icon.png" alt="Aurora" />
    <div class="rail-navigation">
      {#each destinations as destination (destination.id)}
        <button type="button" class="rail-link" aria-label={destination.label}
          aria-current={currentPage === destination.id ? "page" : undefined}
          onclick={() => navigation.goTo(destination.id)}>
          <Icon name={destination.id} />
          <span class="f-tooltip" aria-hidden="true">{destination.label}</span>
        </button>
      {/each}
    </div>
    <div class="sidebar-spacer"></div>

    <button
      type="button"
      class="account-chip"
      onclick={() => accountManager.show()}
      aria-haspopup="dialog"
      aria-label={account
        ? `Accounts — signed in as ${account.minecraftName}`
        : "Accounts — not signed in"}
    >
      <AccountIdentity {account} small />
    </button>

    <p class="sidebar-version">
      {launcher.status ? `v${launcher.status.launcherVersion}` : "Aurora Launcher"}
    </p>
  </nav>

  <main class="content">
    {@render children()}
  </main>
</div>

</div>
<AccountDialog />
