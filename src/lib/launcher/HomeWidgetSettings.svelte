<script lang="ts">
  import { onMount } from "svelte";
  import { homeWidgets } from "./homeLayout.svelte";
  import { widgetCatalog } from "./widgets";
  import { navigation } from './navigation.svelte';
  import SettingsRow from './SettingsRow.svelte';
  import SettingsToggle from './SettingsToggle.svelte';
  import SettingsFeedback from './SettingsFeedback.svelte';
  let saved = $state(false);
  let resetDialog: HTMLDialogElement;
  let resetTrigger: HTMLButtonElement;
  let resetCancel: HTMLButtonElement;
  let retryOperation = $state<(() => Promise<void>) | null>(null);
  async function save(operation: () => Promise<void>) {
    if (homeWidgets.busy) return;
    retryOperation = operation; saved = false;
    await operation(); saved = !homeWidgets.error;
  }
  onMount(() => { void homeWidgets.load(); });
</script>
<section class="group" aria-labelledby="home-widgets-title">
  <div class="group-heading"><div><h3 id="home-widgets-title" class="group-title">Widget visibility</h3></div></div>
  {#if !homeWidgets.layout && !homeWidgets.error}<p class="group-footer" role="status">Loading widgets…</p>{/if}
  {#each widgetCatalog as widget (widget.id)}
    <SettingsToggle id={`show-${widget.id}`} title={widget.title} description={widget.description}
      label={`Show ${widget.title}`} checked={homeWidgets.layout?.widgets.find(item => item.id === widget.id)?.enabled ?? false}
      disabled={homeWidgets.busy || !homeWidgets.layout} change={value => save(() => homeWidgets.enable(widget.id, value))} />
  {/each}
</section>
<section class="group" aria-label="Widget layout">
  <SettingsRow id="edit-home" title="Arrange on Home" description="Reorder and resize widgets in the existing Home editor. Your layout stays saved between sessions.">
    <button class="btn btn-primary" type="button" onclick={() => { homeWidgets.editing = true; navigation.goTo('home'); }}>Edit Home</button>
  </SettingsRow>
  <SettingsRow id="reset-home" title="Reset widget layout" description="Restore default visibility, order and sizes. Your worlds, play history and appearance library stay as they are.">
    <button bind:this={resetTrigger} class="btn btn-quiet" type="button" disabled={homeWidgets.busy || !homeWidgets.layout} onclick={() => { resetDialog.showModal(); resetCancel.focus(); }}>Reset layout…</button>
  </SettingsRow>
</section>
<SettingsFeedback busy={homeWidgets.busy && !!homeWidgets.layout} error={homeWidgets.error} {saved}
  retryLabel={retryOperation ? 'Retry save' : 'Retry loading'} retry={() => retryOperation ? void save(retryOperation) : void homeWidgets.load()} />
<dialog bind:this={resetDialog} class="settings-dialog aurora-dialog" aria-labelledby="reset-widgets-title" onclose={() => resetTrigger?.focus()}>
  <h3 id="reset-widgets-title">Reset Home layout?</h3>
  <p>This restores default widget visibility, positions and sizes. It does not erase your play history or appearance library.</p>
  <div class="dialog-actions"><button bind:this={resetCancel} class="btn btn-quiet" type="button" onclick={() => resetDialog.close()}>Cancel</button>
    <button class="btn btn-primary" type="button" onclick={() => { resetDialog.close(); void save(() => homeWidgets.reset()); }}>Reset layout</button></div>
</dialog>
