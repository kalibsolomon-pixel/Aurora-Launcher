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
  /* Tile: a quiet square ground tinted by the accent, the instance's identity mark. */
  .tile { width: calc(var(--icon-size) * 1.9); height: calc(var(--icon-size) * 1.9); border-radius: calc(var(--icon-size) * .5); background: linear-gradient(160deg, color-mix(in srgb, var(--color-accent) 22%, transparent), color-mix(in srgb, var(--color-accent) 8%, transparent)); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-accent) 24%, transparent); color: color-mix(in srgb, var(--color-accent) 30%, var(--color-text)); }
  .tile.fallback { background: rgb(255 255 255 / 5%); box-shadow: inset 0 0 0 1px var(--f-edge); color: var(--color-text-secondary); }
</style>
