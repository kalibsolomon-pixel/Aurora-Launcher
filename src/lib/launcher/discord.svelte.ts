import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { connectDiscord, getDiscordState, saveDiscordPreferences, type DiscordPreferences, type DiscordState } from "$lib/backend";
export const connectionLabels: Record<DiscordState["connection"], string> = {
  configurationMissing: "Application setup required", ready: "Connecting automatically", connected: "Connected",
  notDetected: "Discord not detected", closed: "Discord closed", failed: "Connection failed",
};
class DiscordStore {
  state = $state<DiscordState | null>(null);
  busy = $state(false);
  error = $state("");
  async refresh(): Promise<void> {
    try { this.state = await getDiscordState(); this.error = ""; }
    catch (reason) { this.error = reason instanceof Error ? reason.message : "Discord status unavailable."; }
  }
  async subscribe(): Promise<UnlistenFn> {
    const stop = await listen<DiscordState>("discord-state", event => { this.state = event.payload; });
    await this.refresh(); return stop;
  }
  async connect(): Promise<void> {
    if (this.busy || !this.state?.configured) return;
    this.busy = true;
    try { this.state = await connectDiscord(); this.error = ""; }
    catch (reason) { this.error = reason instanceof Error ? reason.message : "Discord connection failed."; }
    finally { this.busy = false; }
  }
  async change(field: keyof DiscordPreferences, value: boolean): Promise<void> {
    if (this.busy || !this.state) return;
    this.busy = true;
    try { this.state = await saveDiscordPreferences({ ...this.state.preferences, [field]: value }); this.error = ""; }
    catch (reason) { this.error = reason instanceof Error ? reason.message : "Discord preferences could not be saved."; }
    finally { this.busy = false; }
  }
}
export const discord = new DiscordStore();
