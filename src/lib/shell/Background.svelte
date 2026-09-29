<script lang="ts">
  import { getContext, untrack } from 'svelte';
  import { appearance } from '$lib/launcher/appearance.svelte';
  import { mountBorealis, motionRate, type BorealisReview } from './borealis';
  const review = getContext<BorealisReview | undefined>('borealis-review');
  let video = $state<HTMLVideoElement>();
  $effect(() => {
    if (video) return untrack(() => mountBorealis(video!, review));
  });
  $effect(() => { if (video) video.playbackRate = motionRate(appearance.auroraMotionSpeed); });
</script>
{#if appearance.background === 'borealis'}
  <div class="aurora-background" aria-hidden="true">
    <img src="/backgrounds/borealis.webp" alt="" />
    <video bind:this={video} muted loop playsinline preload="none" tabindex="-1"></video>
  </div>
{/if}
