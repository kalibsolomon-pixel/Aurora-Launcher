import { getHomeWidgets, saveHomeWidgets, resetHomeWidgets, type HomeLayout } from "$lib/backend";
import { moveWidget, reorderWidget, setWidget } from "./widgets";
import type { WidgetSize } from "$lib/backend";

class HomeWidgetsStore {
  editing = $state(false);
  layout = $state<HomeLayout | null>(null);
  busy = $state(false);
  error = $state("");
  async load(): Promise<void> {
    if (this.layout || this.busy) return;
    this.busy = true;
    try { this.layout = await getHomeWidgets(); this.error = ""; }
    catch (reason) { this.error = reason instanceof Error ? reason.message : "Home layout could not be loaded."; }
    finally { this.busy = false; }
  }
  async save(layout: HomeLayout): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try { this.layout = await saveHomeWidgets(layout); this.error = ""; }
    catch (reason) { this.error = reason instanceof Error ? reason.message : "Home layout could not be saved."; }
    finally { this.busy = false; }
  }
  async reset(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try { this.layout = await resetHomeWidgets(); this.error = ""; }
    catch (reason) { this.error = reason instanceof Error ? reason.message : "Home layout could not be reset."; }
    finally { this.busy = false; }
  }
  enable(id: string, enabled: boolean): Promise<void> { return this.layout ? this.save(setWidget(this.layout, id, { enabled })) : Promise.resolve(); }
  resize(id: string, size: WidgetSize): Promise<void> { return this.layout ? this.save(setWidget(this.layout, id, { size })) : Promise.resolve(); }
  reorder(id: string, target: string): Promise<void> { return this.layout ? this.save(reorderWidget(this.layout, id, target)) : Promise.resolve(); }
  move(id: string, direction: -1 | 1): Promise<void> { return this.layout ? this.save(moveWidget(this.layout, id, direction)) : Promise.resolve(); }
}
export const homeWidgets = new HomeWidgetsStore();
