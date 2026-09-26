<script lang="ts">
  import { launcher } from "$lib/launcher/store.svelte";
  import { authErrorMessage, authPhaseLabel } from "$lib/authMessages";
  import AccountIdentity from "$lib/launcher/AccountIdentity.svelte";
  import { tick } from "svelte";

  const accounts = $derived(launcher.accountsState?.accounts ?? []);
  const signedIn = $derived(accounts.length > 0);

  let removing = $state<string | null>(null);
  let confirmButton: HTMLButtonElement | undefined = $state();
  async function confirmRemoval(id: string): Promise<void> {
    removing = id;
    await tick();
    confirmButton?.focus();
  }
  async function cancelRemoval(): Promise<void> {
    const id = removing;
    removing = null;
    await tick();
    document.getElementById(`remove-account-${id}`)?.focus();
  }
</script>

<!--
  Shell dialog contents (the historical filename is retained to preserve the tracked file): one obvious primary task when signed out,
  restrained selected/unselected rows when signed in. The UI speaks in
  actionable language — internal error codes never appear here (see
  authMessages.ts), and tokens never leave Rust in the first place.
-->
<div class="account-manager">
  <header class="page-header">
    <div>
      <h2 class="page-title" id="account-dialog-title">Accounts</h2>
      <p class="page-subtitle">The active account is used across your instances.</p>
    </div>
    {#if signedIn && !launcher.signInBusy}
      <div class="page-header-actions">
        <button
          type="button"
          class="btn btn-primary"
          onclick={() => launcher.runSignIn()}
          disabled={launcher.accountsState === null || launcher.accountBusy !== null}
        >
          Add account
        </button>
      </div>
    {/if}
  </header>

  {#if launcher.accountsError}
    <section class="group" aria-live="polite">
      <div class="group-heading">
        <div>
          <h3 class="group-title">Accounts could not be loaded</h3>
          <p class="group-subtitle">The persisted account list could not be read.</p>
        </div>
      </div>
      <p class="inline-message inline-message-error group-row" role="alert">
        {launcher.accountsError.message}
      </p>
    </section>
  {/if}

  {#if launcher.signInBusy}
    <section class="group" aria-live="polite">
      <div class="group-heading">
        <div>
          <h3 class="group-title">Signing in with Microsoft</h3>
          <p class="group-subtitle">
            {launcher.signInProgress
              ? authPhaseLabel(launcher.signInProgress.phase)
              : "Opening the Microsoft sign-in in your browser…"}
          </p>
        </div>
        <span class="status-badge status-working">In progress</span>
      </div>
      <div class="group-row group-row-loading">
        <span class="spinner" aria-hidden="true"></span>
        <span class="group-row-detail">
          Finish the sign-in in your browser, then return here. The sign-in happens entirely in
          your browser — Aurora never asks for your password.
        </span>
      </div>
      <div class="group-row">
        <span class="group-row-detail">You can cancel if you changed your mind.</span>
        <div class="group-row-actions">
          <button type="button" class="btn" onclick={() => launcher.runCancelSignIn()}>
            Cancel sign-in
          </button>
        </div>
      </div>
      {#if launcher.signInError}
        <p class="inline-message inline-message-error group-row" role="alert">
          {authErrorMessage(launcher.signInError)}
        </p>
      {/if}
    </section>
  {:else if launcher.accountsState === null && !launcher.accountsError}
    <section class="group" aria-live="polite">
      <div class="group-row group-row-loading">
        <span class="spinner" aria-hidden="true"></span>
        <span class="group-row-detail">Loading accounts…</span>
      </div>
    </section>
  {:else if !signedIn}
    <section class="empty-state" aria-live="polite">
      <h3 class="empty-title">No account signed in</h3>
      <p class="empty-detail">
        Sign in with Microsoft to play Minecraft: Java Edition. You can still browse and manage your instances while signed out.
      </p>
      <button
        type="button"
        class="btn btn-primary"
        onclick={() => launcher.runSignIn()}
        disabled={launcher.accountsState === null}
      >
        Sign in with Microsoft
      </button>
      {#if launcher.signInError}
        <p class="inline-message inline-message-error" role="alert">
          {authErrorMessage(launcher.signInError)}
        </p>
      {/if}
      {#if launcher.accountError}
        <p class="inline-message inline-message-error" role="alert">
          {authErrorMessage(launcher.accountError)}
        </p>
      {/if}
    </section>
  {:else}
    <section class="group" aria-label="Active Minecraft account">
      <div class="group-heading">
        <div><h3 class="group-title">Active account</h3></div>
        <span class="status-badge {launcher.selectedAccount?.status === 'signedIn' ? 'status-success' : 'status-warning'}">{launcher.selectedAccount?.status === 'signedIn' ? 'Signed in' : launcher.selectedAccount ? 'Sign-in required' : 'No active account'}</span>
      </div>
      <div class="group-row">
        <AccountIdentity account={launcher.selectedAccount} />
        {#if launcher.selectedAccount?.status === "signedIn"}
          <button type="button" class="btn btn-quiet" onclick={() => launcher.refreshAvatar(launcher.selectedAccount!.accountId, true)} disabled={launcher.avatarBusy !== null || launcher.accountBusy !== null}>Refresh avatar</button>
        {/if}
      </div>
      {#if launcher.selectedAccount && launcher.accountAvatars[launcher.selectedAccount.accountId] === null}<p class="group-footer">Avatar unavailable. Your account and Play are unaffected.</p>{/if}
    </section>
    <section class="group" aria-live="polite">
      <div class="group-heading">
        <div>
          <h3 class="group-title">Your accounts</h3>

        </div>
      </div>

      {#each accounts as account (account.accountId)}
        {@const selected = launcher.accountsState?.selectedAccountId === account.accountId}
        {@const busy = launcher.accountBusy === account.accountId}
        <div class="group-row" class:group-row-selected={selected}>
          <div class="group-row-main">
            <AccountIdentity {account} small />
            {#if selected}<span class="row-marker">Active</span>{/if}
            {#if account.status === "reauthenticationRequired"}
              <span class="group-row-detail">
                This account's stored credential is no longer valid — sign in again to use it.
              </span>
            {/if}
          </div>
          <div class="group-row-actions">
            {#if account.status === "reauthenticationRequired"}
              <span class="status-badge status-warning">Sign-in required</span>
            {/if}
            {#if !selected}
              <button
                type="button"
                class="btn"
                onclick={() => launcher.runSelectAccount(account.accountId)}
                disabled={launcher.accountBusy !== null || launcher.signInBusy}
              >
                Use account
              </button>
            {/if}
            <button
              type="button"
              class="btn"
              onclick={() => launcher.runRefreshAccountSession(account.accountId)}
              disabled={launcher.accountBusy !== null || launcher.signInBusy}
            >
              {busy ? "Checking…" : account.status === "reauthenticationRequired" ? "Check sign-in" : "Check account"}
            </button>
            <button
              type="button"
              class="btn btn-danger"
              id="remove-account-{account.accountId}"
              onclick={() => confirmRemoval(account.accountId)}
              disabled={launcher.accountBusy !== null || launcher.signInBusy}
            >
              Remove
            </button>
          </div>
          {#if removing === account.accountId}
            <div class="remove-confirmation" role="group" aria-label="Confirm account removal">
              <p>Remove {account.minecraftName} from this launcher? You will need to sign in again to use it here.</p>
              <button type="button" class="btn" onclick={cancelRemoval} onkeydown={(event) => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); void cancelRemoval(); } }}>Cancel</button>
              <button bind:this={confirmButton} type="button" class="btn btn-danger" disabled={launcher.accountBusy !== null} onkeydown={(event) => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); void cancelRemoval(); } }} onclick={async () => { await launcher.runRemoveAccount(account.accountId); removing = null; }}>Remove account</button>
            </div>
          {/if}
        </div>
      {/each}

      {#if launcher.signInError}
        <p class="inline-message inline-message-error group-row" role="alert">
          {authErrorMessage(launcher.signInError)}
        </p>
      {/if}
      {#if launcher.accountError}
        <p class="inline-message inline-message-error group-row" role="alert">
          {authErrorMessage(launcher.accountError)}
        </p>
        <details class="group-row"><summary>Technical details</summary><code>{launcher.accountError.code}</code></details>
      {/if}

      <p class="group-footer">
        Removal signs the account out of this launcher. Your Microsoft account and Minecraft data stay intact.
      </p>
    </section>
  {/if}
</div>

<style>
  .account-manager { display: grid; gap: var(--space-4); }
  .account-manager .page-header { margin-bottom: 0; }
  .account-manager .group { margin: 0; }
  .account-manager .group-heading { padding: var(--space-3); }
  .account-manager .group-row { padding: var(--space-3); }
  .account-manager .group-footer { padding: var(--space-3); }
  .remove-confirmation { flex-basis: 100%; font-size: var(--text-secondary); }
  .remove-confirmation .btn { margin-right: var(--space-2); }
</style>
