import { listen } from "@tauri-apps/api/event";

/**
 * Frontend mirror of the launcher window's native minimize state.
 *
 * WebView2 keeps a minimized host window's page "visible" (`document.hidden`
 * never flips) and can drop the page blur when focus transitions race the
 * minimize, so the native `window-minimized` event is the authoritative
 * signal for pausing decorative playback. This store is initialized once at
 * boot (see `+layout.ts`) and pushes state changes to subscribers.
 */
export type MinimizedEvents = (
  event: string,
  handler: (event: { payload: boolean }) => void,
) => Promise<() => void>;

export class WindowActivityStore {
  private minimized = false;
  private listeners = new Set<(minimized: boolean) => void>();
  private started = false;
  private readonly connect: MinimizedEvents;

  constructor(connect: MinimizedEvents = (event, handler) => listen<boolean>(event, handler)) {
    this.connect = connect;
  }

  /** Subscribes to minimize transitions; immediately seeds the current state. */
  subscribe(listener: (minimized: boolean) => void): () => void {
    this.listeners.add(listener);
    listener(this.minimized);
    return () => { this.listeners.delete(listener); };
  }

  /** Connects to the native event; safe to call once at boot, never throws. */
  async initialize(): Promise<void> {
    if (this.started) return;
    this.started = true;
    await this.connect("window-minimized", (event) => {
      if (event.payload === this.minimized) return;
      this.minimized = event.payload;
      for (const listener of [...this.listeners]) listener(event.payload);
    });
  }
}

export const windowActivity = new WindowActivityStore();
