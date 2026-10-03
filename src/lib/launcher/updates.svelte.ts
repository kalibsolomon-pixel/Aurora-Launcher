import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  applyClientUpdate, checkForUpdates, getUpdateOverview, launcherUpdateDownload,
  launcherUpdateInstall, previewClientUpdate, startupUpdateCheck, type UpdateOverview,
} from "$lib/backend";
import { launcher } from "$lib/launcher/store.svelte";
import { UpdateController, type UpdateSnapshot } from "./updateController";

class UpdateStore {
  snapshot = $state<UpdateSnapshot>({ action: { kind: "idle" }, overview: null, context: "" });
  launcherDownloaded = $state<number | null>(null);
  launcherTotal = $state<number | null>(null);
  private started = false;
  private listeners: UnlistenFn[] = [];
  readonly controller = new UpdateController({
    check: checkForUpdates,
    startup: startupUpdateCheck,
    preview: previewClientUpdate,
    apply: applyClientUpdate,
    download: async (version) => {
      this.launcherDownloaded = null;
      this.launcherTotal = null;
      return launcherUpdateDownload(version);
    },
    install: launcherUpdateInstall,
    afterClient: async () => { await launcher.refreshState(); await this.refresh(); },
  }, (snapshot) => { this.snapshot = snapshot; });

  get overview(): UpdateOverview | null { return this.snapshot.overview; }
  async initialize(): Promise<void> {
    if (this.started) return;
    this.started = true;
    try {
      this.listeners.push(await listen<UpdateOverview>("update-status", (event) => {
        this.controller.receive(event.payload);
      }));
      this.listeners.push(await listen<{ downloadedBytes: number | null; totalBytes: number | null }>(
        "launcher-update-progress", (event) => {
          this.launcherDownloaded = event.payload.downloadedBytes;
          this.launcherTotal = event.payload.totalBytes;
        },
      ));
      await this.refresh();
      // Wait for normal launcher state before the single non-blocking check.
      await launcher.refreshState();
      void this.controller.startup();
    } catch { /* Update initialization is optional and nonfatal. */ }
  }
  dispose(): void {
    this.controller.closeView();
    for (const stop of this.listeners) stop();
    this.listeners = [];
  }
  async refresh(): Promise<void> {
    try { this.controller.receive(await getUpdateOverview()); }
    catch { /* Local state failure never degrades Play. */ }
  }
}
export const updates = new UpdateStore();
