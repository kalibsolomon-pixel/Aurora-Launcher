import type { HomeLayout, WidgetPlacement, WidgetSize } from "$lib/backend";

export interface WidgetDefinition { id: string; title: string; description: string; sizes: readonly WidgetSize[] }
/** Registry entries are code-owned. Persistence contains IDs and layout only. */
export const widgetCatalog: readonly WidgetDefinition[] = [
  { id: "instance-details", title: "Instance Details", description: "Installed Minecraft, platform and current Aurora content state.", sizes: ["small", "wide", "large"] },
  { id: "content-summary", title: "Content Summary", description: "Local enabled/disabled mod and pack counts. No provider network requests.", sizes: ["small", "wide", "large"] },
  { id: "session", title: "Session", description: "The selected instance's latest process state in this launcher session. Not a saved play history.", sizes: ["small", "wide"] },
  { id: "playtime", title: "Playtime", description: "Local UTC playtime totals and daily history.", sizes: ["small", "wide", "large"] },
  { id: "recent-worlds", title: "Recent Worlds", description: "Recently played local worlds and validated Quick Launch.", sizes: ["small", "wide", "large"] },
  { id: "recent-servers", title: "Recent Servers", description: "Recently played servers and validated Quick Launch.", sizes: ["small", "wide", "large"] },
  { id: "skin-manager", title: "Skin Library", description: "Current account skin and your saved, reusable imported skins.", sizes: ["small", "wide", "large"] },
  { id: "cape-selector", title: "Cape Selector", description: "Select or disable capes owned by the current Minecraft account.", sizes: ["small", "wide", "large"] },
];
export function registeredWidgets(layout: HomeLayout): WidgetPlacement[] {
  return layout.widgets.filter(widget => widget.enabled && widgetCatalog.some(item => item.id === widget.id));
}
export function moveWidget(layout: HomeLayout, id: string, direction: -1 | 1): HomeLayout {
  const widgets = layout.widgets.map(widget => ({ ...widget }));
  const visible = registeredWidgets(layout);
  const index = visible.findIndex(widget => widget.id === id);
  const neighbor = visible[index + direction];
  if (!neighbor) return { widgets };
  const a = widgets.findIndex(widget => widget.id === id);
  const b = widgets.findIndex(widget => widget.id === neighbor.id);
  [widgets[a], widgets[b]] = [widgets[b], widgets[a]];
  return { widgets };
}
export function setWidget(layout: HomeLayout, id: string, changes: Partial<Pick<WidgetPlacement, "enabled" | "size">>): HomeLayout {
  const definition = widgetCatalog.find(widget => widget.id === id);
  if (!definition || (changes.size && !definition.sizes.includes(changes.size))) return layout;
  const widgets = layout.widgets.map(widget => widget.id === id ? { ...widget, ...changes } : { ...widget });
  if (!widgets.some(widget => widget.id === id)) widgets.push({ id, enabled: false, size: definition.sizes[0], ...changes });
  return { widgets };
}

/** Reorder visible slots only; hidden and unavailable registrations stay intact. */
export function reorderWidget(layout: HomeLayout, id: string, target: string): HomeLayout {
  const visible = registeredWidgets(layout);
  const from = visible.findIndex(widget => widget.id === id);
  const to = visible.findIndex(widget => widget.id === target);
  if (from < 0 || to < 0 || from === to) return layout;
  const reordered = visible.map(widget => ({ ...widget }));
  reordered.splice(to, 0, reordered.splice(from, 1)[0]);
  const ids = new Set(visible.map(widget => widget.id));
  let index = 0;
  return { widgets: layout.widgets.map(widget => ids.has(widget.id) ? reordered[index++] : { ...widget }) };
}
