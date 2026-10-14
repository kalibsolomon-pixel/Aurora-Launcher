<script lang="ts">
  import { onMount } from 'svelte';
  import { launcher } from './store.svelte';
  import { playerModel, renderPlayer, PLAYER_DEFAULT_YAW, PLAYER_RENDER_WIDTH, PLAYER_RENDER_HEIGHT } from './playerModel';
  let canvas: HTMLCanvasElement | undefined = $state();
  let visible = $state(false);
  let revealed = $state(false);
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
    const currentModel = model, angle = PLAYER_DEFAULT_YAW;
    if (!visible || !canvas || !hasSkin) return;
    const context = canvas.getContext('2d');
    if (context) renderPlayer(context, currentModel, angle);
  });
  onMount(() => {
    const width = matchMedia('(min-width: 801px)');
    const update = () => { visible = !document.hidden && width.matches; };
    update(); document.addEventListener('visibilitychange', update); width.addEventListener('change', update);
    return () => { document.removeEventListener('visibilitychange', update); width.removeEventListener('change', update); };
  });
</script>
<div class="player-preview" class:has-skin={hasSkin}>
  <!-- Fixed Home artwork; reusable renderer orientation remains available elsewhere. -->
  <div class="player-model" role="img" aria-label={hasSkin ? 'Current Minecraft player skin' : launcher.selectedAccount ? 'Player skin unavailable' : 'Sign in to preview your Minecraft skin'}>
    {#if hasSkin}
      <canvas bind:this={canvas} width={PLAYER_RENDER_WIDTH} height={PLAYER_RENDER_HEIGHT} aria-hidden="true" class:revealed></canvas>
    {:else}
      <p class="player-placeholder">{launcher.selectedAccount ? 'Your skin will appear here when available.' : 'Sign in to preview your Minecraft skin.'}</p>
    {/if}
  </div>

</div>
<style>
  .player-preview { min-width: 0; position: relative; height: 320px; display: grid; place-items: center; }
  .player-preview.has-skin { position: absolute; top: 0; bottom: 0; right: 0; width: 50%; height: auto; overflow: hidden; border-radius: 0 var(--f-radius-panel) var(--f-radius-panel) 0; pointer-events: none; }
  .player-preview.has-skin::before { content: ''; position: absolute; inset: 0; background: radial-gradient(ellipse at 50% 40%, color-mix(in srgb, var(--color-accent) 12%, transparent), transparent 68%); }
  .player-model { width: 100%; display: grid; place-items: center; }
  .has-skin .player-model { position: absolute; top: 0; }
  canvas { display: block; width: auto; height: clamp(512px, 68vh, 592px); max-width: none; image-rendering: auto; opacity: 0; transition: opacity 420ms ease-out; }
  canvas.revealed { opacity: 1; }
  .player-placeholder { max-width: 20ch; margin: 0; text-align: center; font-size: var(--text-metadata); line-height: 1.7; color: var(--color-text-secondary); }
  @media (min-width: 801px) and (max-width: 1050px) { canvas { height: 512px; } }
  @media (max-width: 800px) { .player-preview { display: none; } }
  @media (prefers-reduced-motion: reduce) { canvas { transition: none; } }
</style>
