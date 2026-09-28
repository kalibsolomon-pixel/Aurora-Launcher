<script lang="ts">
  import type { CapePreview } from "$lib/backend";
  let { preview, name }: { preview: CapePreview | undefined; name: string } = $props();
  let canvas: HTMLCanvasElement | undefined = $state();
  $effect(() => {
    if (!canvas || !preview || preview.rgba.length !== preview.width * preview.height * 4) return;
    const target = canvas.getContext("2d");
    const source = document.createElement("canvas");
    source.width = preview.width; source.height = preview.height;
    const sourceContext = source.getContext("2d");
    if (!target || !sourceContext) return;
    sourceContext.putImageData(new ImageData(Uint8ClampedArray.from(preview.rgba), preview.width, preview.height), 0, 0);
    target.imageSmoothingEnabled = false;
    target.clearRect(0, 0, 40, 64);
    const scale = preview.width / 64;
    target.drawImage(source, 12 * scale, scale, 10 * scale, 16 * scale, 0, 0, 40, 64);
  });
</script>
{#if preview}<span role="img" aria-label={`${name} cape preview`}><canvas bind:this={canvas} width="40" height="64" aria-hidden="true"></canvas></span>{:else}<span class="cape-placeholder" aria-hidden="true">◆</span>{/if}
<style>
  span[role="img"], .cape-placeholder { width: 24px; height: 38px; flex: none; border-radius: 3px; background: var(--color-surface-sunken); }
  canvas { display: block; width: 24px; height: 38px; image-rendering: pixelated; object-fit: contain; }
  .cape-placeholder { display: grid; place-items: center; color: var(--color-text-secondary); font-size: 12px; }
</style>
