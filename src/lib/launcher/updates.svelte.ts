/**
 * The frontend update state hub: presentation state over Rust-owned update
 * truth. All authority — availability, channels, signatures, transactions —
 * lives in the native backend; this store only mirrors the overview the
 * backend computes and drives explicit user actions.
 *
 * One bounded startup check runs per process; manual checks are explicit.
 * There is no polling anywhere: update services being unreachable never
 * degrades the launcher.
 */

import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  LauncherBackendError,
  checkForUpdates,
  dismissUpdateNotice,
  getUpdateOverview,
  launcherUpdateDownload,
  launcherUpdateInstall,
  setUpdateChannel,
  startupUpdateCheck,
  type ProductUpdateAvailability,
  type UpdateOverview,
  type UpdateReleaseChannel,
} from "$lib/backend";

interface LauncherUpdateProgressEvent {
  phase: string | null;
  downloadedBytes: number | null;
  totalBytes: number | null;
}

interface ClientUpdateProgressEvent {
  phase: string;
}

class UpdateStore {
  overview = $state<UpdateOverview | null>(null);
  checking = $state(false);
  busy = $state(false);
  error = $state<LauncherBackendError | null>(null);
  launcherPhase = $state<string | null>(null);
  launcherDownloaded = $state<number | null>(null);
  launcherTotal = $state<number | null>(null);
  clientPhase = $state<string | null>(null);
  private started = false;
  private listeners: UnlistenFn[] = [];

  /** Wires event listeners, loads the cached overview, and performs the one
   * bounded startup check. Never blocks; failures stay silent here and are
   * visible through the overview's own availability states. */
  async initialize(): Promise<void> {
    if (this.started) return;
    this.started = true;
    this.listeners.push(
      await listen<UpdateOverview>("update-status", (event) => {
        this.overview = event.payload;
      }),
    );
    this.listeners.push(
      await listen<LauncherUpdateProgressEvent>("launcher-update-progress", (event) => {
        this.launcherPhase = event.payload.phase;
        this.launcherDownloaded = event.payload.downloadedBytes;
        this.launcherTotal = event.payload.totalBytes;
      }),
    );
    this.listeners.push(
      await listen<ClientUpdateProgressEvent>("client-update-progress", (event) => {
        this.clientPhase = event.payload.phase;
      }),
    );
    try {
      this.overview = await getUpdateOverview();
    } catch {
      // Non-fatal: the launcher stays usable without update services.
    }
    void this.runStartupCheck();
  }

  dispose(): void {
    for (const stop of this.listeners) stop();
    this.listeners = [];
    this.started = false;
  }

  private async runStartupCheck(): Promise<void> {
    try {
      this.overview = await startupUpdateCheck();
    } catch {
      // Non-fatal by design.
    }
  }

  async refresh(): Promise<void> {
    try {
      this.overview = await getUpdateOverview();
    } catch {
      // Non-fatal by design.
    }
  }

  /** The explicit manual check across both update domains. */
  async check(): Promise<void> {
    if (this.checking) return;
    this.checking = true;
    this.error = null;
    try {
      this.overview = await checkForUpdates();
    } catch (cause) {
      this.error =
        cause instanceof LauncherBackendError
          ? cause
          : new LauncherBackendError("update_check_failed", "Could not check for updates.");
    } finally {
      this.checking = false;
    }
  }

  /** Changes the release channel; redefines future eligibility only. */
  async setChannel(channel: UpdateReleaseChannel): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.error = null;
    try {
      this.overview = await setUpdateChannel(channel);
    } catch (cause) {
      this.error =
        cause instanceof LauncherBackendError
          ? cause
          : new LauncherBackendError("update_channel_failed", "The channel could not be changed.");
    } finally {
      this.busy = false;
    }
  }

  /** Session-scoped dismissal of one update notice. */
  async dismiss(domain: "launcher" | "client"): Promise<void> {
    try {
      this.overview = await dismissUpdateNotice(domain);
    } catch {
      // Presentation-only; ignore.
    }
  }

  /** Downloads the offered launcher update; real progress only. */
  async downloadLauncher(version: string): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.error = null;
    this.launcherPhase = "downloading";
    try {
      await launcherUpdateDownload(version);
    } catch (cause) {
      this.error =
        cause instanceof LauncherBackendError
          ? cause
          : new LauncherBackendError("update_download_failed", "The update download failed.");
      this.launcherPhase = null;
    } finally {
      this.busy = false;
      void this.refresh();
    }
  }

  /**
   * Installs the downloaded launcher update. On Windows the verified
   * installer runs and the launcher exits — the UI states this before the
   * call; reaching the end means the platform restarted us.
   */
  async installLauncher(version: string): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.error = null;
    this.launcherPhase = "installing";
    try {
      await launcherUpdateInstall(version);
    } catch (cause) {
      this.error =
        cause instanceof LauncherBackendError
          ? cause
          : new LauncherBackendError("update_install_failed", "The update could not be installed.");
      this.launcherPhase = null;
    } finally {
      this.busy = false;
      void this.refresh();
    }
  }

  /** The launcher availability, for the notification surfaces. */
  launcherAvailability(): ProductUpdateAvailability {
    return this.overview?.launcher.availability ?? { kind: "notChecked" };
  }

  /** The Aurora Client availability for the selected instance. */
  clientAvailability(): ProductUpdateAvailability {
    return this.overview?.client ?? { kind: "notChecked" };
  }
}

export const updates = new UpdateStore();
