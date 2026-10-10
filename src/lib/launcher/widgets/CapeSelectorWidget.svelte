<script lang="ts">
  import { untrack } from 'svelte';
  import type { WidgetSize } from '$lib/backend';
  import { launcher } from '../store.svelte';
  import { cosmetics } from '../cosmetics.svelte';
  import { navigation } from '../navigation.svelte';
  import CapeThumbnail from './CapeThumbnail.svelte';
  let { size }: { size: WidgetSize } = $props();
  const accountId = $derived(launcher.selectedAccount?.accountId ?? null);
  const remote = $derived(cosmetics.remote?.accountId===accountId ? cosmetics.remote : null);
  const active = $derived(remote?.capes.find(c=>c.selected));
  $effect(()=>{const id=accountId;untrack(()=>{void cosmetics.load(id);});});
</script>
<div class="cape-summary" class:large={size==='large'}>
  <div class="active-cape">
    <span class="cape-art">{#if active}<CapeThumbnail preview={active.preview} name={active.name} />{:else}<span class="no-cape" aria-hidden="true">×</span>{/if}</span>
    <div><strong>{!accountId?'Your capes':cosmetics.loading&&!remote?'Loading…':!remote?'Inventory unavailable':active?.name ?? 'No Cape'}</strong><p>{launcher.selectedAccount?.minecraftName ?? 'Connect a Minecraft account'}</p>{#if remote}<p>{cosmetics.fresh?'Current account choice':'Last known choice'} · {remote.capes.length} owned</p>{/if}</div>
  </div>
  {#if accountId && cosmetics.loading}<p role="status">Loading owned capes…</p>{:else if accountId && !cosmetics.fresh}<p>Cape changes are paused until the account refreshes.</p>{:else if remote && !remote.capes.length}<p>No capes are reported for this account.</p>{/if}
  {#if cosmetics.remoteError}<p class="inline-message-error" role="alert">{cosmetics.remoteError}</p>{/if}
  <button type="button" class="btn btn-secondary" onclick={()=>navigation.openCosmetics('capes')}>Open Cape Library <span>View &amp; equip →</span></button>
</div>
<style>
  .cape-summary { display:grid; gap:14px; min-width:0; }
  .active-cape { display:flex; align-items:center; gap:14px; min-width:0; }
  .active-cape>div { min-width:0; }
  .cape-art { width:54px; height:72px; flex:none; display:grid; place-items:center; background:var(--color-accent-soft); border:1px solid var(--f-edge); border-radius:12px; }
  .no-cape { color:var(--color-text-muted); font-size:28px; }
  strong { display:block; overflow-wrap:anywhere; font-size:14px; font-weight:550; margin-bottom:6px; }
  p { margin:0; font-size:var(--text-metadata); line-height:1.65; color:var(--color-text-secondary); overflow-wrap:anywhere; }
  button { display:flex; align-items:center; justify-content:space-between; gap:8px; width:100%; white-space:normal; text-align:left; font-size:12px; }
  button span { font-size:10px; color:var(--color-text-secondary); }
  .large .cape-art { width:64px; height:88px; }
</style>
