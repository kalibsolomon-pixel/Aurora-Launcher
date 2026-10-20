<script lang="ts">
  import { onMount, tick } from 'svelte';
  import DiscordSettings from '$lib/launcher/DiscordSettings.svelte';
  import UpdateSettings from '$lib/launcher/UpdateSettings.svelte';
  import GeneralSettings from '$lib/launcher/GeneralSettings.svelte';
  import HomeWidgetSettings from '$lib/launcher/HomeWidgetSettings.svelte';
  import AppearanceSettings from '$lib/launcher/AppearanceSettings.svelte';
  import AccessibilitySettings from '$lib/launcher/AccessibilitySettings.svelte';
  import AboutSettings from '$lib/launcher/AboutSettings.svelte';
  import { navigation } from '$lib/launcher/navigation.svelte';
  import { settingsCategories, categoryAtKey, type SettingsCategory } from '$lib/launcher/settings';
  import '$lib/launcher/settings.css';
  const category = $derived(navigation.settingsCategory);
  let categoryNav: HTMLElement;
  onMount(() => {
    const main = document.querySelector('main.content');
    if (main) main.scrollTop = navigation.settingsScroll[category] ?? 0;
    return () => { if (main) navigation.settingsScroll[category] = main.scrollTop; };
  });
  async function selectCategory(next: SettingsCategory) {
    const main = document.querySelector('main.content');
    if (next === category) return;
    if (main) navigation.settingsScroll[category] = main.scrollTop;
    navigation.settingsCategory = next;
    await tick();
    if (main) main.scrollTop = navigation.settingsScroll[next] ?? 0;
  }
  async function categoryKey(event: KeyboardEvent) {
    const next = categoryAtKey(category, event.key);
    if (!next) return;
    event.preventDefault(); await selectCategory(next);
    categoryNav.querySelector<HTMLButtonElement>(`[data-category="${next}"]`)?.focus({ preventScroll: true });
  }
</script>
<div class="page launcher-settings f-pilot">
  <header class="page-header"><div><h2 class="page-title">Settings</h2></div></header>
  <div class="settings-layout">
    <nav bind:this={categoryNav} class="settings-nav" aria-label="Settings categories">
      {#each settingsCategories as item}
        <button type="button" data-category={item.id} aria-current={category === item.id ? 'page' : undefined}
          aria-controls="settings-content" onclick={() => selectCategory(item.id)} onkeydown={categoryKey}>{item.label}</button>
      {/each}
    </nav>
    <section class="settings-body" id="settings-content" aria-label={`${settingsCategories.find(item => item.id === category)?.label ?? category} settings`}>
      {#if category === 'General'}
        <div class="section-intro"><h3>Launcher</h3></div>
        <GeneralSettings />
      {:else if category === 'Appearance'}
        <AppearanceSettings />
      {:else if category === 'Home'}
        <div class="section-intro"><h3>Home &amp; Widgets</h3></div>
        <HomeWidgetSettings />
      {:else if category === 'Updates'}
        <UpdateSettings />
      {:else if category === 'Discord'}
        <DiscordSettings />
      {:else if category === 'Accessibility'}
        <AccessibilitySettings openAppearance={() => void selectCategory('Appearance')} />
      {:else if category === 'About'}
        <AboutSettings />
      {/if}
    </section>
  </div>
</div>
