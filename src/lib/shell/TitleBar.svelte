<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { isTauri } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  let custom = $state(false);
  let maximized = $state(false);
  let error = $state('');
  async function action(operation: 'minimize' | 'toggleMaximize' | 'close') {
    try { await getCurrentWindow()[operation](); error = ''; }
    catch { error = 'Window action unavailable. Use Alt+F4 to close.'; }
  }
  onMount(() => {
    if (!isTauri()) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    const window = getCurrentWindow();
    // Rust removes decorations on Windows only. Keep OS chrome elsewhere.
    void window.isDecorated().then(value => { if (!disposed) custom = !value; }).catch(() => { if (!disposed) error = "Window controls unavailable. Use Alt+F4 to close."; });
    const update = async () => { try { const value = await window.isMaximized(); if (!disposed) maximized = value; } catch { /* A closing window no longer has queryable state. */ } };
    void update();
    void window.onResized(() => { void update(); }).then(unlisten => { if (disposed) unlisten(); else stop = unlisten; }).catch(() => { /* Controls still work without resize notifications. */ });
    return () => { disposed = true; stop?.(); };
  });
</script>
{#if custom}
  <header class="aurora-titlebar" aria-label="Window controls">
    <div class="titlebar-drag" data-tauri-drag-region>
      <img src="/aurora-icon.png" alt="" draggable="false" />
      <span>Aurora Launcher</span>
    </div>
    <button class="window-control" aria-label="Minimize" title="Minimize" onclick={() => action('minimize')}><Icon name="minimize" size={16} /></button>
    <button class="window-control" aria-label={maximized ? 'Restore' : 'Maximize'} title={maximized ? 'Restore' : 'Maximize'} onclick={() => action('toggleMaximize')}><Icon name={maximized ? 'restore' : 'maximize'} size={15} /></button>
    <button class="window-control window-close" aria-label="Close Aurora Launcher" title="Close" onclick={() => action('close')}><Icon name="close" size={18} /></button>
  </header>
{/if}
{#if error}<p class="chrome-error" role="alert">{error}</p>{/if}
