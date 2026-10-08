import type { ModEntry } from "../backend";

/** Only verified provider provenance supplies a project identity. */
export function artworkProject(entry: Pick<ModEntry, "ownership" | "provenance">): string | null {
  return entry.ownership === "providerManaged" && entry.provenance?.provider === "modrinth" ? entry.provenance.projectId : null;
}

/** Shared in-flight and completed results. A failure has a retry deadline,
 * never an immortal rejected promise; retries are owned by mounted consumers. */
export class ResolutionCache<T extends { retryAfterMs: number | null }> {
  private entries = new Map<string, { expires: number; promise: Promise<T> }>();
  private now: () => number;
  constructor(now = () => Date.now()) { this.now = now; }
  get(key: string, fetch: () => Promise<T>): Promise<T> {
    const existing = this.entries.get(key);
    if (existing && existing.expires > this.now()) return existing.promise;
    const entry = { expires: Infinity, promise: Promise.resolve().then(fetch) };
    this.entries.set(key, entry);
    while (this.entries.size > 128) this.entries.delete(this.entries.keys().next().value!);
    void entry.promise.then(result => { entry.expires = this.now() + (result.retryAfterMs ?? 600_000); }, () => {
      entry.expires = this.now() + 60_000;
    });
    return entry.promise;
  }
}
