<script lang="ts">
  import { onMount, tick } from "svelte";
  import { homeWidgets } from "./homeLayout.svelte";
  import { widgetCatalog, registeredWidgets } from "./widgets";
  import { launcher } from "./store.svelte";
  import InstanceDetailsWidget from "./widgets/InstanceDetailsWidget.svelte";
  import ContentSummaryWidget from "./widgets/ContentSummaryWidget.svelte";
  import SessionWidget from "./widgets/SessionWidget.svelte";
  import PlaytimeWidget from "./widgets/PlaytimeWidget.svelte";
  import RecentTargetsWidget from "./widgets/RecentTargetsWidget.svelte";
  import SkinManagerWidget from "./widgets/SkinManagerWidget.svelte";
  import CapeSelectorWidget from "./widgets/CapeSelectorWidget.svelte";
  import type { WidgetSize } from "$lib/backend";
  const components = { "instance-details": InstanceDetailsWidget, "content-summary": ContentSummaryWidget, session: SessionWidget };
  import Icon from '$lib/shell/Icon.svelte';
  const editing = $derived(homeWidgets.editing);
  let dragged = $state<string | null>(null);
  let announcement = $state('');
  let editorButton: HTMLButtonElement;
  async function hide(id: string) {
    await homeWidgets.enable(id, false);
    await tick(); editorButton?.focus();
    announcement = homeWidgets.error ? '' : 'Widget hidden. Add it again from the widget tray.';
  }
  async function drop(target: string) {
    if (!editing || !dragged || homeWidgets.busy) return;
    await homeWidgets.reorder(dragged, target); dragged = null;
    announcement = homeWidgets.error ? '' : 'Widget order saved.';
  }
  function finish() { homeWidgets.editing = false; dragged = null; }

  const widgets = $derived(homeWidgets.layout ? registeredWidgets(homeWidgets.layout) : []);
  onMount(() => { void homeWidgets.load(); });
</script>

{#if homeWidgets.error}<p class="inline-message inline-message-error" role="alert">{homeWidgets.error}</p>{/if}
  <section class="home-widgets" aria-label="Home widgets">
    <div class="widgets-heading" class:editing>
      <span class="edit-status" role="status">{editing ? 'Editing Home' : ''}</span>
      <button bind:this={editorButton} type="button" class={editing ? 'btn btn-primary' : 'f-icon-button'} aria-label={editing ? 'Finish editing Home' : 'Edit Home widgets'} title={editing ? 'Finish editing' : 'Edit Home widgets'} aria-pressed={editing} disabled={!homeWidgets.layout} onclick={() => editing ? finish() : homeWidgets.editing = true}>{#if editing}<Icon name="check" size={16} /> Done{:else}<Icon name="pencil" size={18} />{/if}</button>
    </div>
    {#if editing}
      <div class="widget-tray f-surface" aria-label="Available widgets">
        <div class="tray-intro"><strong>Add to your Home</strong><span>Drag a handle to reorder, or use each card’s arrow buttons.</span></div>
        <div class="tray-options">
          {#each widgetCatalog as definition (definition.id)}
            {@const shown = widgets.some(widget => widget.id === definition.id)}
            <button type="button" class="btn btn-quiet" disabled={homeWidgets.busy || shown} onclick={() => homeWidgets.enable(definition.id, true)}><Icon name={shown ? 'check' : 'plus'} size={14} />{definition.title}</button>
          {/each}
        </div>
      </div>
    {/if}
    <span class="sr-only" role="status">{announcement}</span>
    <div class="widget-grid">
      {#each widgets as widget, index (widget.id)}
        {@const definition = widgetCatalog.find(item => item.id === widget.id)!}
        <article class="group f-surface widget" class:widget-wide={widget.size !== "small"} class:widget-large={widget.size === "large"} aria-label={definition.title} data-home-widget={widget.id}
          class:widget-editing={editing} class:widget-dragged={dragged === widget.id}>
          <div class="widget-heading"><h4>{definition.title}</h4>{#if editing}<button type="button" class="f-icon-button drag-handle" disabled={homeWidgets.busy} aria-label={`Drag ${definition.title} to reorder; arrow buttons below also change order`} title="Drag to reorder" onpointerdown={event => { if (event.button !== 0) return; dragged = widget.id; event.currentTarget.setPointerCapture(event.pointerId); }} onpointerup={event => { const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>('[data-home-widget]')?.dataset.homeWidget; if (target) void drop(target); else dragged = null; }} onlostpointercapture={() => dragged = null}><Icon name="grip" size={16} /></button>{/if}</div>
          {#if widget.id === "playtime"}<PlaytimeWidget size={widget.size} />
          {:else if widget.id === "recent-worlds"}<RecentTargetsWidget mode="world" size={widget.size} />
          {:else if widget.id === "recent-servers"}<RecentTargetsWidget mode="server" size={widget.size} />
          {:else if widget.id === "skin-manager"}<SkinManagerWidget size={widget.size} />
          {:else if widget.id === "cape-selector"}<CapeSelectorWidget size={widget.size} />
          {:else}{@const Component = components[widget.id as keyof typeof components]}<Component instance={launcher.selectedInstance} />{/if}
          {#if editing}
            <div class="widget-editor" role="group" aria-label={`Customize ${definition.title}`}>
              <button class="btn btn-quiet" type="button" aria-label={`Move ${definition.title} earlier`} disabled={homeWidgets.busy || index === 0} onclick={() => homeWidgets.move(widget.id, -1)}>←</button>
              <button class="btn btn-quiet" type="button" aria-label={`Move ${definition.title} later`} disabled={homeWidgets.busy || index === widgets.length - 1} onclick={() => homeWidgets.move(widget.id, 1)}>→</button>
              <label>Size <select aria-label={`${definition.title} size`} value={widget.size} disabled={homeWidgets.busy} onchange={event => homeWidgets.resize(widget.id, event.currentTarget.value as WidgetSize)}>{#each definition.sizes as size}<option value={size}>{size[0].toUpperCase() + size.slice(1)}</option>{/each}</select></label>
              <button class="btn btn-quiet" type="button" disabled={homeWidgets.busy} onclick={() => hide(widget.id)}>Hide</button>
            </div>
          {/if}
        </article>
      {/each}
    </div>
  </section>

<style>
  .home-widgets { margin-top: var(--space-4); }
  .widgets-heading { display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--space-3); }
  .widgets-heading.editing { position: sticky; top: 0; z-index: 5; padding: 8px 12px; border: 1px solid var(--f-edge); border-radius: 14px; background: var(--color-surface); }
  .edit-status { margin: 0; font-size: var(--text-body); color: var(--color-text-secondary); font-weight: 500; }
  .widget-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-4); }
  .widget { min-width: 0; margin: 0; padding: var(--space-4); min-height: 156px; }
  .widget-wide { grid-column: span 2; }
  .widget-large { min-height: 240px; }
  .widget-heading { display: flex; align-items: start; justify-content: space-between; gap: 8px; }
  .widget-tray { padding: 20px; margin-bottom: 20px; }
  .tray-intro { display: flex; flex-wrap: wrap; gap: 8px 20px; align-items: baseline; margin-bottom: 14px; }
  .tray-intro strong { font-size: 14px; }
  .tray-intro span { font-size: 12px; color: var(--color-text-secondary); }
  .tray-options { display: flex; flex-wrap: wrap; gap: 6px; }
  .tray-options .btn { font-size: 12px; }
  .widget-editing { outline: 1px dashed var(--color-accent-outline); outline-offset: 3px; }
  .widget-dragged { opacity: .5; }
  .drag-handle { touch-action: none; cursor: grab; width: 28px; height: 28px; margin-top: -6px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  .widget-heading h4 { margin: 0 0 var(--space-3); font-size: var(--text-body); font-weight: 600; }
  .widget-editor { display: flex; flex-wrap: wrap; gap: var(--space-2); align-items: center; padding-top: var(--space-3); margin-top: var(--space-3); border-top: 1px solid var(--color-border); }
  .widget-editor label { display: flex; align-items: center; gap: var(--space-2); color: var(--color-text-secondary); font-size: var(--text-metadata); }
  select { font: inherit; color: var(--color-text); background: var(--color-surface-sunken); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); padding: var(--space-2); }
  @media (max-width: 900px) { .widget-grid { grid-template-columns: minmax(0, 1fr); } .widget-wide { grid-column: span 1; } .widget-large { min-height: 200px; } }
</style>
