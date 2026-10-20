<script lang="ts">
  import { tick, untrack } from 'svelte';
  import SettingsRow from './SettingsRow.svelte';
  let { id, title, description, label, checked, disabled = false, change }: {
    id: string; title: string; description: string; label: string; checked: boolean;
    disabled?: boolean; change: (value: boolean) => Promise<void>;
  } = $props();
  let draft = $state(untrack(() => checked));
  $effect(() => { draft = checked; });
  async function update() {
    await change(draft);
    await tick();
    // Native success owns the value; a rejected write restores the saved state.
    draft = checked;
  }
</script>
<SettingsRow {id} {title} {description} controlId={id}>
  <span class="setting-state" aria-hidden="true">{checked ? 'On' : 'Off'}</span>
  <input type="checkbox" aria-label={label} {disabled} {id}
    bind:checked={draft} onchange={update} />
</SettingsRow>
