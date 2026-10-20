<script lang="ts">
  import { launcher } from './store.svelte';
  import { navigation } from './navigation.svelte';
  import SettingsRow from './SettingsRow.svelte';
  const instance = $derived(launcher.selectedInstance);
</script>
<div class="section-intro"><h3>Minecraft &amp; Java</h3><p>Game preferences belong to each instance. Open its workspace to edit and save them.</p></div>
<section class="group" aria-label="Instance configuration access">
  {#if instance}
    <div class="group-heading"><div><h4 class="group-title">{instance.displayName}</h4><p class="group-subtitle">Selected Play instance · saved configuration</p></div></div>
    <SettingsRow id="instance-memory" title="Memory & game window" description={`Saved memory: ${instance.configuration.memoryMib} MiB. Memory, JVM arguments and window dimensions apply on the next game launch.`}>
      <button class="btn btn-primary" type="button" onclick={() => navigation.openInstance(instance.id, 'settings')}>Open instance Settings</button>
    </SettingsRow>
    <SettingsRow id="instance-java" title="Managed Java" description="Aurora selects the required Java version from the installed game plan and validates it before launch. Check or install that runtime from the instance's Settings.">
      <button class="btn" type="button" onclick={() => navigation.openInstance(instance.id, 'settings')}>Check Java in Settings</button>
    </SettingsRow>
    <p class="group-footer">Minecraft or loader changes require Save changes, then Install new configuration. They never change another instance.</p>
  {:else}
    <SettingsRow id="instance-library" title="Choose an instance" description="Open an instance from Library to configure Minecraft, memory, JVM arguments and game window size, or to manage Java.">
      <button class="btn btn-primary" type="button" onclick={() => navigation.goTo('instances')}>Open Library</button>
    </SettingsRow>
  {/if}
</section>
