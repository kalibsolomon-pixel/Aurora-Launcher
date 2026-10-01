import type { BrowseKind, ModrinthSearchPage } from "../backend";

export class DefaultBrowseCache {
  private readonly pages = new Map<string, { page: ModrinthSearchPage; at: number }>();
  private readonly ttlMs: number;

  constructor(ttlMs = 45_000) { this.ttlMs = ttlMs; }

  get(instanceId: string, kind: BrowseKind, minecraftVersion: string, now = Date.now()): ModrinthSearchPage | null {
    const key = `${instanceId}:${kind}:${minecraftVersion}`;
    const cached = this.pages.get(key);
    if (!cached) return null;
    if (now - cached.at >= this.ttlMs) {
      this.pages.delete(key);
      return null;
    }
    return cached.page;
  }

  put(instanceId: string, kind: BrowseKind, minecraftVersion: string, page: ModrinthSearchPage, now = Date.now()): void {
    this.pages.set(`${instanceId}:${kind}:${minecraftVersion}`, { page, at: now });
  }
}

export function appendBrowsePage(
  current: ModrinthSearchPage | null,
  incoming: ModrinthSearchPage,
  requestedOffset: number,
): ModrinthSearchPage {
  return requestedOffset === 0 || !current
    ? incoming
    : { ...incoming, hits: [...current.hits, ...incoming.hits] };
}

/* Presentation only: provider category slugs become readable labels while the
   provider value sent in requests stays byte-for-byte identical. Compound
   words the provider itself renders with a kept hyphen (Modrinth's
   "Vanilla-like", "Semi-Realistic") keep that form, matching the source
   presentation. */
const hyphenatedCompoundParts = new Set(["semi", "like"]);

export function formatCategoryLabel(category: string): string {
  const parts = category.trim().split(/[-_\s]+/).filter(Boolean);
  if (parts.length === 0) return category;
  let label = parts[0].charAt(0).toUpperCase() + parts[0].slice(1);
  let joinsByHyphen = hyphenatedCompoundParts.has(parts[0].toLowerCase()) && parts[0].toLowerCase() !== "like";
  for (let index = 1; index < parts.length; index += 1) {
    const isLikeSuffix = parts[index].toLowerCase() === "like";
    const capitalized = parts[index].charAt(0).toUpperCase() + parts[index].slice(1);
    label += (joinsByHyphen || isLikeSuffix ? "-" : " ") + (isLikeSuffix ? parts[index] : capitalized);
    joinsByHyphen = hyphenatedCompoundParts.has(parts[index].toLowerCase()) && !isLikeSuffix;
  }
  return label;
}

type Timer = ReturnType<typeof setTimeout>;
type Schedule = (callback: () => void, delay: number) => Timer;
type Cancel = (timer: Timer) => void;

export class TransientNotice {
  private fadeTimer: Timer | null = null;
  private removeTimer: Timer | null = null;
  private readonly update: (message: string, exiting: boolean) => void;
  private readonly schedule: Schedule;
  private readonly cancel: Cancel;

  constructor(
    update: (message: string, exiting: boolean) => void,
    schedule: Schedule = (callback, delay) => setTimeout(callback, delay),
    cancel: Cancel = (timer) => clearTimeout(timer),
  ) { this.update = update; this.schedule = schedule; this.cancel = cancel; }

  show(message: string): void {
    this.dispose();
    this.update(message, false);
    this.fadeTimer = this.schedule(() => this.update(message, true), 2700);
    this.removeTimer = this.schedule(() => {
      this.update("", false);
      this.fadeTimer = null;
      this.removeTimer = null;
    }, 3000);
  }

  dispose(): void {
    if (this.fadeTimer !== null) this.cancel(this.fadeTimer);
    if (this.removeTimer !== null) this.cancel(this.removeTimer);
    this.fadeTimer = null;
    this.removeTimer = null;
  }
}
