import {
  LauncherBackendError,
  createDesktopShortcut,
  createStartMenuShortcut,
  openLauncherFolder,
  getDesktopIntegration,
  removeDesktopShortcut,
  type DesktopIntegrationState,
} from "$lib/backend";

function backendError(cause: unknown, fallback: string): LauncherBackendError {
  return cause instanceof LauncherBackendError
    ? cause
    : new LauncherBackendError("unknown_error", fallback);
}

/**
 * The frontend owner of Windows desktop-integration state.
 *
 * Rust owns every decision — which slots exist, who owns them, whether this
 * build may manage them — and the state is always the live answer from the
 * operating system, queried on demand and never persisted here. A shortcut
 * deleted outside Aurora disappears from this state the next time it
 * refreshes.
 */
class DesktopIntegrationStore {
  state = $state<DesktopIntegrationState | null>(null);
  error = $state<LauncherBackendError | null>(null);
  busy = $state(false);
  notice = $state("");

  /** Queries the live shortcut state from the backend. */
  async refresh(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      this.state = await getDesktopIntegration();
      this.error = null;
    } catch (cause: unknown) {
      this.error = backendError(cause, "Windows shortcut status could not be read.");
    } finally {
      this.busy = false;
    }
  }

  /** Creates (or refreshes) the Aurora-owned desktop shortcut. */
  async create(): Promise<void> {
    await this.run(createDesktopShortcut, "The desktop shortcut could not be created.", "Desktop shortcut added.");
  }

  async addToStart(): Promise<void> {
    await this.run(createStartMenuShortcut, "The Start menu shortcut could not be created.", "Added to the Start menu app list.");
  }

  async openPath(): Promise<void> {
    await this.run(async () => { await openLauncherFolder(); return this.state; }, "The launcher data folder could not be opened.", "Launcher data folder opened.");
  }

  /** Removes the Aurora-owned desktop shortcut; conflicts are refused in Rust. */
  async remove(): Promise<void> {
    await this.run(removeDesktopShortcut, "The desktop shortcut could not be removed.");
  }

  private async run(
    action: () => Promise<DesktopIntegrationState | null>,
    fallback: string,
    success = "",
  ): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.notice = "";
    try {
      this.state = await action();
      this.error = null;
      this.notice = success;
    } catch (cause: unknown) {
      this.error = backendError(cause, fallback);
      // Re-query so the shown state stays honest after a refusal.
      try {
        this.state = await getDesktopIntegration();
      } catch {
        // The action's error is the one to surface.
      }
    } finally {
      this.busy = false;
    }
  }
}

export const desktopIntegration = new DesktopIntegrationStore();
