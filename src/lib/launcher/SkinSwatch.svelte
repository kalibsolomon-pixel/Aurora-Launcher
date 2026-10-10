<script lang="ts">
  import { onMount } from 'svelte';
  import type { SkinPreset, HeadAvatar } from '$lib/backend';
  import { savedSkinTexture } from './skinTextures';
  import { playerModel, renderPlayer } from './playerModel';
  let { preset }: { preset: SkinPreset } = $props();
  let canvas: HTMLCanvasElement | undefined = $state();
  let host: HTMLSpanElement | undefined = $state();
  let visible = $state(false), texture = $state<HeadAvatar | null>(null), failed = $state(false);
  $effect(() => {
    const entry = preset;
    if (!visible) return;
    let live = true; texture = null; failed = false;
    void savedSkinTexture(entry).then(value => { if (live) texture = value; }).catch(() => { if (live) failed = true; });
    return () => { live = false; };
  });
  $effect(() => { if (canvas && texture && visible) { const context = canvas.getContext('2d'); if(context) renderPlayer(context,playerModel(texture),-.25,140,200); } });
  onMount(() => {
    const observer = new IntersectionObserver(entries => { visible=entries[0].isIntersecting; });
    if (host) observer.observe(host);
    return () => observer.disconnect();
  });
</script>
<span class="skin-swatch" bind:this={host} role="img" aria-label={`${preset.name} skin preview${failed ? ', texture unavailable' : ''}`}>
  {#if texture}<canvas bind:this={canvas} width="140" height="200" aria-hidden="true"></canvas>{:else}<span class="placeholder">{failed ? 'Texture unavailable' : 'Loading preview…'}</span>{/if}
</span>
<style>
  .skin-swatch { height:160px; display:grid; place-items:center; width:100%; background:radial-gradient(ellipse at 50% 100%,var(--color-accent-soft),transparent 70%); border-radius:12px; overflow:hidden; }
  canvas { display:block; height:160px; width:112px; object-fit:contain; }
  .placeholder { font-size:11px; color:var(--color-text-muted); text-align:center; padding:10px; }
</style>
