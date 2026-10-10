<script lang="ts">
  import { untrack } from 'svelte';
  import type { WidgetSize } from '$lib/backend';
  import { launcher } from '../store.svelte';
  import { cosmetics } from '../cosmetics.svelte';
  import { navigation } from '../navigation.svelte';
  import { libraryEntries } from '../skinLibrary';
  import AccountIdentity from '../AccountIdentity.svelte';
  import SkinThumbnail from './SkinThumbnail.svelte';
  let { size }: { size: WidgetSize } = $props();
  const account = $derived(launcher.selectedAccount);
  const accountId = $derived(account?.accountId ?? null);
  const remote = $derived(cosmetics.remote?.accountId===accountId ? cosmetics.remote : null);
  const recent = $derived(libraryEntries(cosmetics.presets,'',false,'recent'));
  const visible = $derived([...recent.filter(p=>p.favorite),...recent.filter(p=>!p.favorite)].slice(0,size==='small'?2:size==='wide'?3:4));
  $effect(()=>{ const id=accountId; untrack(()=>{void cosmetics.load(id);}); });
  function open(id: string | null = null) { cosmetics.selectedPresetId=id; navigation.openCosmetics('skins'); }
</script>
<div class="skin-summary">
  <div class="account-row"><AccountIdentity {account} small /></div>
  <p class="current-state">{!accountId?'Browse your local collection without an account.':cosmetics.loading?'Loading current skin…':!cosmetics.fresh?'Account appearance may be cached. Saved skins remain available.':remote?.hasCurrentSkin?`Current skin · ${remote.currentSkinModel==='slim'?'Slim':'Classic'}`:'No current skin reported.'}</p>
  {#if visible.length}
    <ul aria-label="Favorite and recent skins">
      {#each visible as preset (preset.id)}
        <li><button type="button" onclick={()=>open(preset.id)} aria-label={`Open saved skin ${preset.name}`}><SkinThumbnail presetId={preset.id} name={preset.name} /><span class="saved-name" title={preset.name}>{preset.name}</span>{#if preset.favorite}<span class="star" aria-label="Favorite">★</span>{/if}<span class="model">{preset.model==='slim'?'Slim':'Classic'}</span></button></li>
      {/each}
    </ul>
  {:else}<p>No saved skins yet. Import your first skin in the library.</p>{/if}
  {#if cosmetics.libraryError}<p class="inline-message-error" role="alert">{cosmetics.libraryError}</p>{/if}
  <button type="button" class="btn btn-secondary open-library" onclick={()=>open()}>Open Skin Library <span>{cosmetics.presets.length} saved →</span></button>
</div>
<style>
  .skin-summary { display:grid; gap:12px; min-width:0; }
  .account-row { min-width:0; }
  p { margin:0; font-size:var(--text-metadata); line-height:1.55; color:var(--color-text-secondary); }
  ul { display:grid; gap:6px; list-style:none; margin:0; padding:0; }
  li { min-width:0; }
  li button { display:flex; align-items:center; gap:8px; width:100%; min-width:0; padding:8px; color:var(--color-text); background:rgb(255 255 255 / 3%); border:1px solid var(--f-edge); border-radius:10px; cursor:pointer; font:inherit; }
  li button:hover { background:var(--color-accent-soft); }
  .saved-name { flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; text-align:left; font-size:12px; }
  .model { flex:none; font-size:10px; color:var(--color-text-muted); }
  .star { color:var(--color-accent); font-size:12px; }
  .open-library { display:flex; align-items:center; justify-content:space-between; gap:8px; width:100%; white-space:normal; text-align:left; font-size:12px; }
  .open-library span { color:var(--color-text-secondary); font-size:10px; }
</style>
