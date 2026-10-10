<script lang="ts">
  import { tick } from "svelte";
  import { launcher } from "$lib/launcher/store.svelte";
  let dialog: HTMLDialogElement;
  let input: HTMLInputElement;
  let returnFocus: HTMLElement | null = null;
  const name = $derived(launcher.renaming?.name.trim() ?? "");
  const invalid = $derived(!name ? "Enter an instance name." : new TextEncoder().encode(name).length > 80 ? "Use at most 80 UTF-8 bytes for the name." : /[\u0000-\u001f\u007f-\u009f]/.test(name) ? "Control characters are not allowed." : "");
  $effect(() => {
    if (launcher.renaming && dialog && !dialog.open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      dialog.showModal();
      void tick().then(() => { input?.focus(); input?.select(); });
    } else if (!launcher.renaming && dialog?.open) dialog.close();
  });
  function cancel() { if (!launcher.renameBusy) launcher.renaming = null; }
</script>

<dialog bind:this={dialog} class="rename-dialog" aria-labelledby="rename-title" oncancel={event => { event.preventDefault(); cancel(); }} onclose={() => returnFocus?.focus()}>
  <form onsubmit={event => { event.preventDefault(); if (!invalid) void launcher.runRename(); }}>
    <h3 id="rename-title">Rename instance</h3>
    <p class="field-hint">Choose the name shown throughout Aurora.</p>
    <label class="field"><span class="field-label">Instance name</span><input bind:this={input} value={launcher.renaming?.name ?? ""} oninput={event => { if (launcher.renaming) launcher.renaming.name = event.currentTarget.value; launcher.renameError = null; }} disabled={launcher.renameBusy} maxlength="80" autocomplete="off" aria-describedby="rename-hint" /></label>
    <p id="rename-hint" class="field-hint">{invalid || "Names can be shared by different instances. Maximum 80 UTF-8 bytes."}</p>
    {#if launcher.renameError}<p class="inline-message inline-message-error" role="alert">{launcher.renameError.message}</p>{/if}
    <div class="actions"><button type="button" class="btn btn-quiet" disabled={launcher.renameBusy} onclick={cancel}>Cancel</button><button type="submit" class="btn btn-primary" disabled={launcher.renameBusy || !!invalid}>{launcher.renameBusy ? "Saving…" : "Save name"}</button></div>
  </form>
</dialog>

<style>
  .rename-dialog { width: min(440px, calc(100vw - 48px)); padding: var(--space-5); border: 1px solid var(--color-border-strong); border-radius: var(--radius-lg); background: var(--color-surface-raised); color: var(--color-text); box-shadow: var(--shadow-lg); }
  .rename-dialog::backdrop { background: #0009; backdrop-filter: blur(5px); }
  h3 { margin-top: 0; }
  .actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-4); }
</style>
