<script lang="ts">
  import { onMount } from 'svelte';
  import { launcher } from './store.svelte';
  import { playerModel, renderPlayer } from './playerModel';
  import Icon from '$lib/shell/Icon.svelte';
  let canvas: HTMLCanvasElement | undefined = $state();
  let visible = $state(false);
  let yaw = $state(-.42);
  let startX: number | null = null;
  let startYaw = 0;
  let pending: number | null = null;
  const model = $derived(playerModel(launcher.selectedAccount ? launcher.accountAvatars[launcher.selectedAccount.accountId] : null));
  $effect(() => {
    // One redraw for a changed skin/angle; no idle requestAnimationFrame loop.
    const currentModel = model, angle = yaw;
    if (!visible || !canvas) return;
    const context = canvas.getContext('2d');
    if (context) renderPlayer(context, currentModel, angle);
  });
  onMount(() => {
    const width = matchMedia('(min-width: 901px)');
    const update = () => { visible = !document.hidden && width.matches; };
    update(); document.addEventListener('visibilitychange', update); width.addEventListener('change', update);
    return () => { document.removeEventListener('visibilitychange', update); width.removeEventListener('change', update); if (pending !== null) cancelAnimationFrame(pending); };
  });
  function move(event: PointerEvent) {
    if (startX === null || pending !== null || !visible) return;
    const next = startYaw + (event.clientX - startX) * .01;
    pending = requestAnimationFrame(() => { pending = null; yaw = next; });
  }
</script>
<div class="player-preview">
  <div class="player-model" role="img" aria-label={model.fallback ? 'Default Minecraft player' : 'Current Minecraft player skin'}
    onpointerdown={event => { if (event.button !== 0) return; startX = event.clientX; startYaw = yaw; event.currentTarget.setPointerCapture(event.pointerId); }}
    onpointermove={move} onpointerup={() => startX = null} onlostpointercapture={() => startX = null}>
    <canvas bind:this={canvas} width="420" height="600" aria-hidden="true"></canvas>
  </div>
  <div class="player-controls" role="group" aria-label="Player view">
    <button class="f-icon-button" aria-label="Rotate player left" title="Rotate left" onclick={() => yaw -= .35}><Icon name="left" size={14} /></button>
    <button class="reset-view" onclick={() => yaw = -.42} title="Reset player view">Reset view</button>
    <button class="f-icon-button" aria-label="Rotate player right" title="Rotate right" onclick={() => yaw += .35}><Icon name="right" size={14} /></button>
  </div>
</div>
<style>
  .player-preview { display: grid; justify-items: center; min-width: 0; position: relative; }
  .player-model { width: 100%; display: grid; place-items: center; cursor: grab; touch-action: pan-y; }
  .player-model:active { cursor: grabbing; }
  canvas { display: block; width: min(100%, 310px); height: auto; filter: drop-shadow(0 14px 18px rgb(0 0 0 / 25%)); }
  .player-controls { display: flex; align-items: center; gap: 12px; opacity: 0; transition: opacity var(--f-duration); }
  .player-preview:hover .player-controls, .player-preview:focus-within .player-controls { opacity: 1; }
  .player-controls .f-icon-button { width: 28px; height: 28px; }
  .reset-view { background: none; border: 0; font: inherit; font-size: 11px; color: var(--color-text-secondary); cursor: pointer; }
  @media (max-width: 900px) { .player-preview { display: none; } }
  @media (prefers-reduced-motion: reduce) { .player-controls { transition: none; } }
</style>
