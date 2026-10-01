<script lang="ts">
  import { onMount } from 'svelte';
  import { launcher } from './store.svelte';
  import { playerModel, renderPlayer, arrowYawDelta, PLAYER_DEFAULT_YAW, PLAYER_RENDER_WIDTH, PLAYER_RENDER_HEIGHT } from './playerModel';
  import Icon from '$lib/shell/Icon.svelte';
  let canvas: HTMLCanvasElement | undefined = $state();
  let visible = $state(false);
  let revealed = $state(false);
  let yaw = $state(PLAYER_DEFAULT_YAW);
  const avatar = $derived(launcher.selectedAccount ? launcher.accountAvatars[launcher.selectedAccount.accountId] : null);
  const model = $derived(playerModel(avatar));
  // A fallback model means no validated skin for THIS account is available
  // yet: render an empty region instead of a generic player. The account-keyed
  // avatar cache guarantees another account's skin is never displayed.
  const hasSkin = $derived(!model.fallback);
  $effect(() => {
    if (hasSkin) {
      const id = setTimeout(() => { revealed = true; }, 30);
      return () => clearTimeout(id);
    }
    revealed = false;
  });
  $effect(() => {
    // One redraw for a changed skin/angle; no idle animation loop.
    const currentModel = model, angle = yaw;
    if (!visible || !canvas || !hasSkin) return;
    const context = canvas.getContext('2d');
    if (context) renderPlayer(context, currentModel, angle);
  });
  onMount(() => {
    const width = matchMedia('(min-width: 901px)');
    const update = () => { visible = !document.hidden && width.matches; };
    update(); document.addEventListener('visibilitychange', update); width.addEventListener('change', update);
    return () => { document.removeEventListener('visibilitychange', update); width.removeEventListener('change', update); };
  });
</script>
<div class="player-preview">
  <!-- Passive visual surface: the arrows below are the only manual rotation. -->
  <div class="player-model" role="img" aria-label={hasSkin ? 'Current Minecraft player skin' : 'Player skin loading'}>
    {#if hasSkin}
      <canvas bind:this={canvas} width={PLAYER_RENDER_WIDTH} height={PLAYER_RENDER_HEIGHT} aria-hidden="true" class:revealed></canvas>
    {/if}
  </div>
  {#if hasSkin}
    <!-- In normal flow beneath the model with its own reserved space, so the
         only manual view controls never depend on leftover room or overlays. -->
    <div class="player-controls" role="group" aria-label="Player view">
      <!-- Positive yaw turns the rendered player toward screen left (see
           arrowYawDelta); each activation is one bounded state update. -->
      <button class="f-icon-button" aria-label="Rotate player left" title="Rotate left" onclick={() => yaw += arrowYawDelta("left")}><Icon name="left" size={18} /></button>
      <button class="reset-view" aria-label="Reset player view" title="Reset player view" onclick={() => yaw = PLAYER_DEFAULT_YAW}>Reset view</button>
      <button class="f-icon-button" aria-label="Rotate player right" title="Rotate right" onclick={() => yaw += arrowYawDelta("right")}><Icon name="right" size={18} /></button>
    </div>
  {/if}
</div>
<style>
  .player-preview { --player-max-h: min(66vh, 640px, max(280px, calc(100dvh - 300px))); display: flex; flex-direction: column; align-items: center; gap: 14px; min-width: 0; position: relative; }
  /* The model is bounded so the model plus the control row fits the visible
     window: the approved large scale applies while space allows and yields
     only when the viewport is constrained. The canvas width is derived from
     the same bound, so the raster can never overflow its container into the
     control row. The floor keeps extreme cases scrollable instead of
     collapsing the preview. */
  .player-model { width: 100%; aspect-ratio: 560 / 800; max-height: var(--player-max-h); display: grid; place-items: center; }
  canvas { display: block; width: min(100%, 480px, calc(var(--player-max-h) * 0.7)); height: auto; opacity: 0; filter: drop-shadow(0 18px 24px rgb(0 0 0 / 28%)); transition: opacity 420ms ease-out; }
  canvas.revealed { opacity: 1; }
  /* Always visible while a player is shown: these are the only view controls,
     so they must never hide behind a hover reveal or shrink with the model. */
  .player-controls { display: flex; align-items: center; gap: 4px; flex: none; padding: 6px 8px; border: 1px solid var(--f-edge); border-radius: 999px; background: var(--f-panel); backdrop-filter: var(--f-blur); box-shadow: var(--shadow-group); }
  .player-controls .f-icon-button { width: 40px; height: 40px; border-color: transparent; border-radius: 999px; background: transparent; }
  .player-controls .f-icon-button:hover { background: rgb(255 255 255 / 10%); color: var(--color-text); }
  .player-controls .reset-view { border: 0; background: transparent; font: inherit; font-size: 12.5px; font-weight: 550; color: var(--color-text-secondary); padding: 10px 12px; border-radius: 999px; cursor: pointer; transition: background var(--f-fast), color var(--f-fast); }
  .player-controls .reset-view:hover { background: rgb(255 255 255 / 10%); color: var(--color-text); }
  @media (max-width: 900px) { .player-preview { display: none; } }
  @media (prefers-reduced-motion: reduce) { canvas { transition: none; } }
</style>
