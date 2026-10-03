<script lang="ts">
  import { onMount } from 'svelte';
  import DiscordSettings from '$lib/launcher/DiscordSettings.svelte';
  import UpdateSettings from '$lib/launcher/UpdateSettings.svelte';
  import { appearance } from '$lib/launcher/appearance.svelte';
  import { homeWidgets } from '$lib/launcher/homeLayout.svelte';
  import { navigation } from '$lib/launcher/navigation.svelte';
  import Icon from '$lib/shell/Icon.svelte';
  const categories = ['Appearance', 'Home', 'Updates', 'Discord & privacy'] as const;
  const category = $derived(navigation.settingsCategory);
  let motionSpeed = $state(50);
  $effect(() => { motionSpeed = appearance.auroraMotionSpeed; });
  let customHex = $state('#8b80ff');
  $effect(() => { if (appearance.accent.type === 'custom') customHex = appearance.accent.hex; });
  onMount(() => { void homeWidgets.load(); });
  function editHome() { homeWidgets.editing = true; navigation.goTo('home'); }
</script>
<div class="page launcher-settings f-pilot">
  <header class="page-header"><div><h2 class="page-title">Settings</h2><p class="page-subtitle">Make yourself at home.</p></div></header>
  <div class="settings-layout">
    <nav class="settings-nav" aria-label="Settings categories">
      {#each categories as item}<button type="button" aria-current={category === item ? 'page' : undefined} onclick={() => navigation.settingsCategory = item}>{item}</button>{/each}
    </nav>
    <div class="settings-body">
      {#if category === 'Appearance'}
        <section class="appearance-surface f-surface" aria-labelledby="appearance-title">
          <div class="section-intro"><h3 id="appearance-title">Appearance</h3><p>Three choices. One atmosphere.</p></div>
          {#if !appearance.state && !appearance.error}<p role="status">Loading appearance…</p>{/if}
          <fieldset class="choice-section"><legend>Theme</legend>
            <div class="theme-grid">
              {#each appearance.state?.themes ?? [] as theme}
                <label class="theme-tile" class:chosen={appearance.theme === theme.id}>
                  <input type="radio" name="theme" value={theme.id} checked={appearance.theme === theme.id} disabled={appearance.busy} onchange={() => appearance.setTheme(theme.id)} />
                  <span class="theme-preview" data-preview={theme.id}><span class="mini-rail"></span><span class="mini-card"></span><span class="mini-play"></span></span>
                  <span class="choice-label">{theme.label}{#if appearance.theme === theme.id}<Icon name="check" size={15} />{/if}</span>
                </label>
              {/each}
            </div>
          </fieldset>
          <fieldset class="choice-section"><legend>Background</legend>
            <div class="background-grid">
              {#each [{id:'simple',label:'Simple'},{id:'borealis',label:'Aurora Borealis'}] as background}
                <label class="background-tile" class:chosen={appearance.background === background.id}>
                  <input type="radio" name="background" checked={appearance.background === background.id} disabled={appearance.busy} onchange={() => appearance.setBackground(background.id as 'simple' | 'borealis')} />
                  <span class="background-preview" class:borealis-preview={background.id === 'borealis'}></span><span class="choice-label">{background.label}{#if appearance.background === background.id}<Icon name="check" size={15} />{/if}</span>
                </label>
              {/each}
            </div>
            <p class="choice-note">Borealis adds gentle motion and frosted surfaces. Motion rests when Aurora is inactive and follows your system’s reduced-motion setting.</p>
          </fieldset>
          {#if appearance.background === 'borealis'}
            <div class="motion-setting">
              <label for="aurora-motion">Aurora motion</label>
              <div class="motion-scale"><span>Slow</span><input id="aurora-motion" type="range" min="0" max="100" step="1" bind:value={motionSpeed} disabled={appearance.busy} aria-valuetext={motionSpeed < 30 ? 'Slow' : motionSpeed > 70 ? 'Fast' : 'Balanced'} onchange={() => appearance.setAuroraMotionSpeed(motionSpeed)} /><span>Fast</span></div>
              <p class="choice-note">Follows your system’s reduced-motion setting.</p>
            </div>
          {/if}
          <fieldset class="choice-section accent-section"><legend>Accent</legend>
            <div class="accent-picker">
              {#each appearance.state?.accents ?? [] as accent}
                {@const selected = appearance.accent.type === 'preset' && appearance.accent.id === accent.id}
                <label class="accent-option" class:chosen={selected}>
                  <input type="radio" name="accent" checked={selected} disabled={appearance.busy} onchange={() => appearance.setAccent({ type: 'preset', id: accent.id })} />
                  <span class="accent-swatch" style:background={accent.hex}>{#if selected}<span class="swatch-check">✓</span>{/if}</span><span>{accent.label}</span>
                </label>
              {/each}
              <!-- The Custom circle opens the system color picker directly;
                   choosing a color applies it immediately. No second row. -->
              <label class="accent-option" class:chosen={appearance.accent.type === 'custom'} title="Choose a custom accent color">
                <input class="custom-picker-input" type="color" aria-label="Custom accent color" value={customHex} disabled={appearance.busy}
                  oninput={(event) => { customHex = event.currentTarget.value; appearance.setAccent({ type: 'custom', hex: customHex }); }} />
                <span class="accent-swatch custom-swatch" style:background={customHex}>{appearance.accent.type === 'custom' ? '' : '+'}</span><span>Custom</span>
              </label>
            </div>
            <p class="choice-note">Selection and primary actions follow your accent. Status colors keep their meaning.</p>
          </fieldset>
          {#if appearance.error}<p class="inline-message inline-message-error" role="alert">{appearance.error.message}</p>{/if}
        </section>
      {:else if category === 'Home'}
        <section aria-labelledby="home-title">
          <div class="section-intro"><h3 id="home-title">Your space to play</h3><p>Arrange widgets directly on Home.</p></div>
          <div class="group"><div class="group-row"><div class="group-row-main"><span class="group-row-title">Edit Home</span><span class="group-row-detail">Add, hide, reorder and resize your widgets.</span></div><button class="btn btn-primary" onclick={editHome}><Icon name="pencil" size={16} /> Edit Home</button></div>
          <div class="group-row"><div class="group-row-main"><span class="group-row-title">Start fresh</span><span class="group-row-detail">Restore the default widget layout.</span></div><button class="btn btn-quiet" disabled={homeWidgets.busy || !homeWidgets.layout} onclick={() => homeWidgets.reset()}>Reset layout</button></div></div>
          {#if homeWidgets.error}<p role="alert" class="inline-message inline-message-error">{homeWidgets.error}</p>{/if}
        </section>
      {:else if category === 'Updates'}
        <UpdateSettings />
      {:else}
        <div class="section-intro"><h3>Discord & privacy</h3><p>You decide what leaves the launcher.</p></div>
        <DiscordSettings />
      {/if}
    </div>
  </div>
  <footer class="settings-footer">Aurora Launcher · MIT licensed · Independent of Microsoft, Mojang Studios and Fabric. Minecraft requires your own entitled account.</footer>
</div>
<style>
  .launcher-settings { width: 100%; max-width: 1120px; margin-inline: auto; }
  .settings-footer { margin-top: 32px; color: var(--color-text-secondary); font-size: 11px; line-height: 1.6; }
  .page-header { margin-bottom: 32px; }
  .settings-layout { display: grid; grid-template-columns: 168px minmax(0, 1fr); gap: 36px; align-items: start; }
  .settings-nav { display: grid; gap: 6px; position: sticky; top: 0; }
  .settings-nav button { border: 0; border-radius: 10px; padding: 12px 14px; background: transparent; color: var(--color-text-secondary); text-align: left; font: inherit; font-size: 13px; cursor: pointer; }
  .settings-nav button:hover { background: var(--color-surface-raised); }
  .settings-nav button[aria-current] { background: var(--color-accent-soft); color: var(--color-text); box-shadow: inset 3px 0 var(--color-accent); }
  .settings-body { min-width: 0; }
  .appearance-surface { padding: 28px; }
  .motion-setting { margin: 0 0 28px; padding-bottom: 26px; border-bottom: 1px solid var(--f-edge); }
  .motion-setting > label { font-size: 14px; font-weight: 600; }
  .motion-scale { display: flex; align-items: center; gap: 18px; margin: 16px 0 8px; font-size: 12px; color: var(--color-text-secondary); }
  .motion-scale input { width: 100%; min-width: 60px; accent-color: var(--color-accent); cursor: pointer; }
  @media (max-width: 760px) { .appearance-surface { padding: 20px; } }

  .section-intro { margin-bottom: 26px; }
  .section-intro h3 { font-size: 20px; font-weight: 600; letter-spacing: -.025em; margin: 0 0 6px; }
  .section-intro p, .choice-note { margin: 0; color: var(--color-text-secondary); font-size: 13px; line-height: 1.6; }
  .choice-section { border: 0; border-bottom: 1px solid var(--f-edge); padding: 0 0 26px; margin: 0 0 26px; min-width: 0; }
  legend { font-size: 14px; font-weight: 600; padding: 0; margin-bottom: 14px; }
  .theme-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  .theme-tile, .background-tile { display: grid; gap: 10px; position: relative; padding: 8px; border: 1px solid var(--f-edge); border-radius: 14px; background: rgb(8 15 25 / 24%); cursor: pointer; min-width: 0; }
  .chosen { border-color: var(--color-accent); }
  .theme-tile:focus-within, .background-tile:focus-within { outline: 2px solid var(--color-accent); outline-offset: 3px; }
  .theme-tile input, .background-tile input, .accent-option input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .theme-preview { display: block; height: 84px; border-radius: 8px; background: #101116; position: relative; overflow: hidden; }
  .theme-preview[data-preview="midnight"] { background: #10192a; }
  .theme-preview[data-preview="oled"] { background: #000; }
  .mini-rail { position: absolute; left: 0; height: 100%; width: 18%; background: rgb(0 0 0 / 30%); border-right: 1px solid rgb(255 255 255 / 6%); }
  .mini-card { position: absolute; left: 28%; top: 22%; width: 60%; height: 56%; border-radius: 6px; background: rgb(255 255 255 / 6%); }
  .mini-play { position: absolute; left: 35%; top: 34%; width: 46%; height: 20%; border-radius: 3px; background: var(--color-accent); opacity: .85; }
  .choice-label { display: flex; justify-content: space-between; align-items: center; font-size: 12px; padding: 0 3px 3px; }
  .background-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px; }
  .background-preview { height: 106px; border-radius: 8px; display: block; background: var(--color-background); }
  .borealis-preview { background: url('/backgrounds/borealis.webp') center 30% / cover; }
  .accent-picker { display: flex; flex-wrap: wrap; gap: 18px; }
  .accent-option { display: grid; justify-items: center; gap: 9px; font-size: 11px; color: var(--color-text-secondary); width: 56px; cursor: pointer; position: relative; border-radius: 8px; }
  .accent-swatch { width: 30px; height: 30px; border-radius: 50%; display: grid; place-items: center; }
  .chosen .accent-swatch { outline: 2px solid var(--color-accent); outline-offset: 4px; }
  .swatch-check { background: #101116; color: white; border-radius: 50%; width: 15px; height: 15px; text-align: center; font-size: 11px; }
  .custom-swatch { color: #08090c; font-size: 20px; display: grid; place-items: center; }
  .custom-picker-input { position: absolute; width: 0; height: 0; opacity: 0; padding: 0; border: 0; pointer-events: none; }
  .accent-option:has(input:focus-visible) { outline: 2px solid var(--color-accent); outline-offset: 3px; }
  .accent-section .choice-note { margin-top: 20px; }
  @media (max-width: 1050px) { .settings-layout { grid-template-columns: 1fr; gap: 26px; } .settings-nav { display: flex; flex-wrap: wrap; position: static; } }
  @media (max-width: 760px) { .theme-preview { height: 65px; } .theme-grid { gap: 8px; } .choice-label { font-size: 11px; } }
</style>
