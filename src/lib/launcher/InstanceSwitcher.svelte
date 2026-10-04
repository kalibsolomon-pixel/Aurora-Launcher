<script lang="ts">
  import { tick } from "svelte";
  import { launcher } from "./store.svelte";
  import Icon from '$lib/shell/Icon.svelte';
  let { compact = false }: { compact?: boolean } = $props();
  const uid = $props.id();
  const instances = $derived(launcher.launcherState?.instances ?? []);
  const selectedId = $derived(launcher.launcherState?.config.selectedInstanceId ?? "");
  const selected = $derived(instances.find(instance => instance.id === selectedId));
  const busy = $derived(launcher.instanceBusy !== null || launcher.playBusy || launcher.createBusy);
  let open = $state(false);
  let current = $state(0);
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;
  function close(restore = false) { open = false; if (restore) trigger?.focus(); }
  async function reveal(index = instances.findIndex(instance => instance.id === selectedId)) {
    current = Math.max(0, index); open = true; await tick();
    document.getElementById(`${uid}-${current}`)?.focus();
  }
  async function choose(id: string) {
    close();
    if (id !== selectedId) await launcher.runSelect(id);
    // Native selection briefly disables the trigger; restore focus after it is usable.
    await tick(); trigger?.focus();
  }
  function key(event: KeyboardEvent) {
    if (event.key === "Escape") { event.preventDefault(); close(true); }
    else if (event.key === "Tab") close();
    else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const next = event.key === "Home" ? 0 : event.key === "End" ? instances.length - 1
        : (current + (event.key === "ArrowDown" ? 1 : -1) + instances.length) % instances.length;
      void reveal(next);
    } else if (event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey && event.key !== " ") {
      const matches = instances.map((instance, index) => ({instance, index})).filter(({instance}) => instance.displayName.toLowerCase().startsWith(event.key.toLowerCase()));
      const match = matches.find(({index}) => index > current) ?? matches[0];
      if (match) { event.preventDefault(); void reveal(match.index); }
    }
  }
</script>
<svelte:window onpointerdown={(event) => { if (open && !root?.contains(event.target as Node)) close(); }} />
<div class="instance-switcher" class:compact bind:this={root}>
  <button bind:this={trigger} type="button" class="picker-trigger" aria-label="Select Play instance"
    aria-haspopup="listbox" aria-expanded={open} aria-controls={`${uid}-list`} disabled={busy || !instances.length}
    onclick={() => open ? close() : void reveal()}
    onkeydown={(event) => { if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); void reveal(); } }}>
    <span>{selected?.displayName ?? "Choose an instance"}</span><span class="chevron" class:expanded={open}><Icon name="chevron" size={24} /></span>
  </button>
  <div id={`${uid}-list`} class="picker-menu" role="listbox" tabindex="-1" aria-label="Play instances" hidden={!open} onkeydown={key}>
    {#each instances as instance, index (instance.id)}
      <button id={`${uid}-${index}`} type="button" role="option" aria-selected={instance.id === selectedId}
        tabindex={open && current === index ? 0 : -1} class="picker-option" disabled={busy}
        onfocus={() => current = index} onclick={() => void choose(instance.id)}>
        <span class="option-identity"><strong>{instance.displayName}</strong>
          <small>{instance.pack ? `${instance.pack.name} ${instance.pack.packVersion} · ` : ""}Minecraft {instance.minecraftVersion} · {instance.platform.kind === "vanilla" ? "Vanilla" : instance.platform.kind === "neoForge" ? "NeoForge" : "Fabric"}{instance.aurora ? " · Aurora configured" : ""}</small>
        </span><span class="selected-marker">{#if instance.id === selectedId}<Icon name="check" size={16} /><span>Selected</span>{/if}</span>
      </button>
    {/each}
  </div>
</div>
<style>
  .instance-switcher { position: relative; min-width: 0; }
  .picker-trigger { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); width: 100%; min-width: 0; min-height: 48px; border: 1px solid var(--f-edge); border-radius: var(--f-radius-control); padding: 8px 12px; background: rgb(255 255 255 / 3%); color: var(--color-text); font: inherit; font-size: 16px; font-weight: 600; text-align: left; cursor: pointer; transition: background var(--f-fast), border-color var(--f-fast); }
  .picker-trigger > span:first-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .picker-trigger:hover:not(:disabled), .picker-trigger[aria-expanded="true"] { background: rgb(255 255 255 / 7%); border-color: var(--f-edge); }
  .picker-trigger:disabled { opacity: .55; cursor: default; }
  .chevron { display: grid; place-items: center; width: 32px; height: 32px; flex: none; color: var(--color-text-secondary); transition: transform var(--f-duration) var(--f-ease); }
  .chevron.expanded { transform: rotate(180deg); }
  .picker-menu { position: absolute; z-index: 20; top: calc(100% + var(--space-2)); left: 0; width: 100%; max-height: min(360px, 50dvh); overflow-y: auto; padding: var(--space-2); border: 1px solid var(--f-edge); border-radius: var(--f-radius-panel); background: var(--f-dialog-panel); background-color: var(--color-surface-raised); backdrop-filter: var(--f-blur); box-shadow: var(--f-shadow); }
  .picker-menu[hidden] { display: none; }
  .picker-option { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); width: 100%; min-height: 60px; padding: var(--space-3); border: 0; border-radius: var(--f-radius-control); background: transparent; color: var(--color-text); font: inherit; text-align: left; cursor: pointer; transition: background var(--f-fast); }
  .picker-option + .picker-option { margin-top: var(--space-1); }
  .picker-option:hover { background: rgb(255 255 255 / 7%); }
  .picker-option[aria-selected="true"] { background: var(--color-accent-soft); }
  .option-identity { display: grid; gap: var(--space-1); min-width: 0; }
  .option-identity strong { font-size: var(--text-body); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .option-identity small { color: var(--color-text-secondary); font-size: 12px; line-height: 1.5; }
  .selected-marker { display: flex; align-items: center; gap: 5px; color: var(--color-accent); font-size: 11px; white-space: nowrap; }
  .picker-option:focus-visible, .picker-trigger:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  @media (prefers-reduced-motion: reduce) { .chevron, .picker-trigger, .picker-option { transition: none; } }
</style>
