<script lang="ts">
  let { view = $bindable(), label, canBrowse }: {
    view: "installed" | "browse"; label: string; canBrowse: boolean;
  } = $props();
</script>

<div class="content-view-tabs" role="group" aria-label={label}>
  <button type="button" class="btn btn-quiet" aria-pressed={view === "installed"} onclick={() => view = "installed"}>Installed</button>
  {#if canBrowse}<button type="button" class="btn btn-quiet" aria-pressed={view === "browse"} onclick={() => view = "browse"}>Browse Modrinth</button>{/if}
</div>

<style>
  .content-view-tabs { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-2); margin-bottom: var(--space-4); }
  .content-view-tabs .btn { min-height: 40px; }
  .content-view-tabs [aria-pressed="true"] { color: var(--color-text); background: var(--color-surface-raised); }
</style>
