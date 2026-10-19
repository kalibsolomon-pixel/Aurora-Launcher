<script lang="ts">
  import { onMount } from 'svelte';
  import { launcher } from './store.svelte';
  import { accountManager } from './accountManager.svelte';
  import Icon from '$lib/shell/Icon.svelte';
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
  {#if hasSkin}
    <div class="player-model" role="img" aria-label="Current Minecraft player skin">
      <canvas bind:this={canvas} width={PLAYER_RENDER_WIDTH} height={PLAYER_RENDER_HEIGHT} aria-hidden="true" class:revealed></canvas>
    </div>
  {:else}
    <div class="player-model" aria-label={launcher.selectedAccount ? 'Player skin unavailable' : 'Sign in to preview your Minecraft skin'} role="group">
      <div class="player-placeholder">
        <span class="placeholder-mark" aria-hidden="true"><Icon name="cosmetics" size={26} /></span>
        <p>{launcher.selectedAccount ? 'Your skin will appear here when available.' : 'Sign in to preview your Minecraft skin.'}</p>
        {#if !launcher.selectedAccount}<button type="button" class="btn btn-quiet" onclick={() => accountManager.show()}>Sign in</button>{/if}
      </div>
    </div>
  {/if}

</div>
<style>
  .player-preview { min-width: 0; position: relative; height: 320px; display: grid; place-items: center; }
  .player-preview.has-skin { position: absolute; top: 0; bottom: 0; right: 0; width: 50%; height: auto; overflow: hidden; border-radius: 0 var(--f-radius-panel) var(--f-radius-panel) 0; pointer-events: none; }
  .player-preview.has-skin::before { content: ''; position: absolute; inset: 0; background: radial-gradient(ellipse at 50% 40%, color-mix(in srgb, var(--color-accent) 12%, transparent), transparent 68%); }
  .player-model { width: 100%; display: grid; place-items: center; }
  .has-skin .player-model { position: absolute; top: 0; }
  canvas { display: block; width: auto; height: clamp(512px, 68vh, 592px); max-width: none; image-rendering: auto; opacity: 0; transition: opacity 420ms ease-out; }
  canvas.revealed { opacity: 1; }
  .player-placeholder { display: grid; justify-items: center; gap: var(--space-3); max-width: 24ch; text-align: center; }
  .player-placeholder p { margin: 0; font-size: var(--text-secondary); line-height: 1.55; color: var(--color-text-secondary); }
  .placeholder-mark { display: grid; place-items: center; width: 56px; height: 56px; border-radius: 50%; color: var(--color-text-muted); background: rgb(255 255 255 / 4%); box-shadow: inset 0 0 0 1px var(--f-edge); }
  @media (min-width: 801px) and (max-width: 1050px) { canvas { height: 512px; } }
  @media (max-width: 800px) { .player-preview { display: none; } }
  @media (prefers-reduced-motion: reduce) { canvas { transition: none; } }
</style>
