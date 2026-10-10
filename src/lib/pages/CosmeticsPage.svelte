<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { HeadAvatar, SkinModel } from '$lib/backend';
  import { launcher } from '$lib/launcher/store.svelte';
  import { cosmetics } from '$lib/launcher/cosmetics.svelte';
  import { navigation } from '$lib/launcher/navigation.svelte';
  import { accountManager } from '$lib/launcher/accountManager.svelte';
  import { libraryEntries, isCurrentSkin, validSkinName } from '$lib/launcher/skinLibrary';
  import { savedSkinTexture } from '$lib/launcher/skinTextures';
  import CosmeticPreview from '$lib/launcher/CosmeticPreview.svelte';
  import SkinSwatch from '$lib/launcher/SkinSwatch.svelte';
  import CapeThumbnail from '$lib/launcher/widgets/CapeThumbnail.svelte';
  import Icon from '$lib/shell/Icon.svelte';
  const account = $derived(launcher.selectedAccount);
  const accountId = $derived(account?.accountId ?? null);
  const remote = $derived(cosmetics.remote?.accountId === accountId ? cosmetics.remote : null);
  const fresh = $derived(cosmetics.fresh && cosmetics.accountId === accountId && account?.status !== 'reauthenticationRequired');
  const tab = $derived(navigation.cosmeticsTab);
  const entries = $derived(libraryEntries(cosmetics.presets,cosmetics.query,cosmetics.favoritesOnly,cosmetics.sort));
  const selected = $derived(cosmetics.presets.find(p=>p.id===cosmetics.selectedPresetId) ?? null);
  const currentAvatar = $derived(remote?.currentSkin ?? (accountId ? launcher.accountAvatars[accountId] : null) ?? null);
  const currentConfirmed = $derived(fresh ? remote?.currentSkin : null);
  const capes = $derived(remote?.capes ?? []);
  let texture = $state<HeadAvatar | null>(null), textureError = $state(''), textureLoading = $state(false), retryTexture = $state(0);
  let capeChoice = $state<string | null | undefined>(undefined);
  const cape = $derived(capeChoice === null ? null : capes.find(c=>c.id === capeChoice) ?? capes.find(c=>c.selected) ?? null);
  const capeIsActive = $derived(cape ? cape.selected : !capes.some(c=>c.selected));
  const previewAvatar = $derived(selected ? texture : currentAvatar);
  let notice = $state('');
  let dialog: HTMLDialogElement | undefined = $state(), nameInput: HTMLInputElement | undefined = $state();
  let mode = $state<'import' | 'rename' | 'delete' | null>(null), dialogId = $state(''), draft = $state('');
  let file = $state<globalThis.File | null>(null), importModel = $state<SkinModel>('classic');
  let focusReturn: HTMLElement | null = null;
  const dialogPreset = $derived(cosmetics.presets.find(p=>p.id===dialogId));
  $effect(() => { const id = accountId; untrack(() => { capeChoice=undefined; notice=''; void cosmetics.load(id); }); });
  $effect(() => {
    const entry=selected; void retryTexture;
    let live=true; texture=null; textureError=''; textureLoading=!!entry;
    if(entry) void savedSkinTexture(entry,retryTexture>0).then(value=>{if(live) texture=value;}).catch(cause=>{if(live) textureError=cause instanceof Error?cause.message:'This texture is unavailable.';}).finally(()=>{if(live) textureLoading=false;});
    return ()=>{live=false;};
  });
  $effect(() => {
    if(mode && dialog && !dialog.open) { dialog.showModal(); void tick().then(()=>{nameInput?.focus(); if(mode==='rename') nameInput?.select();}); }
    else if(!mode && dialog?.open) { dialog.close(); void tick().then(()=>{if(focusReturn?.isConnected) focusReturn.focus();}); }
  });
  function openDialog(next: 'import' | 'rename' | 'delete') {
    focusReturn=document.activeElement as HTMLElement; mode=next; dialogId=selected?.id ?? ''; draft=next==='rename'?selected?.name ?? '':''; file=null; cosmetics.libraryError='';
  }
  function chooseFile(event: Event) {
    file=(event.currentTarget as HTMLInputElement).files?.[0] ?? null;
    if(file) { const candidate=file.name.replace(/\.png$/i,''); draft=validSkinName(candidate)?candidate:'Imported skin'; }
  }
  async function saveDialog(event: SubmitEvent) {
    event.preventDefault();
    if(mode==='import' && file && validSkinName(draft)) {
      const result=await cosmetics.importFile(file,importModel,draft);
      if(result) { cosmetics.selectedPresetId=result.id; cosmetics.query=''; cosmetics.favoritesOnly=false; notice=result.duplicate?'Already in your library.':'Skin saved to your local library.'; mode=null; }
    } else if(mode==='rename' && validSkinName(draft)) {
      if(await cosmetics.rename(dialogId,draft)) { notice='Saved skin renamed.'; mode=null; }
    } else if(mode==='delete') {
      if(await cosmetics.remove(dialogId)) { if(cosmetics.selectedPresetId===dialogId) cosmetics.selectedPresetId=null; notice='Skin removed from your local library. Your account skin is unchanged.'; mode=null; }
    }
  }
  async function apply() { if(selected && await cosmetics.apply(selected.id)) notice='Skin request completed and account profile refreshed.'; }
  async function equipCape() { const success=cape ? await cosmetics.selectCape(cape.id) : await cosmetics.disableCape(); if(success) notice='Cape choice confirmed by Minecraft.'; }
  async function saveCurrent() { if(accountId) { const result=await cosmetics.saveCurrent(accountId); if(result) { cosmetics.selectedPresetId=result.id; notice=result.duplicate?'Already in your library.':'Current skin saved to your library.'; } } }
  function tabsKey(event: KeyboardEvent) {
    if(['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) { event.preventDefault(); navigation.cosmeticsTab=event.key==='Home'?'skins':event.key==='End'?'capes':tab==='skins'?'capes':'skins'; void tick().then(()=>document.getElementById(`cosmetic-tab-${navigation.cosmeticsTab}`)?.focus()); }
  }
</script>

<div class="page f-pilot cosmetics-page">
  <header class="page-header">
    <div><h2 class="page-title">Skins &amp; Capes</h2><p class="page-subtitle">Your collection. Your character.</p></div>
  </header>
  <div class="cosmetics-topbar">
    <div class="cosmetics-tabs" role="tablist" tabindex="-1" aria-label="Cosmetic library" onkeydown={tabsKey}>
      <button id="cosmetic-tab-skins" role="tab" aria-selected={tab==='skins'} aria-controls="cosmetic-panel" tabindex={tab==='skins'?0:-1} onclick={()=>navigation.cosmeticsTab='skins'}>Skin Library <span>{cosmetics.presets.length}</span></button>
      <button id="cosmetic-tab-capes" role="tab" aria-selected={tab==='capes'} aria-controls="cosmetic-panel" tabindex={tab==='capes'?0:-1} onclick={()=>navigation.cosmeticsTab='capes'}>Capes {#if remote}<span>{capes.length}</span>{/if}</button>
    </div>
    <div class="top-actions">
      {#if accountId}<button class="btn btn-secondary" disabled={cosmetics.loading || cosmetics.remoteBusy} onclick={()=>cosmetics.load(accountId,true)}>{cosmetics.loading?'Refreshing…':'Refresh account'}</button>{/if}
      <button class="btn btn-primary" disabled={cosmetics.localBusy} onclick={()=>openDialog('import')}><Icon name="plus" size={16} /> Import skin</button>
    </div>
  </div>
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}
  {#if cosmetics.remoteBusy}<p class="notice" role="status">{cosmetics.operation}</p>{/if}
  {#if cosmetics.remoteError}<div class="service-message" role="alert"><p>{cosmetics.remoteError}</p><span>{remote?'Showing the last fetched profile. Account changes are paused.':'Saved skins are still available below.'}</span></div>{/if}
  {#if !accountId}<p class="account-note">Your local library is always available. <button onclick={()=>accountManager.show()}>Connect an account</button> to apply skins and see owned capes.</p>
  {:else}<p class="account-note">{cosmetics.loading?'Checking Minecraft profile…':fresh?'Minecraft profile refreshed this session.':'Account appearance may be cached.'} {#if account?.status==='reauthenticationRequired'}Sign in again through Accounts to change cosmetics.{/if}</p>{/if}

  <div id="cosmetic-panel" class="cosmetics-layout" role="tabpanel" aria-labelledby={`cosmetic-tab-${tab}`}>
    <section class="library-panel f-surface" aria-label={tab==='skins'?'Saved skin library':'Owned Minecraft capes'}>
      {#if tab==='skins'}
        <div class="section-heading"><div><h3>Skin Library</h3><p>Saved on this Launcher · shared across accounts</p></div><span class="count">{cosmetics.presets.length} / 256</span></div>
        <div class="library-controls">
          <input type="search" aria-label="Search saved skins" placeholder="Search your skins…" bind:value={cosmetics.query} />
          <button class="filter-button" aria-pressed={cosmetics.favoritesOnly} onclick={()=>cosmetics.favoritesOnly=!cosmetics.favoritesOnly}>★ Favorites</button>
          <select aria-label="Sort saved skins" bind:value={cosmetics.sort}><option value="recent">Recent additions</option><option value="name">Name A–Z</option></select>
        </div>
        {#if currentAvatar || remote?.hasCurrentSkin}
          <button class="current-skin" class:chosen={!selected} onclick={()=>cosmetics.selectedPresetId=null}><span class="current-dot"></span><span><strong>{account?.minecraftName}'s skin</strong><small>{currentConfirmed?'Current account skin':'Cached account appearance'} · {currentAvatar?.model==='slim'?'Slim':'Classic'}</small></span><span>Preview →</span></button>
        {/if}
        {#if cosmetics.libraryError}<div class="service-message" role="alert">{cosmetics.libraryError}<button class="btn btn-quiet" onclick={()=>cosmetics.loadPresets(true)}>Reload library</button></div>{/if}
        {#if cosmetics.libraryLoading && !cosmetics.presets.length}<p class="empty" role="status">Loading your saved skins…</p>
        {:else if !cosmetics.presets.length}<div class="empty"><Icon name="cosmetics" size={38} /><h4>Make room for your favorites</h4><p>Import a Java skin to start your collection.<br />You can organize and preview it before applying.</p><button class="btn btn-secondary" onclick={()=>openDialog('import')}>Import your first skin</button></div>
        {:else if !entries.length}<div class="empty"><h4>No matching skins</h4><p>Try a different search or show all saved skins.</p><button class="btn btn-secondary" onclick={()=>{cosmetics.query='';cosmetics.favoritesOnly=false;}}>Clear filters</button></div>
        {:else}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex (named scroll region supports keyboard scrolling) -->
          <div class="cosmetic-grid" role="region" aria-label="Scrollable saved skins" tabindex="0">
            {#each entries as preset (preset.id)}
              <div class="cosmetic-card" class:chosen={selected?.id===preset.id}>
                <button class="card-preview" aria-pressed={selected?.id===preset.id} aria-label={`Preview saved skin ${preset.name}`} onclick={()=>{cosmetics.selectedPresetId=preset.id;notice='';}}><SkinSwatch {preset} /><strong title={preset.name}>{preset.name}</strong><span class="card-meta">{preset.model==='slim'?'Slim · 3px arms':'Classic · 4px arms'}{#if isCurrentSkin(preset,currentConfirmed)}<b>Current</b>{/if}</span></button>
                <button class="favorite" aria-pressed={preset.favorite} aria-label={`${preset.favorite?'Unfavorite':'Favorite'} saved skin ${preset.name}`} disabled={cosmetics.localBusy} onclick={()=>cosmetics.favorite(preset.id,!preset.favorite)}>{preset.favorite?'★':'☆'}</button>
              </div>
            {/each}
          </div>
        {/if}
      {:else}
        <div class="section-heading"><div><h3>Official capes</h3><p>{account ? `Owned by ${account.minecraftName}` : 'Your Minecraft account inventory'}</p></div>{#if remote}<span class="count">{capes.length} owned</span>{/if}</div>
        <p class="cape-explanation">Preview any owned cape, then choose Equip. A preview never changes your account.</p>
        {#if !accountId}<div class="empty"><Icon name="cosmetics" size={38} /><h4>Your capes live with your account</h4><p>Connect a Minecraft account to see its official capes.</p><button class="btn btn-secondary" onclick={()=>accountManager.show()}>Connect account</button></div>
        {:else if cosmetics.loading && !remote}<p class="empty" role="status">Loading owned capes…</p>
        {:else if !remote}<div class="empty"><h4>Cape inventory unavailable</h4><p>Refresh your account when Minecraft Services is available.</p></div>
        {:else}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex (named scroll region supports keyboard scrolling) -->
          <div class="cosmetic-grid cape-grid" role="region" aria-label="Scrollable owned capes" tabindex="0">
            <button class="cosmetic-card no-cape" class:chosen={!cape} aria-pressed={!cape} onclick={()=>capeChoice=null}><span class="no-cape-art"><Icon name="close" size={32} /></span><strong>No Cape</strong><span class="card-meta">{!capes.some(c=>c.selected)?fresh?'Active':'Last known active':'Hide your cape'}</span></button>
            {#each capes as owned (owned.id)}
              <button class="cosmetic-card cape-card" class:chosen={cape?.id===owned.id} aria-pressed={cape?.id===owned.id} aria-label={`Preview owned cape ${owned.name}`} onclick={()=>capeChoice=owned.id}><span class="cape-art"><CapeThumbnail preview={owned.preview} name={owned.name} large /></span><strong title={owned.name}>{owned.name}</strong><span class="card-meta">{owned.selected?fresh?'Active':'Last known active':'Owned · Available'}</span></button>
            {/each}
          </div>
          {#if !capes.length}<p class="no-capes">No capes are reported for this account. Only capes owned by this account appear here.</p>{/if}
        {/if}
      {/if}
    </section>
    <aside class="preview-panel f-surface" aria-label="Cosmetic preview and actions">
      <div class="preview-heading"><span class="eyebrow">{tab==='capes'?'CAPE PREVIEW':selected?'SAVED SKIN':'ACCOUNT APPEARANCE'}</span><h3>{tab==='capes'?cape?.name ?? 'No Cape':selected?.name ?? account?.minecraftName ?? 'Your character'}</h3><p>{tab==='capes'?(fresh&&capeIsActive?'Currently active on this account':'Preview only · not confirmed equipped'):selected?'Local preview · not applied':currentConfirmed?'Current Minecraft skin':'Cached or unavailable appearance'}</p></div>
      <CosmeticPreview avatar={previewAvatar} cape={cape?.preview} label={tab==='capes'?'Player with cape preview':'Player skin preview'} />
      {#if tab==='capes' && selected}<p class="detail-note preview-skin-note">Shown on saved skin {selected.name} · local preview</p>{/if}
      {#if selected && textureLoading}<p role="status">Loading saved texture…</p>{/if}
      {#if selected && textureError}<div class="service-message" role="alert">{textureError}<button class="btn btn-quiet" onclick={()=>retryTexture++}>Retry texture</button></div>{/if}
      <div class="detail-actions">
        {#if tab==='skins' && selected}
          <label class="model-setting">Saved model <select aria-label="Saved skin model" value={selected.model} disabled={cosmetics.localBusy} onchange={event=>cosmetics.setModel(selected.id,event.currentTarget.value as SkinModel)}><option value="classic">Classic · 4px arms</option><option value="slim">Slim · 3px arms</option></select></label>
          <button class="btn btn-primary" disabled={!fresh || cosmetics.remoteBusy || cosmetics.localBusy || !texture || isCurrentSkin(selected,currentConfirmed)} onclick={apply}>{isCurrentSkin(selected,currentConfirmed)?'Current skin':cosmetics.remoteBusy?'Updating account…':`Apply to ${account?.minecraftName ?? 'Minecraft account'}`}</button>
          <div class="secondary-actions"><button class="btn btn-secondary" disabled={cosmetics.localBusy} onclick={()=>openDialog('rename')}>Rename</button><button class="btn btn-quiet" disabled={cosmetics.localBusy} onclick={()=>openDialog('delete')}>Remove from library</button></div>
          <p class="detail-note">Model and name changes stay local. Apply uploads this saved skin to the selected account.</p>
        {:else if tab==='skins'}
          <button class="btn btn-secondary" disabled={!fresh || !remote?.hasCurrentSkin || cosmetics.localBusy} onclick={saveCurrent}>Save current skin to library</button><p class="detail-note">Select a saved skin to preview, rename or apply it.</p>
        {:else}
          <button class="btn btn-primary" disabled={!fresh || cosmetics.remoteBusy || capeIsActive} onclick={equipCape}>{capeIsActive&&fresh?'Currently active':cape?`Equip ${cape.name}`:'Use No Cape'}</button><p class="detail-note">Only your official owned capes can be equipped. No custom cape imports or unlocks.</p>
        {/if}
      </div>
    </aside>
  </div>
</div>

<dialog class="cosmetic-dialog" bind:this={dialog} aria-labelledby="cosmetic-dialog-title" oncancel={event=>{if(cosmetics.localBusy)event.preventDefault();else mode=null;}} onclose={()=>mode=null}>
  <form onsubmit={saveDialog}>
    <h3 id="cosmetic-dialog-title">{mode==='import'?'Import a skin':mode==='rename'?'Rename saved skin':'Remove saved skin?'}</h3>
    {#if mode==='import'}
      <p>Save a Java PNG locally. Your Minecraft account changes only when you choose Apply.</p>
      <label>Skin file<input type="file" accept="image/png,.png" aria-label="Choose skin PNG" onchange={chooseFile} disabled={cosmetics.localBusy} /></label>
      <small>64×64 or legacy 64×32 PNG · up to 128 KiB</small>
      <label>Name<input bind:this={nameInput} bind:value={draft} placeholder="Give this skin a name" disabled={cosmetics.localBusy} /></label>
      <label>Model<select bind:value={importModel} disabled={cosmetics.localBusy}><option value="classic">Classic · 4px arms</option><option value="slim">Slim · 3px arms</option></select></label>
      <small>Choose the intended model. Legacy 64×32 skins use Classic.</small>
    {:else if mode==='rename'}<label>Name<input bind:this={nameInput} bind:value={draft} disabled={cosmetics.localBusy} /></label>
    {:else}<p>Remove <strong>{dialogPreset?.name}</strong> from this Launcher's library? Your active Minecraft skin stays unchanged.</p>{/if}
    {#if mode!=='delete' && draft && !validSkinName(draft)}<p class="form-error">Use a name of 1–80 UTF-8 bytes without control characters.</p>{/if}
    {#if cosmetics.libraryError}<p class="form-error" role="alert">{cosmetics.libraryError}</p>{/if}
    <div class="dialog-actions"><button type="button" class="btn btn-secondary" disabled={cosmetics.localBusy} onclick={()=>mode=null}>Cancel</button><button type="submit" class="btn" class:btn-primary={mode!=='delete'} class:btn-danger={mode==='delete'} disabled={cosmetics.localBusy || (mode!=='delete' && !validSkinName(draft)) || (mode==='import' && !file)}>{cosmetics.localBusy?'Saving…':mode==='import'?'Save to library':mode==='rename'?'Save name':'Remove from library'}</button></div>
  </form>
</dialog>

<style>
  .cosmetics-page { max-width:1600px; margin:0 auto; }
  .page-header { align-items:center; gap:20px; }
  .cosmetics-topbar { display:flex; align-items:center; justify-content:space-between; flex-wrap:wrap; gap:16px; margin:24px 0 12px; }
  .cosmetics-tabs { display:flex; gap:4px; padding:5px; border:1px solid var(--f-edge); border-radius:16px; background:var(--f-panel); backdrop-filter:var(--f-blur); }
  .cosmetics-tabs button { border:0; border-radius:11px; padding:11px 16px; color:var(--color-text-secondary); background:transparent; font:inherit; font-size:14px; cursor:pointer; }
  .cosmetics-tabs button[aria-selected=true] { background:var(--color-accent-soft); color:var(--color-text); }
  .cosmetics-tabs span { font-size:11px; margin-left:7px; color:var(--color-text-muted); }
  .top-actions { display:flex; gap:8px; flex-wrap:wrap; }
  .top-actions .btn { display:inline-flex; align-items:center; gap:6px; }
  .account-note,.notice { font-size:12px; color:var(--color-text-secondary); margin:12px 0 18px; }
  .account-note button { background:transparent; border:0; font:inherit; color:var(--color-accent); text-decoration:underline; cursor:pointer; padding:0; }
  .notice { color:var(--color-success); }
  .cosmetics-layout { display:grid; grid-template-columns:minmax(0,1fr) minmax(300px,380px); gap:20px; align-items:start; }
  .library-panel,.preview-panel { padding:24px; min-width:0; }
  .section-heading { display:flex; align-items:start; justify-content:space-between; gap:12px; margin-bottom:20px; }
  h3 { margin:0; font-size:18px; font-weight:600; overflow-wrap:anywhere; }
  .section-heading p,.preview-heading p { margin:6px 0 0; font-size:12px; color:var(--color-text-secondary); line-height:1.5; }
  .count { color:var(--color-text-muted); font-size:11px; white-space:nowrap; padding-top:5px; }
  .library-controls { display:flex; gap:8px; flex-wrap:wrap; margin-bottom:18px; }
  input,select,.filter-button { background:var(--color-surface-sunken); color:var(--color-text); border:1px solid var(--color-border-strong); border-radius:11px; font:inherit; font-size:12px; min-height:38px; padding:9px 11px; max-width:100%; }
  input[type=search] { flex:1 1 160px; min-width:0; }
  .filter-button { cursor:pointer; }
  .filter-button[aria-pressed=true] { background:var(--color-accent-soft); border-color:var(--color-accent-outline); }
  .current-skin { display:flex; width:100%; align-items:center; gap:12px; padding:14px; margin-bottom:16px; background:rgb(255 255 255 / 3%); color:var(--color-text); border:1px solid var(--f-edge); border-radius:12px; text-align:left; cursor:pointer; font:inherit; }
  .current-skin>span:nth-child(2) { min-width:0; flex:1; }
  .current-skin strong { font-size:13px; overflow-wrap:anywhere; }
  .current-skin small { display:block; margin-top:4px; font-size:11px; color:var(--color-text-secondary); }
  .current-skin>span:last-child { font-size:11px; flex:none; color:var(--color-text-secondary); }
  .current-dot { width:8px; height:8px; border-radius:50%; background:var(--color-accent); flex:none; }
  .cosmetic-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(145px,1fr)); gap:12px; max-height:clamp(260px,calc(100dvh - 560px),620px); overflow-y:auto; overflow-x:hidden; padding:3px; scrollbar-gutter:stable; }
  .cape-grid { grid-template-columns:repeat(auto-fit,minmax(145px,1fr)); }
  .cosmetic-card { position:relative; border:1px solid var(--f-edge); border-radius:15px; background:rgb(255 255 255 / 3%); color:var(--color-text); min-width:0; }
  .cosmetic-card:hover { background:rgb(255 255 255 / 6%); }
  .chosen { border-color:var(--color-accent-outline); box-shadow:inset 0 0 0 1px var(--color-accent-outline); background:var(--color-accent-soft); }
  .card-preview { width:100%; border:0; background:transparent; color:inherit; text-align:left; padding:10px; cursor:pointer; font:inherit; }
  .card-preview strong,.cape-card strong,.no-cape strong { display:block; font-size:13px; font-weight:550; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; margin:10px 0 7px; }
  .card-meta { font-size:10.5px; color:var(--color-text-secondary); display:flex; align-items:center; justify-content:space-between; gap:4px; flex-wrap:wrap; }
  .card-meta b { font-weight:550; color:var(--color-accent); }
  .favorite { position:absolute; top:8px; right:8px; width:30px; height:30px; border:0; border-radius:9px; background:var(--color-surface-sunken); color:var(--color-text-secondary); font-size:20px; cursor:pointer; }
  .favorite[aria-pressed=true] { color:var(--color-accent); }
  .empty { min-height:280px; display:flex; align-items:center; justify-content:center; flex-direction:column; text-align:center; gap:16px; padding:30px 16px; color:var(--color-text-secondary); }
  .empty h4 { color:var(--color-text); margin:0; font-size:17px; font-weight:550; }
  .empty p { font-size:13px; line-height:1.65; margin:0; }
  .preview-heading { text-align:center; margin-bottom:16px; }
  .eyebrow { display:block; font-size:10px; letter-spacing:.12em; color:var(--color-text-muted); margin-bottom:10px; }
  .detail-actions { display:grid; gap:14px; margin-top:20px; }
  .detail-actions>.btn { white-space:normal; overflow-wrap:anywhere; }
  .model-setting { display:flex; flex-direction:column; gap:8px; font-size:12px; color:var(--color-text-secondary); }
  .secondary-actions { display:flex; flex-wrap:wrap; justify-content:center; gap:8px; }
  .detail-note,.cape-explanation,.no-capes { font-size:12px; line-height:1.6; color:var(--color-text-secondary); margin:0; }
  .preview-skin-note { text-align:center; margin-top:12px; }
  .cape-explanation { margin-bottom:20px; }
  .no-capes { padding:20px 0 0; }
  .cape-card,.no-cape { padding:14px; cursor:pointer; text-align:left; font:inherit; }
  .cape-art,.no-cape-art { height:160px; display:grid; place-items:center; background:radial-gradient(ellipse,var(--color-accent-soft),transparent 70%); }
  .no-cape-art { color:var(--color-text-muted); }
  .service-message { padding:12px 14px; margin-bottom:14px; font-size:12px; line-height:1.6; color:var(--color-text); border:1px solid var(--color-border-strong); background:var(--color-surface-sunken); border-radius:12px; }
  .service-message p { margin:0; }
  .service-message span { color:var(--color-text-secondary); }
  .cosmetic-dialog { width:min(460px,calc(100vw - 32px)); box-sizing:border-box; padding:26px; border:1px solid var(--f-edge); border-radius:22px; background:var(--f-dialog-panel); backdrop-filter:var(--f-blur); color:var(--color-text); box-shadow:var(--f-shadow); }
  .cosmetic-dialog::backdrop { background:var(--f-dialog-dim); }
  form { display:grid; gap:16px; }
  form p { font-size:13px; line-height:1.6; margin:0; color:var(--color-text-secondary); }
  form label { display:grid; gap:7px; font-size:12px; }
  form small { color:var(--color-text-muted); font-size:11px; }
  .dialog-actions { display:flex; justify-content:flex-end; flex-wrap:wrap; gap:8px; margin-top:6px; }
  form .form-error { color:var(--color-danger); }
  @media(max-width:1100px) { .cosmetics-layout { grid-template-columns:minmax(0,1fr) minmax(260px,320px); gap:16px; } .library-panel,.preview-panel { padding:20px; } }
  @media(max-width:900px) { .cosmetics-layout { grid-template-columns:minmax(0,1fr); } .preview-panel { max-width:none; } .page-header { flex-wrap:wrap; } .cosmetic-grid { max-height:440px; } }
  @media(max-width:600px) { .library-panel,.preview-panel { padding:16px; } .cosmetic-grid { grid-template-columns:repeat(2,minmax(0,1fr)); gap:8px; } .current-skin>span:last-child { display:none; } .top-actions .btn { padding:10px; } }
</style>
