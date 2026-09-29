import {
  getAppearance,
  setAppearance,
  LauncherBackendError,
  type AccentSelection,
  type BackgroundId,
  type AppearanceState,
} from "$lib/backend";

function backendError(cause: unknown, fallback: string): LauncherBackendError {
  return cause instanceof LauncherBackendError
    ? cause
    : new LauncherBackendError("unknown_error", fallback);
}

/** The accent-family CSS custom properties owned by the appearance system. */
const ACCENT_PROPERTIES = [
  "--color-accent",
  "--color-accent-strong",
  "--color-accent-hover",
  "--color-accent-pressed",
  "--color-accent-contrast",
  "--color-accent-soft",
  "--color-accent-outline",
] as const;

/**
 * The frontend owner of launcher-wide appearance. Rust validates and derives
 * every value; this store only applies the returned theme attribute and
 * accent tokens to the document root and persists user choices.
 *
 * Changes apply live (no restart), and the initial load runs before the app
 * renders (see +layout.ts) so the persisted look is in place for the first
 * paint of the shell.
 */
class AppearanceStore {
  state = $state<AppearanceState | null>(null);
  error = $state<LauncherBackendError | null>(null);
  busy = $state(false);

  private initialized = false;

  get theme(): string {
    return this.state?.theme ?? "aurora-dark";
  }

  get background(): BackgroundId {
    return this.state?.background ?? "simple";
  }

  get auroraMotionSpeed(): number { return this.state?.auroraMotionSpeed ?? 50; }

  get accent(): AccentSelection {
    return this.state?.accent ?? { type: "preset", id: "violet" };
  }

  /** Loads and applies the persisted appearance; safe to call once at boot. */
  async initialize(): Promise<void> {
    if (this.initialized) return;
    this.initialized = true;

    try {
      this.apply(await getAppearance());
    } catch (cause: unknown) {
      // Appearance is cosmetic: a failure never blocks the launcher. The
      // default look stays applied and the Settings page reports the error.
      this.error = backendError(cause, "The launcher appearance could not be loaded.");
    }
  }

  /** Puts a Rust-derived appearance state onto the document root. */
  apply(state: AppearanceState): void {
    this.state = state;
    this.error = null;

    const root = document.documentElement;
    root.dataset.theme = state.theme;
    root.dataset.background = state.background;

    if (state.accent.type === "preset") {
      delete root.dataset.accentCustom;
      root.dataset.accent = state.accent.id;
    } else {
      delete root.dataset.accent;
      root.dataset.accentCustom = "true";
    }
    // Both preset and custom selections use the native-derived palette. CSS
    // presets are only the pre-initialization fallback, never a separate path.
    const palette = state.palette;
    const values = [palette.accent, palette.accentStrong, palette.accentHover,
      palette.accentPressed, palette.accentContrast, palette.accentSoft, palette.accentOutline];
    ACCENT_PROPERTIES.forEach((property, index) => root.style.setProperty(property, values[index]));
  }

  /** Switches the built-in theme: applied live, persisted by Rust. */
  async setTheme(theme: string): Promise<void> {
    if (this.busy || theme === this.theme) return;
    this.busy = true;
    try {
      this.apply(await setAppearance(theme, this.accent, this.background, this.auroraMotionSpeed));
    } catch (cause: unknown) {
      this.error = backendError(cause, "The theme could not be saved.");
    } finally {
      this.busy = false;
    }
  }

  async setBackground(background: BackgroundId): Promise<void> {
    if (this.busy || background === this.background) return;
    this.busy = true;
    try { this.apply(await setAppearance(this.theme, this.accent, background, this.auroraMotionSpeed)); }
    catch (cause: unknown) { this.error = backendError(cause, "The background could not be saved."); }
    finally { this.busy = false; }
  }

  async setAuroraMotionSpeed(speed: number): Promise<void> {
    if (this.busy || !Number.isFinite(speed)) return;
    const bounded = Math.round(Math.max(0, Math.min(100, speed)));
    if (bounded === this.auroraMotionSpeed) return;
    this.busy = true;
    try { this.apply(await setAppearance(this.theme, this.accent, this.background, bounded)); }
    catch (cause: unknown) { this.error = backendError(cause, "Aurora motion could not be saved."); }
    finally { this.busy = false; }
  }

  /** Switches the accent: Rust derives and validates, then this applies it. */
  async setAccent(accent: AccentSelection): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      this.apply(await setAppearance(this.theme, accent, this.background, this.auroraMotionSpeed));
    } catch (cause: unknown) {
      this.error = backendError(cause, "The accent could not be saved.");
    } finally {
      this.busy = false;
    }
  }
}

export const appearance = new AppearanceStore();
