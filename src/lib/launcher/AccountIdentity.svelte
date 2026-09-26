<script lang="ts">
  import { launcher } from "./store.svelte";
  import type { AccountSummary } from "../backend";
  let { account, small = false }: { account: AccountSummary | null; small?: boolean } = $props();
  let canvas: HTMLCanvasElement | undefined = $state();
  const avatar = $derived(account ? launcher.accountAvatars[account.accountId] : null);
  const valid = $derived(avatar?.rgba.length === 256 && avatar.rgba.every(value => Number.isInteger(value) && value >= 0 && value <= 255));
  $effect(() => {
    if (canvas && valid && avatar) {
      canvas.getContext("2d")?.putImageData(new ImageData(new Uint8ClampedArray(avatar.rgba), 8, 8), 0, 0);
    }
  });
</script>

<span class="account-identity" class:small>
  {#if valid}
    <span role="img" aria-label="Minecraft head for {account?.minecraftName}"><canvas bind:this={canvas} width="8" height="8" aria-hidden="true"></canvas></span>
  {:else}
    <span class="avatar-fallback" role="img" aria-label="Default account avatar">{account?.minecraftName.charAt(0).toUpperCase() ?? "?"}</span>
  {/if}
  <span class="identity-text">
    <span class="identity-name" title={account?.minecraftName}>{account?.minecraftName ?? "Not signed in"}</span>
    <span class="identity-detail">{!account ? "Sign in to play Minecraft" : account.status === "reauthenticationRequired" ? "Sign in again" : "Minecraft account"}</span>
  </span>
</span>

<style>
  .account-identity { display: flex; flex: 1; align-items: center; gap: var(--space-3); min-width: 0; }
  canvas, .avatar-fallback { width: 40px; height: 40px; flex: none; border-radius: var(--radius-sm); image-rendering: pixelated; }
  .avatar-fallback { display: grid; place-items: center; background: var(--color-surface-raised); color: var(--color-text-secondary); font-weight: 600; }
  .identity-text { display: grid; gap: 2px; min-width: 0; }
  .identity-name { color: var(--color-text); font-size: var(--text-body); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .identity-detail { color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .small { gap: var(--space-2); }
  .small canvas, .small .avatar-fallback { width: 28px; height: 28px; }
  .small .identity-detail { font-size: var(--text-label); }
</style>
