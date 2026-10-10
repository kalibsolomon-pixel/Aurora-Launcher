<script lang="ts">
  import { onMount } from 'svelte';
  import type { CapePreview, HeadAvatar } from '$lib/backend';
  import { playerModel, renderPlayer, PLAYER_DEFAULT_YAW, PLAYER_RENDER_WIDTH, PLAYER_RENDER_HEIGHT, arrowYawDelta } from './playerModel';
  let { avatar, cape = null, label = 'Skin preview' }: { avatar: HeadAvatar | null; cape?: CapePreview | null; label?: string } = $props();
  let canvas: HTMLCanvasElement | undefined = $state();
  let surface: HTMLDivElement | undefined = $state();
  let visible = $state(false), yaw = $state(PLAYER_DEFAULT_YAW), dragging = $state(false);
  let pointer = -1, lastX = 0;
  const model = $derived(playerModel(avatar, cape));
  const degrees = $derived(Math.round(((yaw * 180 / Math.PI) % 360 + 360) % 360));
  $effect(() => {
    const current = model, angle = yaw, target = canvas;
    if (!visible || !target || current.fallback) return;
    const frame = requestAnimationFrame(() => { const context = target.getContext('2d'); if (context) renderPlayer(context,current,angle); });
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
    pointer = event.pointerId; lastX = event.clientX; dragging = true;
    surface?.setPointerCapture(pointer);
  }
  function move(event: PointerEvent) {
    if (!dragging || pointer !== event.pointerId) return;
    yaw -= Math.max(-80, Math.min(80, event.clientX-lastX)) * .012; lastX = event.clientX;
  }
  function end() { dragging = false; pointer = -1; }
  function key(event: KeyboardEvent) {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); yaw += arrowYawDelta(event.key==='ArrowLeft'?'left':'right'); }
    else if (event.key==='Home') { event.preventDefault(); yaw=PLAYER_DEFAULT_YAW; }
  }
</script>
<div class="cosmetic-preview">
  <div class="preview-stage" bind:this={surface} class:dragging class:missing={model.fallback} role="slider" tabindex={model.fallback ? -1 : 0} aria-label={`${label}. Drag or use arrow keys to rotate`} aria-valuemin="0" aria-valuemax="360" aria-valuenow={degrees} aria-valuetext={`${degrees} degrees`} onpointerdown={start} onpointermove={move} onpointerup={end} onpointercancel={end} onlostpointercapture={end} onkeydown={key}>
    {#if !model.fallback}<canvas bind:this={canvas} width={PLAYER_RENDER_WIDTH} height={PLAYER_RENDER_HEIGHT} aria-hidden="true"></canvas>
    {:else}<p>Texture unavailable<br /><span>Select a saved skin to preview it.</span></p>{/if}
  </div>
  {#if !model.fallback}
    <div class="view-controls" role="group" aria-label="Player preview views">
      <button type="button" class="btn btn-quiet" onclick={() => yaw=0}>Front</button>
      <button type="button" class="btn btn-quiet" onclick={() => yaw=Math.PI}>Back</button>
      <button type="button" class="btn btn-quiet" onclick={() => yaw=PLAYER_DEFAULT_YAW}>Reset view</button>
    </div>
    <p class="drag-hint">Drag to rotate · Arrow keys when focused</p>
  {/if}
</div>
<style>
  .cosmetic-preview { min-width:0; text-align:center; }
  .preview-stage { position:relative; height:clamp(240px,35vh,360px); min-width:0; overflow:hidden; display:grid; place-items:center; border-radius:18px; cursor:grab; touch-action:pan-y; background:radial-gradient(ellipse at 50% 80%,var(--color-accent-soft),transparent 65%); }
  .preview-stage.dragging { cursor:grabbing; }
  .preview-stage.missing { cursor:default; }
  .preview-stage:focus-visible { outline:2px solid var(--color-accent); outline-offset:3px; }
  canvas { position:absolute; inset:0; display:block; width:100%; height:100%; object-fit:contain; filter:drop-shadow(0 12px 10px rgb(0 0 0 / 24%)); }
  p { color:var(--color-text-secondary); font-size:13px; line-height:1.7; margin:0; }
  p span,.drag-hint { color:var(--color-text-muted); font-size:11px; }
  .view-controls { display:flex; justify-content:center; gap:4px; flex-wrap:wrap; margin:8px 0; }
  .view-controls .btn { font-size:12px; min-height:34px; padding:7px 10px; }
  @media(max-width:1050px) { .preview-stage { height:280px; } }
</style>
