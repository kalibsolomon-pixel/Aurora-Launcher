<script lang="ts">
  import { onMount } from 'svelte';
  import { desktopIntegration } from './desktopIntegration.svelte';
  import type { ShortcutStatus } from '$lib/backend';
  import Tooltip from '$lib/shell/Tooltip.svelte';
  const labels: Record<ShortcutStatus['state'], string> = {
    present: 'Available', absent: 'Not created', conflict: 'Another shortcut occupies this location', unknown: 'Status unavailable',
  };
  onMount(() => { void desktopIntegration.refresh(); });
</script>
<section class="group" aria-label="Desktop integration">
  <div class="group-row">
    <div class="action-label"><span class="group-row-title">Desktop shortcut</span>
      <Tooltip label="About desktop shortcuts">Creates a Windows shortcut to this launcher executable. {desktopIntegration.state ? labels[desktopIntegration.state.desktopShortcut.state] : 'Reading shortcut status…'}. Shortcut changes require an installed production build.</Tooltip>
    </div>
    <button type="button" class="btn" disabled={desktopIntegration.busy} onclick={() => desktopIntegration.create()}>Add Desktop Shortcut</button>
  </div>
  <div class="group-row">
    <div class="action-label"><span class="group-row-title">Start menu</span>
      <Tooltip label="About the Start menu shortcut">Adds Aurora to the Start menu app list with a Programs shortcut. Windows requires you to pin it to the Start grid yourself. {desktopIntegration.state ? labels[desktopIntegration.state.startMenuShortcut.state] : 'Reading shortcut status…'}. Shortcut changes require an installed production build.</Tooltip>
    </div>
    <button type="button" class="btn" disabled={desktopIntegration.busy} onclick={() => desktopIntegration.addToStart()}>Add to Start Menu</button>
  </div>
  <div class="group-row">
    <div class="action-label"><span class="group-row-title">Launcher data</span>
      <Tooltip label="About the launcher path">Opens Aurora’s managed data folder in Windows Explorer.</Tooltip>
    </div>
    <button type="button" class="btn" disabled={desktopIntegration.busy} onclick={() => desktopIntegration.openPath()}>Open Path</button>
  </div>
  {#if desktopIntegration.error}<p role="alert" class="group-row inline-message inline-message-error">{desktopIntegration.error.message}</p>{/if}
  {#if desktopIntegration.notice}<p role="status" class="group-row group-row-detail">{desktopIntegration.notice}</p>{/if}
</section>
<style>
  .action-label { display: flex; align-items: center; gap: 6px; }
</style>
