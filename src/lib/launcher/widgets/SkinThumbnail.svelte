<script module lang="ts">
  // One bounded native fetch per saved skin per session; static head crops only.
  const thumbnails = new Map<string, number[] | null>();
</script>
<script lang="ts">
  import { skinPresetThumbnail } from "$lib/backend";
  let { presetId, name }: { presetId: string; name: string } = $props();
  let rgba = $state<number[] | null>(null);
  let canvas: HTMLCanvasElement | undefined = $state();
  $effect(() => {
    const id = presetId;
    let current = true;
    rgba = null;
    if (thumbnails.has(id)) {
      rgba = thumbnails.get(id) ?? null;
    } else {
      void skinPresetThumbnail(id).then(thumbnail => {
        const value = thumbnail?.rgba.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255) ? thumbnail.rgba : null;
        thumbnails.set(id, value);
        if (current) rgba = value;
      }).catch(() => {
        thumbnails.set(id, null);
      });
    }
    return () => { current = false; };
  });
  $effect(() => {
    if (canvas && rgba?.length === 256) {
      canvas.getContext("2d")?.putImageData(new ImageData(new Uint8ClampedArray(rgba), 8, 8), 0, 0);
    }
  });
</script>
{#if rgba?.length === 256}
  <span role="img" aria-label={`Saved skin thumbnail for ${name}`}><canvas bind:this={canvas} width="8" height="8" aria-hidden="true"></canvas></span>
{:else}
  <span class="thumb-fallback" role="img" aria-label={`Saved skin ${name}`}></span>
{/if}
<style>
  canvas { display: block; width: 100%; height: 100%; image-rendering: pixelated; }
  span { display: block; width: 28px; height: 28px; flex: none; border-radius: var(--radius-sm); overflow: hidden; }
  .thumb-fallback { background: var(--color-surface-raised); }
</style>
