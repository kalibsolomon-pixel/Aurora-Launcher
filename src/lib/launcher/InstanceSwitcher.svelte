<script lang="ts">
  import { tick } from "svelte";
  import { launcher } from "./store.svelte";
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
  async function choose(id: string) { close(true); if (id !== selectedId) await launcher.runSelect(id); }
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
    aria-haspopup="listbox" aria-expanded={open} aria-controls={`${uid}-list`} disabled={busy}
    onclick={() => open ? close() : void reveal()}
    onkeydown={(event) => { if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); void reveal(); } }}>
    <span>{selected?.displayName ?? "Choose an instance"}</span><span class="chevron" aria-hidden="true">⌄</span>
  </button>
  <div id={`${uid}-list`} class="picker-menu" role="listbox" tabindex="-1" aria-label="Play instances" hidden={!open} onkeydown={key}>
    {#each instances as instance, index (instance.id)}
      <button id={`${uid}-${index}`} type="button" role="option" aria-selected={instance.id === selectedId}
        tabindex={open && current === index ? 0 : -1} class="picker-option" disabled={busy}
        onfocus={() => current = index} onclick={() => void choose(instance.id)}>
        <span class="option-identity"><strong>{instance.displayName}</strong>
          <small>Minecraft {instance.minecraftVersion} · {instance.platform.kind === "vanilla" ? "Vanilla" : "Fabric"}{instance.aurora ? " · Aurora configured" : ""}</small>
        </span><span class="selected-marker">{instance.id === selectedId ? "✓ Selected" : ""}</span>
      </button>
    {/each}
  </div>
</div>
<style>
  .instance-switcher { position: relative; min-width: 0; }
  .picker-trigger { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); width: 100%; min-width: 0; border: 1px solid transparent; border-radius: var(--radius-sm); padding: var(--space-2); margin-left: calc(-1 * var(--space-2)); background: transparent; color: var(--color-text); font: inherit; font-size: 1.5rem; font-weight: 600; text-align: left; cursor: pointer; }
  .picker-trigger > span:first-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .picker-trigger:hover, .picker-trigger[aria-expanded="true"] { background: var(--color-surface-raised); border-color: var(--color-border); }
  .chevron { color: var(--color-text-muted); font-size: 1rem; }
  .picker-menu { position: absolute; z-index: 20; top: calc(100% + var(--space-2)); left: 0; width: 100%; max-height: 360px; overflow-y: auto; padding: var(--space-2); border: 1px solid var(--color-border-strong); border-radius: var(--radius-md); background: var(--color-surface-raised); box-shadow: var(--shadow-group); }
  .picker-menu[hidden] { display: none; }
  .picker-option { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); width: 100%; padding: var(--space-3); border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--color-text); font: inherit; text-align: left; cursor: pointer; }
  .picker-option:hover, .picker-option[aria-selected="true"] { background: var(--color-surface); }
  .option-identity { display: grid; gap: var(--space-1); min-width: 0; }
  .option-identity strong { font-size: var(--text-body); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .option-identity small { color: var(--color-text-secondary); font-size: var(--text-metadata); }
  .selected-marker { color: var(--color-text-secondary); font-size: var(--text-metadata); white-space: nowrap; }
  .picker-option:focus-visible, .picker-trigger:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
</style>
