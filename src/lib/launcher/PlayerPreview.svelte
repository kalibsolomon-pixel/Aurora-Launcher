<script lang="ts">
  import { launcher } from "./store.svelte";
  import { playerPixels } from "./playerPixels";
  let canvas: HTMLCanvasElement | undefined = $state();
  const pixels = $derived(playerPixels(launcher.selectedAccount ? launcher.accountAvatars[launcher.selectedAccount.accountId] : null));
  $effect(() => {
    if (canvas && pixels) canvas.getContext("2d")?.putImageData(new ImageData(new Uint8ClampedArray(pixels), 20, 32), 0, 0);
  });
</script>

<div class="player-preview" role="img" aria-label={pixels ? "Current Minecraft player skin" : "Default Minecraft player"}>
  {#if pixels}<canvas bind:this={canvas} width="20" height="32" aria-hidden="true"></canvas>
  {:else}
    <svg viewBox="0 0 20 32" aria-hidden="true"><path fill="#ad8067" d="M6 0h8v8H6zM2 8h4v12H2zM14 8h4v12h-4z"/><path fill="#493529" d="M6 0h8v2H6zM6 2h2v2H6zM12 2h2v2h-2z"/><path fill="#293543" d="M6 20h8v12H6z"/><path fill="#40535d" d="M6 8h8v12H6zM2 8h4v4H2zM14 8h4v4h-4z"/><path fill="#23303b" d="M6 30h8v2H6z"/><path fill="#ece3db" d="M8 4h1v1H8zM11 4h1v1h-1z"/></svg>
  {/if}
</div>

<style>
  .player-preview { display: grid; place-items: center; align-self: stretch; min-width: 160px; padding: var(--space-4); background: radial-gradient(ellipse at 50% 80%, var(--color-surface-raised), transparent 70%); }
  canvas, svg { width: 160px; height: 256px; image-rendering: pixelated; filter: drop-shadow(0 12px 12px rgb(0 0 0 / 25%)); }
  @media (max-width: 900px) { .player-preview { display: none; } }
</style>
