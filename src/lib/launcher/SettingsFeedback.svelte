<script lang="ts">
  let { busy, error, saved, retry, retryLabel = 'Retry save' }: { busy: boolean; error: string; saved: boolean; retry: () => void; retryLabel?: string } = $props();
</script>
<div class="settings-feedback" aria-live="polite" aria-atomic="true">
  {#if busy}<p role="status">Saving…</p>
  {:else if error}<div class="settings-save-error"><p role="alert">{error}</p><button class="btn btn-quiet" type="button" onclick={retry}>{retryLabel}</button></div>
  {:else if saved}<p role="status">Saved. Changes are live.</p>{/if}
</div>
