<script lang="ts">
  import { accountManager } from "$lib/launcher/accountManager.svelte";
  // Existing management content is retained here; it has no navigation route.
  import AccountManager from "$lib/pages/AccountsPage.svelte";
  let dialog: HTMLDialogElement | undefined = $state();
  let returnFocus: HTMLElement | null = null;
  $effect(() => {
    if (!dialog) return;
    if (accountManager.open && !dialog.open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      dialog.showModal();
    } else if (!accountManager.open && dialog.open) dialog.close();
  });
  function closed() {
    accountManager.close();
    if (returnFocus?.isConnected) returnFocus.focus();
    returnFocus = null;
  }
</script>

<!-- Native modal dialog supplies focus containment, background inertness and Escape. -->
<dialog bind:this={dialog} aria-labelledby="account-dialog-title" onclose={closed}>
  <button type="button" class="btn btn-quiet dialog-close" aria-label="Close accounts" onclick={() => accountManager.close()}>Close</button>
  {#if accountManager.open}<AccountManager />{/if}
</dialog>

<style>
  dialog { box-sizing: border-box; width: min(680px, calc(100vw - 32px)); max-height: calc(100dvh - 48px); padding: var(--space-5); border: 1px solid var(--color-border); border-radius: var(--radius-lg); background: var(--color-background); color: var(--color-text); overflow: auto; }
  dialog::backdrop { background: rgb(0 0 0 / 65%); }
  .dialog-close { display: block; margin-left: auto; margin-bottom: var(--space-3); }
  @media (max-width: 760px) { dialog { padding: var(--space-3); } }
</style>
