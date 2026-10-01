<script lang="ts">
  import { onMount } from 'svelte';
  import { launcher } from './store.svelte';
  import { playerModel, renderPlayer, PLAYER_RENDER_WIDTH, PLAYER_RENDER_HEIGHT } from './playerModel';
  import Icon from '$lib/shell/Icon.svelte';
  let canvas: HTMLCanvasElement | undefined = $state();
  let visible = $state(false);
  let revealed = $state(false);
  let yaw = $state(-.42);
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
    // One redraw for a changed skin/angle; no idle requestAnimationFrame loop.
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
  <div class="player-controls" role="group" aria-label="Player view">
    <button class="f-icon-button" aria-label="Rotate player left" title="Rotate left" disabled={!hasSkin} onclick={() => yaw -= .35}><Icon name="left" size={14} /></button>
    <button class="reset-view" disabled={!hasSkin} onclick={() => yaw = -.42} title="Reset player view">Reset view</button>
    <button class="f-icon-button" aria-label="Rotate player right" title="Rotate right" disabled={!hasSkin} onclick={() => yaw += .35}><Icon name="right" size={14} /></button>
  </div>
</div>
<style>
  .player-preview { display: grid; justify-items: center; min-width: 0; position: relative; }
  .player-model { width: 100%; aspect-ratio: 560 / 800; max-height: min(66vh, 640px); display: grid; place-items: center; }
  canvas { display: block; width: min(100%, 480px); max-height: 100%; height: auto; opacity: 0; filter: drop-shadow(0 18px 24px rgb(0 0 0 / 28%)); transition: opacity 420ms ease-out; }
  canvas.revealed { opacity: 1; }
  .player-controls { display: flex; align-items: center; gap: 12px; opacity: 0; transition: opacity var(--f-duration); }
  .player-preview:hover .player-controls, .player-preview:focus-within .player-controls { opacity: 1; }
  .player-controls .f-icon-button { width: 28px; height: 28px; }
  .reset-view { background: none; border: 0; font: inherit; font-size: 11px; color: var(--color-text-secondary); cursor: pointer; }
  @media (max-width: 900px) { .player-preview { display: none; } }
  @media (prefers-reduced-motion: reduce) { .player-controls, canvas { transition: none; } }
</style>
