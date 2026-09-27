<script lang="ts">
  import { onMount } from "svelte";
  import { homeWidgets } from "./homeLayout.svelte";
  import { widgetCatalog } from "./widgets";
  onMount(() => { void homeWidgets.load(); });
</script>
<section class="group" aria-labelledby="home-widgets-title">
  <div class="group-heading"><div><h3 id="home-widgets-title" class="group-title">Home Widgets</h3><p class="group-subtitle">Choose what appears beneath your Play area.</p></div></div>
  {#each widgetCatalog as widget (widget.id)}
    <label class="group-row"><div class="group-row-main"><span class="group-row-title">{widget.title}</span><span class="group-row-detail">{widget.description}</span></div>
      <input type="checkbox" aria-label={`Show ${widget.title}`} checked={homeWidgets.layout?.widgets.find(item => item.id === widget.id)?.enabled ?? false} disabled={homeWidgets.busy || !homeWidgets.layout} onchange={event => homeWidgets.enable(widget.id, event.currentTarget.checked)} />
    </label>
  {/each}
  <div class="group-row"><span class="group-row-detail">Use Customize Home to change order and size.</span><button class="btn btn-quiet" type="button" disabled={homeWidgets.busy || !homeWidgets.layout} onclick={() => homeWidgets.reset()}>Reset layout to defaults</button></div>
  {#if homeWidgets.error}<p class="group-row inline-message inline-message-error" role="alert">{homeWidgets.error}</p>{/if}
</section>
