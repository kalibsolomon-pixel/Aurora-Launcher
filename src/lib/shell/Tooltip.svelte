<script module lang="ts">
  let dismissActive: (() => void) | undefined;
</script>
<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let { label, children, icon = 'about', tone = 'neutral' }: {
    label: string; children: Snippet; icon?: string; tone?: 'neutral' | 'warning';
  } = $props();
  const id = $props.id();
  let trigger: HTMLButtonElement;
  let popup: HTMLDivElement;
  let open = $state(false);
  let pinned = false;
  let closeTimer: ReturnType<typeof setTimeout> | undefined;

  function position() {
    if (!open) return;
    const anchor = trigger.getBoundingClientRect();
    const box = popup.getBoundingClientRect();
    const gap = 8, edge = 12;
    popup.style.left = `${Math.max(edge, Math.min(anchor.left, innerWidth - box.width - edge))}px`;
    const below = anchor.bottom + gap;
    const above = anchor.top - box.height - gap;
    popup.style.top = `${Math.max(edge, Math.min(below + box.height <= innerHeight - edge ? below : above, innerHeight - box.height - edge))}px`;
  }

  async function show() {
    clearTimeout(closeTimer);
    if (!open) { dismissActive?.(); dismissActive = hide; popup.showPopover(); open = true; }
    await tick();
    position();
  }
  function hide() { clearTimeout(closeTimer); pinned = false; popup.hidePopover(); open = false; if (dismissActive === hide) dismissActive = undefined; }
  function outside(event: PointerEvent) {
    if (event.target instanceof Node && !trigger.contains(event.target) && !popup.contains(event.target)) hide();
  }
  function leave() {
    clearTimeout(closeTimer);
    closeTimer = setTimeout(() => {
      if (!pinned && !trigger.matches(':focus-visible') && !popup.matches(':hover')) hide();
    }, 120);
  }
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && open) { event.preventDefault(); event.stopPropagation(); hide(); }
  }
  $effect(() => {
    if (!open) return;
    // Geometry is read only while a tooltip is visible, never for closed rows.
    window.addEventListener('resize', position);
    document.addEventListener('scroll', position, { capture: true, passive: true });
    document.addEventListener('keydown', keydown, true);
    document.addEventListener('pointerdown', outside, true);
    return () => {
      window.removeEventListener('resize', position);
      document.removeEventListener('scroll', position, true);
      document.removeEventListener('keydown', keydown, true);
      document.removeEventListener('pointerdown', outside, true);
    };
  });
  $effect(() => () => { clearTimeout(closeTimer); if (dismissActive === hide) dismissActive = undefined; });
</script>

<button bind:this={trigger} type="button" class="tooltip-trigger" class:warning={tone === 'warning'}
  aria-label={label} aria-describedby={id} aria-expanded={open}
  onpointerenter={show} onpointerleave={leave} onfocus={show} onblur={() => { if (!pinned) hide(); }}
  onclick={() => { if (pinned) hide(); else { pinned = true; void show(); } }}>
  <Icon name={icon} size={16} />
</button>
<!-- The shared Aurora tooltip material, in the native top layer so rows cannot
     clip it. One shared active tooltip and explicit dismissal keep hover,
     keyboard and touch behavior consistent. -->
<div bind:this={popup} id={id} class="f-tooltip content-tooltip" role="tooltip" popover="manual"
  ontoggle={() => { open = popup.matches(':popover-open'); if (!open) pinned = false; }}
  onpointerenter={() => clearTimeout(closeTimer)} onpointerleave={leave}>
  {@render children()}
</div>

<style>
  .tooltip-trigger { display: inline-grid; place-items: center; vertical-align: middle; flex: none; width: 24px; height: 24px; padding: 0; border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--color-text-secondary); cursor: help; }
  .tooltip-trigger.warning { color: var(--color-warning); }
  .tooltip-trigger:hover { background: var(--color-surface-raised); }
  .content-tooltip { position: fixed; inset: auto; margin: 0; transform: none; width: max-content; max-width: min(360px, calc(100vw - 24px)); max-height: calc(100dvh - 24px); overflow: auto; white-space: normal; overflow-wrap: anywhere; opacity: 1; pointer-events: auto; line-height: 1.5; text-align: left; }
</style>
