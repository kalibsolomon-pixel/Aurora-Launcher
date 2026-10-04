<script lang="ts">
  import { accountManager } from "$lib/launcher/accountManager.svelte";
  // Existing management content is retained here; it has no navigation route.
  import AccountManager from "$lib/pages/AccountsPage.svelte";
  import Icon from './Icon.svelte';
  let dialog: HTMLDialogElement | undefined = $state();
  let returnFocus: HTMLElement | null = null;
  let backdropPressed = false;
  function outside(event: MouseEvent | PointerEvent): boolean {
    if (event.target !== dialog || !dialog) return false;
    const bounds = dialog.getBoundingClientRect();
    return event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom;
  }
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

<!-- Native dialog retains focus containment, background inertness and Escape. -->
<dialog bind:this={dialog} class="account-drawer" aria-labelledby="account-dialog-title" onclose={closed}
  onpointerdown={event => backdropPressed = outside(event)} onclick={event => { if (backdropPressed && outside(event)) accountManager.close(); }}>
  <div class="drawer-toolbar"><button type="button" class="f-icon-button dialog-close" aria-label="Close accounts" onclick={() => accountManager.close()}><Icon name="close" size={22} /></button></div>
  <div class="drawer-body">{#if accountManager.open}<AccountManager />{/if}</div>
</dialog>

<style>
  dialog { position: fixed; inset: 40px auto 0 0; margin: 0; box-sizing: border-box; width: min(420px, 100vw); height: calc(100dvh - 40px); max-width: 100vw; max-height: none; padding: 0; border: 1px solid var(--f-edge); border-left: 0; border-bottom: 0; border-radius: 0 var(--f-radius-panel) 0 0; background: var(--f-dialog-panel); color: var(--color-text); box-shadow: var(--f-shadow); backdrop-filter: var(--f-blur); overflow: hidden; }
  dialog[open] { display: flex; flex-direction: column; animation: drawer-enter var(--f-duration) var(--f-ease); }
  dialog::backdrop { background: var(--f-dialog-dim); }
  dialog :global(.group) { background: var(--f-dialog-group); border-color: var(--f-edge); box-shadow: none; }
  .drawer-toolbar { display: flex; justify-content: flex-end; padding: 12px 16px 8px; flex: none; }
  .drawer-body { min-height: 0; overflow-y: auto; overscroll-behavior: contain; padding: 0 20px 24px; }
  .drawer-body :global(.page-header) { flex-direction: column; align-items: flex-start; gap: var(--space-4); }
  .drawer-body :global(.group-row-actions) { width: 100%; }
  @keyframes drawer-enter { from { transform: translateX(-24px); opacity: .8; } to { transform: translateX(0); opacity: 1; } }
  @media (max-width: 600px) { dialog { width: 100vw; border-radius: 0; } }
  @media (prefers-reduced-motion: reduce) { dialog[open] { animation: none; } }
</style>
