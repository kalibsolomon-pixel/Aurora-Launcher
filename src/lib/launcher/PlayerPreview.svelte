<script lang="ts">
  import { launcher } from "./store.svelte";
  import { playerModel, renderPlayer } from "./playerModel";
  let canvas: HTMLCanvasElement | undefined = $state();
  const model = $derived(playerModel(launcher.selectedAccount ? launcher.accountAvatars[launcher.selectedAccount.accountId] : null));
  $effect(() => {
    const context = canvas?.getContext("2d");
    if (context) renderPlayer(context, model);
  });
</script>

<div class="player-preview" role="img" aria-label={model.fallback ? "Default Minecraft player" : "Current Minecraft player skin"}>
  <canvas bind:this={canvas} width="420" height="600" aria-hidden="true"></canvas>
</div>

<style>
  .player-preview { display: grid; place-items: center; min-width: 0; pointer-events: none; position: relative; }
  .player-preview::after { content: ""; width: 55%; height: 24px; position: absolute; bottom: 5%; border-radius: 50%; background: rgb(0 0 0 / 30%); filter: blur(10px); z-index: -1; }
  canvas { display: block; width: min(100%, 420px); height: auto; filter: drop-shadow(0 14px 18px rgb(0 0 0 / 25%)); }
  @media (max-width: 1050px) { .player-preview { display: none; } }
</style>
