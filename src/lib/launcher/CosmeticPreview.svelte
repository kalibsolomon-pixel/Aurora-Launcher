<script lang="ts">
  import { onMount } from 'svelte';
  import type { CapePreview, HeadAvatar, SkinModel } from '$lib/backend';
  import { playerModel, renderPlayer, PLAYER_RENDER_WIDTH, PLAYER_RENDER_HEIGHT, arrowYawDelta } from './playerModel';
  import { DEFAULT_VIEW, MIN_ZOOM, MAX_ZOOM, playerView } from './playerView';
  let { avatar, cape = null, label = 'Skin preview' }: { avatar: HeadAvatar | null; cape?: CapePreview | null; label?: string } = $props();
  let canvas: HTMLCanvasElement | undefined = $state();
  let surface: HTMLDivElement | undefined = $state();
  let visible = $state(false), view = $state({...DEFAULT_VIEW}), dragging = $state(false);
  let geometry = $state<SkinModel>('classic'), outerLayers = $state(true), capeVisible = $state(true);
  let pointer = -1, lastX = 0, lastY = 0;
  const model = $derived(playerModel(avatar ? {...avatar,model:geometry} : null, capeVisible ? cape : null, outerLayers));
  const yaw = $derived(view.yaw);
  const degrees = $derived(Math.round(((yaw * 180 / Math.PI) % 360 + 360) % 360));
  $effect(() => { const identity=avatar?.sha256, sourceModel=avatar?.model; void identity; geometry=sourceModel ?? 'classic'; view={...DEFAULT_VIEW}; });
  $effect(() => {
    const current = model, camera = view, target = canvas;
    if (!visible || !target || current.fallback) return;
    const frame = requestAnimationFrame(() => { const context = target.getContext('2d'); if (context) renderPlayer(context,current,camera.yaw,PLAYER_RENDER_WIDTH,PLAYER_RENDER_HEIGHT,camera); });
    return () => cancelAnimationFrame(frame);
  });
  onMount(() => {
    let intersecting = false;
    const update = () => visible = intersecting && !document.hidden;
    const observer = new IntersectionObserver(entries => { intersecting = entries[0].isIntersecting; update(); });
    if (surface) observer.observe(surface);
    document.addEventListener('visibilitychange',update);
    return () => { observer.disconnect(); document.removeEventListener('visibilitychange',update); };
  });
  function start(event: PointerEvent) {
    if (model.fallback || !event.isPrimary || event.button !== 0) return;
    pointer = event.pointerId; lastX = event.clientX; lastY = event.clientY; dragging = true;
    surface?.focus({preventScroll:true});
    surface?.setPointerCapture(pointer);
  }
  function move(event: PointerEvent) {
    if (!dragging || pointer !== event.pointerId) return;
    adjust(-Math.max(-80, Math.min(80, event.clientX-lastX)) * .012, Math.max(-60,Math.min(60,event.clientY-lastY))*.006);
    lastX = event.clientX; lastY = event.clientY;
  }
  function end() { dragging = false; pointer = -1; }
  function adjust(yawDelta=0, pitchDelta=0, zoomDelta=0) { view=playerView(view.yaw+yawDelta,view.pitch+pitchDelta,view.zoom+zoomDelta); }
  function key(event: KeyboardEvent) {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); adjust(arrowYawDelta(event.key==='ArrowLeft'?'left':'right')); }
    else if(event.key==='ArrowUp'||event.key==='ArrowDown') { event.preventDefault(); adjust(0,event.key==='ArrowUp'?-.08:.08); }
    else if(event.key==='+'||event.key==='=') { event.preventDefault(); adjust(0,0,.05); }
    else if(event.key==='-') { event.preventDefault(); adjust(0,0,-.05); }
    else if (event.key==='Home') { event.preventDefault(); view={...DEFAULT_VIEW}; }
  }
</script>
<div class="cosmetic-preview">
  <div class="preview-stage" bind:this={surface} class:dragging class:missing={model.fallback} role="slider" tabindex={model.fallback ? -1 : 0} aria-valuemin="0" aria-valuemax="360" aria-valuenow={degrees} aria-valuetext={`${degrees} degrees rotation, ${Math.round(view.pitch*180/Math.PI)} degrees tilt, ${Math.round(view.zoom*100)} percent zoom`} aria-label={`${label}. Drag horizontally to rotate and vertically to tilt. Arrow keys rotate and tilt. Plus and minus zoom. Home resets.`} onpointerdown={start} onpointermove={move} onpointerup={end} onpointercancel={end} onlostpointercapture={end} onkeydown={key}>
    {#if !model.fallback}<canvas bind:this={canvas} width={PLAYER_RENDER_WIDTH} height={PLAYER_RENDER_HEIGHT} aria-hidden="true"></canvas>
    {:else}<p>Texture unavailable<br /><span>Select a saved skin to preview it.</span></p>{/if}
  </div>
  {#if !model.fallback}
    <div class="view-controls" role="group" aria-label="Player preview views">
      <button type="button" class="btn btn-quiet" onclick={() => view={...view,yaw:0}}>Front</button>
      <button type="button" class="btn btn-quiet" onclick={() => view={...view,yaw:Math.PI}}>Back</button>
      <button type="button" class="btn btn-quiet" onclick={() => view={...DEFAULT_VIEW}}>Reset view</button>
      <button type="button" class="btn btn-quiet" aria-label="Zoom out" disabled={view.zoom<=MIN_ZOOM} onclick={()=>adjust(0,0,-.05)}>−</button>
      <output aria-label="Preview zoom">{Math.round(view.zoom*100)}%</output>
      <button type="button" class="btn btn-quiet" aria-label="Zoom in" disabled={view.zoom>=MAX_ZOOM} onclick={()=>adjust(0,0,.05)}>+</button>
    </div>
    <div class="model-controls" role="group" aria-label="Local preview settings">
      <label>Model preview <select aria-label="Model preview" bind:value={geometry} disabled={avatar?.skinHeight===32}><option value="classic">Classic</option><option value="slim">Slim</option></select></label>
      <label><input type="checkbox" bind:checked={outerLayers} /> Outer layers</label>
      {#if cape}<label><input type="checkbox" bind:checked={capeVisible} /> Show cape</label>{/if}
    </div>
    <p class="drag-hint">Drag to rotate &amp; tilt · Arrows / + / − / Home when focused<br />{degrees}° rotation · {Math.round(view.pitch*180/Math.PI)}° tilt · View controls stay local</p>
  {/if}
</div>
<style>
  .cosmetic-preview { min-width:0; text-align:center; }
  .preview-stage { position:relative; height:clamp(320px,48vh,480px); min-width:0; overflow:hidden; display:grid; place-items:center; border-radius:18px; cursor:grab; touch-action:none; background:radial-gradient(ellipse at 50% 65%,var(--color-accent-soft),transparent 65%); }
  .preview-stage.dragging { cursor:grabbing; }
  .preview-stage.missing { cursor:default; }
  .preview-stage:focus-visible { outline:2px solid var(--color-accent); outline-offset:3px; }
  canvas { position:absolute; inset:0; display:block; width:100%; height:100%; object-fit:contain; filter:drop-shadow(0 12px 10px rgb(0 0 0 / 24%)); }
  p { color:var(--color-text-secondary); font-size:13px; line-height:1.7; margin:0; }
  p span,.drag-hint { color:var(--color-text-muted); font-size:11px; }
  .view-controls { display:flex; justify-content:center; gap:4px; flex-wrap:wrap; margin:8px 0; }
  .view-controls .btn { font-size:12px; min-height:34px; padding:7px 10px; }
  output { align-self:center; min-width:4ch; font-size:12px; color:var(--color-text-secondary); }
  .model-controls { display:flex; justify-content:center; align-items:center; flex-wrap:wrap; gap:16px; margin:12px 0; }
  .model-controls label { display:flex; align-items:center; gap:6px; color:var(--color-text-secondary); font-size:12px; }
  select { font:inherit; color:var(--color-text); background:var(--color-surface-sunken); border:1px solid var(--color-border-strong); border-radius:8px; min-height:34px; padding:5px; }
  input { accent-color:var(--color-accent); }
  @media(max-width:900px) { .preview-stage { height:360px; } }
</style>
