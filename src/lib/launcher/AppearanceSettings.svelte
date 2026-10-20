<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { appearance } from './appearance.svelte';
  import SettingsFeedback from './SettingsFeedback.svelte';
  import { getAppearance, LauncherBackendError } from '$lib/backend';
  import Icon from '$lib/shell/Icon.svelte';
  import Tooltip from '$lib/shell/Tooltip.svelte';
  let motionSpeed = $state(untrack(() => appearance.auroraMotionSpeed));
  let customHex = $state(untrack(() => appearance.accent.type === 'custom' ? appearance.accent.hex : '#8b80ff'));
  let themeChoice = $state(untrack(() => appearance.theme)), backgroundChoice = $state(untrack(() => appearance.background));
  let accentChoice = $state(untrack(() => appearance.accent.type === 'preset' ? appearance.accent.id : ''));
  let saved = $state(false);
  const loadAppearance = async () => {
    appearance.busy = true;
    try { appearance.apply(await getAppearance()); }
    catch (cause) { appearance.error = cause instanceof LauncherBackendError ? cause : new LauncherBackendError('backend_unavailable', 'The launcher appearance could not be loaded.'); }
    finally { appearance.busy = false; }
  };
  let retryOperation: () => Promise<void> = loadAppearance;
  $effect(() => { motionSpeed = appearance.auroraMotionSpeed; });
  $effect(() => { if (appearance.accent.type === 'custom') customHex = appearance.accent.hex; });
  $effect(() => { themeChoice = appearance.theme; backgroundChoice = appearance.background; accentChoice = appearance.accent.type === 'preset' ? appearance.accent.id : ''; });
  async function save(operation: () => Promise<void>) {
    if (appearance.busy) return;
    retryOperation = operation; saved = false;
    await operation(); await tick();
    saved = !appearance.error && operation !== loadAppearance;
    themeChoice = appearance.theme; backgroundChoice = appearance.background; accentChoice = appearance.accent.type === 'preset' ? appearance.accent.id : '';
  }
</script>
<section class="appearance-surface f-surface" aria-labelledby="appearance-title">
          <div class="section-intro"><h3 id="appearance-title">Appearance</h3></div>
          {#if !appearance.state && !appearance.error}<p role="status">Loading appearance…</p>{/if}
          <ul class="appearance-summary" aria-label="Current appearance"><li>Theme: {appearance.state?.themes.find(item => item.id === appearance.theme)?.label ?? appearance.theme}</li><li>Background: {appearance.background === 'borealis' ? 'Aurora Borealis' : 'Simple'}</li><li>Accent: {appearance.accent.type === 'custom' ? appearance.accent.hex : appearance.state?.accents.find(item => appearance.accent.type === 'preset' && item.id === appearance.accent.id)?.label ?? appearance.accent.id}</li></ul>
          <fieldset class="choice-section" disabled={!appearance.state}><legend>Theme</legend>
            <div class="theme-grid">
              {#each appearance.state?.themes ?? [] as theme}
                <label class="theme-tile" class:chosen={appearance.theme === theme.id}>
                  <input type="radio" name="theme" value={theme.id} bind:group={themeChoice} disabled={appearance.busy} onchange={() => save(() => appearance.setTheme(theme.id))} />
                  <span class="theme-preview" data-preview={theme.id}><span class="mini-rail"></span><span class="mini-card"></span><span class="mini-play"></span></span>
                  <span class="choice-label">{theme.label}{#if appearance.theme === theme.id}<Icon name="check" size={15} />{/if}</span>
                </label>
              {/each}
            </div>
          </fieldset>
          <fieldset class="choice-section" disabled={!appearance.state}><legend>Background <Tooltip label="About background">Borealis adds gentle motion and frosted surfaces. Motion rests when Aurora is inactive and follows your system’s reduced-motion setting.</Tooltip></legend>
            <div class="background-grid">
              {#each [{id:'simple',label:'Simple'},{id:'borealis',label:'Aurora Borealis'}] as background}
                <label class="background-tile" class:chosen={appearance.background === background.id}>
                  <input type="radio" name="background" value={background.id} bind:group={backgroundChoice} disabled={appearance.busy} onchange={() => save(() => appearance.setBackground(background.id as 'simple' | 'borealis'))} />
                  <span class="background-preview" class:borealis-preview={background.id === 'borealis'}></span><span class="choice-label">{background.label}{#if appearance.background === background.id}<Icon name="check" size={15} />{/if}</span>
                </label>
              {/each}
            </div>

          </fieldset>
          {#if appearance.background === 'borealis'}
            <div class="motion-setting">
              <div class="motion-heading"><span><label for="aurora-motion">Aurora motion</label> <Tooltip label="About Aurora motion">Playback speed from slow to fast; 0 is the slowest speed. Pauses when Aurora is inactive. Reduced motion in your system keeps Borealis still.</Tooltip></span><output for="aurora-motion">{motionSpeed} / 100</output></div>
              <div class="motion-scale"><span>Slow</span><input id="aurora-motion" type="range" min="0" max="100" step="1" bind:value={motionSpeed} disabled={appearance.busy} aria-valuetext={motionSpeed < 30 ? 'Slow' : motionSpeed > 70 ? 'Fast' : 'Balanced'} onchange={() => { const speed = motionSpeed; void save(() => appearance.setAuroraMotionSpeed(speed)); }} /><span>Fast</span></div>

            </div>
          {/if}
          <fieldset class="choice-section accent-section" disabled={!appearance.state}><legend>Accent <Tooltip label="About accent">Selection, focus and primary actions follow your accent across the launcher. Status colors keep their meaning. Custom colors must stay visible on dark surfaces.</Tooltip></legend>
            <div class="accent-picker">
              {#each appearance.state?.accents ?? [] as accent}
                {@const selected = appearance.accent.type === 'preset' && appearance.accent.id === accent.id}
                <label class="accent-option" class:chosen={selected}>
                  <input type="radio" name="accent" value={accent.id} bind:group={accentChoice} disabled={appearance.busy} onchange={() => save(() => appearance.setAccent({ type: 'preset', id: accent.id }))} />
                  <span class="accent-swatch" style:background={accent.hex}>{#if selected}<span class="swatch-check">✓</span>{/if}</span><span>{accent.label}</span>
                </label>
              {/each}
              <!-- The Custom circle opens the system color picker directly;
                   choosing a color applies it immediately. No second row. -->
              <label class="accent-option" class:chosen={appearance.accent.type === 'custom'} title="Choose a custom accent color">
                <input class="custom-picker-input" type="color" aria-label="Custom accent color" value={customHex} disabled={appearance.busy}
                  onchange={(event) => { customHex = event.currentTarget.value; const hex = customHex; void save(() => appearance.setAccent({ type: 'custom', hex })); }} />
                <span class="accent-swatch custom-swatch" style:background={customHex}>{#if appearance.accent.type === 'custom'}<span class="swatch-check">✓</span>{:else}+{/if}</span><span>Custom</span>
              </label>
            </div>

          </fieldset>
          <SettingsFeedback busy={appearance.busy} error={appearance.error?.message ?? ''} {saved} retryLabel={appearance.state ? 'Retry save' : 'Retry loading'} retry={() => void save(retryOperation)} />
        </section>
<style>
  .appearance-surface { padding: 24px; }
  .motion-setting { margin: 0 0 28px; padding-bottom: 26px; border-bottom: 1px solid var(--f-edge); }
  .motion-heading label { font-size: .875rem; font-weight: 600; }
  .motion-scale { display: flex; align-items: center; gap: 18px; margin: 16px 0 8px; font-size: .75rem; color: var(--color-text-secondary); }
  .motion-scale input { width: 100%; min-width: 60px; accent-color: var(--color-accent); cursor: pointer; }
  @media (max-width: 760px) { .appearance-surface { padding: 20px; } }


  .section-intro h3 { font-size: 20px; font-weight: 600; letter-spacing: -.025em; margin: 0 0 6px; }
  .choice-section { border: 0; border-bottom: 1px solid var(--f-edge); padding: 0 0 26px; margin: 0 0 26px; min-width: 0; }
  legend { font-size: .875rem; font-weight: 600; padding: 0; margin-bottom: 14px; }
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
  .choice-label { display: flex; justify-content: space-between; align-items: center; font-size: .75rem; padding: 0 3px 3px; }
  .background-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px; }
  .background-preview { height: 106px; border-radius: 8px; display: block; background: var(--color-background); }
  .borealis-preview { background: url('/backgrounds/borealis.webp') center 30% / cover; }
  .accent-picker { display: flex; flex-wrap: wrap; gap: 18px; }
  .accent-option { display: grid; justify-items: center; gap: 9px; font-size: .6875rem; color: var(--color-text-secondary); width: 56px; cursor: pointer; position: relative; border-radius: 8px; }
  .accent-swatch { width: 30px; height: 30px; border-radius: 50%; display: grid; place-items: center; }
  .chosen .accent-swatch { outline: 2px solid var(--color-accent); outline-offset: 4px; }
  .swatch-check { background: #101116; color: white; border-radius: 50%; width: 15px; height: 15px; text-align: center; font-size: .6875rem; }
  .custom-swatch { color: #08090c; font-size: 20px; display: grid; place-items: center; }
  .custom-picker-input { position: absolute; width: 0; height: 0; opacity: 0; padding: 0; border: 0; pointer-events: none; }
  .accent-option:has(input:focus-visible) { outline: 2px solid var(--color-accent); outline-offset: 3px; }
  @media (max-width: 760px) { .theme-grid { grid-template-columns: 1fr; } .theme-tile { grid-template-columns: 80px minmax(0, 1fr); align-items: center; } .theme-preview { height: 55px; } .background-preview { height: 76px; } }
  .appearance-summary { display: flex; flex-wrap: wrap; gap: 8px 20px; margin: 0 0 20px; padding: 0; list-style: none; color: var(--color-text-secondary); font-size: .8125rem; }
  .motion-setting output { font-variant-numeric: tabular-nums; font-size: .8125rem; color: var(--color-text-secondary); }
  .accent-option .custom-picker-input { inset: 0; width: 100%; height: 100%; cursor: pointer; pointer-events: auto; z-index: 1; }
  .accent-option .custom-picker-input:disabled { cursor: wait; }
  .motion-heading { display: flex; justify-content: space-between; gap: 12px; flex-wrap: wrap; }

</style>
