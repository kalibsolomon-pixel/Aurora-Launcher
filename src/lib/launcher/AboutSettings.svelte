<script lang="ts">
  import { launcher } from './store.svelte';
  import SettingsRow from './SettingsRow.svelte';
</script>
<div class="section-intro"><h3>About Aurora</h3></div>
<section class="group" aria-label="Running application">
  <SettingsRow id="about-version" title="Launcher version" description="Reported by the running native application.">
    <span class="setting-state">{launcher.status?.launcherVersion ?? 'Version unavailable'}</span>
  </SettingsRow>
  <SettingsRow id="about-platform" title="Platform" description="The operating system and architecture reported by this application.">
    <span class="setting-state">{launcher.status?.platform ? `${launcher.status.platform.os} · ${launcher.status.platform.architecture}` : 'Platform unavailable'}</span>
  </SettingsRow>
  <SettingsRow id="about-license" title="License" description="Aurora Launcher is MIT licensed. Minecraft requires your own entitled account." />

</section>
