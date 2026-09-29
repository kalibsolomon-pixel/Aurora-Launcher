<script lang="ts">
  import { onMount } from 'svelte';
  import { appearance } from '$lib/launcher/appearance.svelte';
  let active = $state(false);
  let failed = $state(false);
  onMount(() => {
    const update = () => { active = !document.hidden && document.hasFocus(); };
    update();
    document.addEventListener('visibilitychange', update);
    window.addEventListener('focus', update);
    window.addEventListener('blur', update);
    return () => {
      document.removeEventListener('visibilitychange', update);
      window.removeEventListener('focus', update);
      window.removeEventListener('blur', update);
    };
  });
</script>
{#if appearance.background === 'borealis' && !failed}
  <div class="aurora-background" class:background-active={active} aria-hidden="true">
    <img src="/backgrounds/borealis.webp" alt="" onerror={() => failed = true} />
  </div>
{/if}
