<script lang="ts">
  import { onMount } from "svelte";
  import { homeWidgets } from "./homeLayout.svelte";
  import { widgetCatalog, registeredWidgets } from "./widgets";
  import { launcher } from "./store.svelte";
  import InstanceDetailsWidget from "./widgets/InstanceDetailsWidget.svelte";
  import ContentSummaryWidget from "./widgets/ContentSummaryWidget.svelte";
  import SessionWidget from "./widgets/SessionWidget.svelte";
  import PlaytimeWidget from "./widgets/PlaytimeWidget.svelte";
  import RecentTargetsWidget from "./widgets/RecentTargetsWidget.svelte";
  import type { WidgetSize } from "$lib/backend";
  const components = { "instance-details": InstanceDetailsWidget, "content-summary": ContentSummaryWidget, session: SessionWidget };
  let editing = $state(false);
  const widgets = $derived(homeWidgets.layout ? registeredWidgets(homeWidgets.layout) : []);
  onMount(() => { void homeWidgets.load(); });
</script>

{#if homeWidgets.error}<p class="inline-message inline-message-error" role="alert">{homeWidgets.error}</p>{/if}
{#if widgets.length}
  <section class="home-widgets" aria-label="Home widgets">
    <div class="widgets-heading"><h3>Your Home</h3><button type="button" class="btn btn-quiet" aria-pressed={editing} onclick={() => editing = !editing}>{editing ? "Done" : "Customize Home"}</button></div>
    <div class="widget-grid">
      {#each widgets as widget, index (widget.id)}
        {@const definition = widgetCatalog.find(item => item.id === widget.id)!}
        <article class="group widget" class:widget-wide={widget.size !== "small"} class:widget-large={widget.size === "large"} aria-label={definition.title}>
          <div class="widget-heading"><h4>{definition.title}</h4></div>
          {#if widget.id === "playtime"}<PlaytimeWidget size={widget.size} />
          {:else if widget.id === "recent-worlds"}<RecentTargetsWidget mode="world" size={widget.size} />
          {:else if widget.id === "recent-servers"}<RecentTargetsWidget mode="server" size={widget.size} />
          {:else}{@const Component = components[widget.id as keyof typeof components]}<Component instance={launcher.selectedInstance} />{/if}
          {#if editing}
            <div class="widget-editor" role="group" aria-label={`Customize ${definition.title}`}>
              <button class="btn btn-quiet" type="button" aria-label={`Move ${definition.title} earlier`} disabled={homeWidgets.busy || index === 0} onclick={() => homeWidgets.move(widget.id, -1)}>←</button>
              <button class="btn btn-quiet" type="button" aria-label={`Move ${definition.title} later`} disabled={homeWidgets.busy || index === widgets.length - 1} onclick={() => homeWidgets.move(widget.id, 1)}>→</button>
              <label>Size <select aria-label={`${definition.title} size`} value={widget.size} disabled={homeWidgets.busy} onchange={event => homeWidgets.resize(widget.id, event.currentTarget.value as WidgetSize)}>{#each definition.sizes as size}<option value={size}>{size[0].toUpperCase() + size.slice(1)}</option>{/each}</select></label>
              <button class="btn btn-quiet" type="button" disabled={homeWidgets.busy} onclick={() => homeWidgets.enable(widget.id, false)}>Hide</button>
            </div>
          {/if}
        </article>
      {/each}
    </div>
  </section>
{/if}

<style>
  .home-widgets { margin-top: var(--space-4); }
  .widgets-heading { display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--space-3); }
  .widgets-heading h3 { margin: 0; font-size: var(--text-body); color: var(--color-text-secondary); font-weight: 500; }
  .widget-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-4); }
  .widget { min-width: 0; margin: 0; padding: var(--space-4); min-height: 156px; }
  .widget-wide { grid-column: span 2; }
  .widget-large { min-height: 240px; }
  .widget-heading h4 { margin: 0 0 var(--space-3); font-size: var(--text-body); font-weight: 600; }
  .widget-editor { display: flex; flex-wrap: wrap; gap: var(--space-2); align-items: center; padding-top: var(--space-3); margin-top: var(--space-3); border-top: 1px solid var(--color-border); }
  .widget-editor label { display: flex; align-items: center; gap: var(--space-2); color: var(--color-text-secondary); font-size: var(--text-metadata); }
  select { font: inherit; color: var(--color-text); background: var(--color-surface-sunken); border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); padding: var(--space-2); }
  @media (max-width: 900px) { .widget-grid { grid-template-columns: minmax(0, 1fr); } .widget-wide { grid-column: span 1; } .widget-large { min-height: 200px; } }
</style>
