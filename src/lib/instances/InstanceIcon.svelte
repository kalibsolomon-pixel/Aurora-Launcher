<script lang="ts">
  // Self-hosted Google Material Symbols (Outlined) from the pinned package; the
  // font loads from the app origin, so the CSP and offline use are unchanged.
  import "material-symbols/outlined.css";
  import { FALLBACK_INSTANCE_ICON, isValidIconName } from "./instanceIcons";

  let { icon = null, size = 24, tile = false }: { icon?: string | null; size?: number; tile?: boolean } = $props();
  const glyph = $derived(icon && isValidIconName(icon) ? icon : FALLBACK_INSTANCE_ICON);
</script>

<span class="instance-icon" class:tile class:fallback={glyph !== icon} style:--icon-size="{size}px" aria-hidden="true">
  <span class="material-symbols-outlined">{glyph}</span>
</span>

<style>
  .instance-icon { display: inline-grid; place-items: center; flex: none; color: var(--color-text); }
  .instance-icon .material-symbols-outlined { font-size: var(--icon-size); font-variation-settings: "FILL" 0, "wght" 300, "GRAD" 0, "opsz" 24; user-select: none; }
  /* The glyph identifies the instance; its ground stays neutral. */
  .tile { width: calc(var(--icon-size) * 1.6); height: calc(var(--icon-size) * 1.6); border-radius: var(--radius-md); background: var(--color-surface-raised); color: var(--color-text); }
  .tile.fallback { color: var(--color-text-secondary); }
</style>
