<script lang="ts">
  import { onMount } from 'svelte';
  import SettingsRow from './SettingsRow.svelte';
  let { openAppearance }: { openAppearance: () => void } = $props();
  let motion = $state<boolean | null>(null), transparency = $state<boolean | null>(null), contrast = $state<boolean | null>(null);
  onMount(() => {
    const queries = [matchMedia('(prefers-reduced-motion: reduce)'), matchMedia('(prefers-reduced-transparency: reduce)'), matchMedia('(forced-colors: active)')];
    const update = () => { [motion, transparency, contrast] = queries.map(query => query.matches); };
    update(); queries.forEach(query => query.addEventListener('change', update));
    return () => queries.forEach(query => query.removeEventListener('change', update));
  });
</script>
<div class="section-intro"><h3>Accessibility</h3></div>
<section class="group" aria-label="System visual preferences">
  <SettingsRow id="system-motion" title="Reduced motion" description="Keeps Borealis still and removes nonessential control transitions when your system requests reduced motion.">
    <span class="setting-state" role="status">{motion === null ? 'Reading system…' : motion ? 'Following system · Reduced' : 'Following system · No reduction requested'}</span>
  </SettingsRow>
  <SettingsRow id="system-transparency" title="Reduced transparency" description="Uses opaque panels and solid primary controls when your system requests reduced transparency.">
    <span class="setting-state" role="status">{transparency === null ? 'Reading system…' : transparency ? 'Following system · Reduced' : 'Following system · No reduction requested'}</span>
  </SettingsRow>
  <SettingsRow id="system-contrast" title="System contrast" description="Your browser’s forced-colors mode uses system colors. Selected choices also use borders, checkmarks and text.">
    <span class="setting-state" role="status">{contrast === null ? 'Reading system…' : contrast ? 'System colors active' : 'Standard colors'}</span>
  </SettingsRow>
</section>
<section class="group" aria-label="Appearance access">
  <SettingsRow id="comfort-appearance" title="Theme & accent" description="Choose darker OLED surfaces or a visible selection color using the existing launcher appearance controls.">
    <button type="button" class="btn" onclick={openAppearance}>Open launcher Appearance</button>
  </SettingsRow>
</section>
