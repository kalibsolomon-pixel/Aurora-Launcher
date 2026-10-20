import { getInstanceIcons, setInstanceIcon, LauncherBackendError } from "$lib/backend";
import { isValidIconName } from "./instanceIcons";

/**
 * Instance icons, loaded lazily on first use and kept separate from launcher
 * state so startup and readiness are unaffected. A failed read leaves every
 * instance on its fallback glyph; nothing here can block launch.
 */
export const instanceIcons = new class {
  icons = $state<Record<string, string>>({});
  error = $state<string | null>(null);
  busy = $state<string | null>(null);
  /** Icon chosen in the creation form, applied once the instance exists. */
  pendingCreateIcon = $state<string | null>(null);
  private loading: Promise<void> | null = null;

  ensureLoaded(): Promise<void> {
    if (this.loading) return this.loading;
    this.loading = getInstanceIcons()
      .then((icons) => { this.icons = icons; this.error = null; })
      .catch((cause: unknown) => {
        this.error = cause instanceof LauncherBackendError ? cause.message : "Instance icons are unavailable.";
        this.loading = null;
      });
    return this.loading;
  }

  iconFor(instanceId: string | null | undefined): string | null {
    return instanceId ? this.icons[instanceId] ?? null : null;
  }

  async set(instanceId: string, icon: string | null): Promise<boolean> {
    if (icon !== null && !isValidIconName(icon)) return false;
    this.busy = instanceId;
    try {
      this.icons = await setInstanceIcon(instanceId, icon);
      this.error = null;
      return true;
    } catch (cause: unknown) {
      this.error = cause instanceof LauncherBackendError ? cause.message : "The icon could not be saved.";
      return false;
    } finally {
      this.busy = null;
    }
  }

  /** Apply the creation form's choice. An icon failure never fails creation. */
  async applyPending(instanceId: string): Promise<void> {
    const icon = this.pendingCreateIcon;
    this.pendingCreateIcon = null;
    if (icon) await this.set(instanceId, icon);
  }
}();
