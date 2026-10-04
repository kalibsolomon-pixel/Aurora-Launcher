<script lang="ts">
  import { onMount } from 'svelte';
  import { desktopIntegration } from './desktopIntegration.svelte';
  import type { ShortcutStatus } from '$lib/backend';
  const labels: Record<ShortcutStatus['state'], string> = {
    present: 'Available', absent: 'Not created', conflict: 'Another shortcut occupies this location', unknown: 'Status unavailable',
  };
  onMount(() => { void desktopIntegration.refresh(); });
</script>
<section class="group" aria-labelledby="general-desktop-title">
  <div class="group-heading"><div><h3 id="general-desktop-title" class="group-title">Desktop integration</h3><p class="group-subtitle">Open Aurora from your Windows desktop or Start menu.</p></div></div>
  {#if !desktopIntegration.state && !desktopIntegration.error}
    <div class="group-row" role="status"><span class="group-row-detail">Reading shortcut status…</span></div>
  {:else if desktopIntegration.state?.supported}
    {@const state = desktopIntegration.state}
    <div class="group-row"><div class="group-row-main"><span class="group-row-title">Desktop shortcut</span><span class="group-row-detail">{labels[state.desktopShortcut.state]}</span></div>
      {#if state.manageable && ['present', 'absent'].includes(state.desktopShortcut.state)}
        <button type="button" class="btn" disabled={desktopIntegration.busy} onclick={() => state.desktopShortcut.state === 'present' ? desktopIntegration.remove() : desktopIntegration.create()}>{state.desktopShortcut.state === 'present' ? 'Remove shortcut' : 'Create shortcut'}</button>
      {/if}
    </div>
    <div class="group-row"><div class="group-row-main"><span class="group-row-title">Start menu shortcut</span><span class="group-row-detail">{labels[state.startMenuShortcut.state]} · Managed by the Aurora installer.</span></div></div>
    {#if !state.manageable}<p class="group-footer">Desktop shortcut changes are available in an installed production build.</p>{/if}
  {:else if desktopIntegration.state}
    <p class="group-footer">Desktop integration is currently available on Windows.</p>
  {/if}
  <div class="group-row"><span class="group-row-detail">Status reflects the shortcuts on this computer.</span><button type="button" class="btn btn-quiet" disabled={desktopIntegration.busy} onclick={() => desktopIntegration.refresh()}>{desktopIntegration.busy ? 'Reading…' : 'Refresh status'}</button></div>
  {#if desktopIntegration.error}<p role="alert" class="group-row inline-message inline-message-error">{desktopIntegration.error.message}</p>{/if}
</section>
