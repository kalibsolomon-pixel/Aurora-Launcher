import type { ModEntry } from "../backend";

/** Only verified provider provenance supplies a project identity. */
export function artworkProject(entry: Pick<ModEntry, "ownership" | "provenance">): string | null {
  return entry.ownership === "providerManaged" && entry.provenance?.provider === "modrinth" ? entry.provenance.projectId : null;
}

export class ArtworkCache {
  private entries = new Map<string, { expires: number; promise: Promise<string | null> }>();
  private now: () => number;
  constructor(now = () => Date.now()) { this.now = now; }
  get(id: string, fetch: (id: string) => Promise<string | null>): Promise<string | null> {
    const existing = this.entries.get(id);
    if (existing && existing.expires > this.now()) return existing.promise;
    const promise = fetch(id).catch(() => null);
    const entry = { expires: this.now() + 600_000, promise };
    this.entries.delete(id);
    this.entries.set(id, entry);
    while(this.entries.size > 128) this.entries.delete(this.entries.keys().next().value!);
    void promise.then(url => { if (!url) entry.expires = this.now() + 60_000; });
    return promise;
  }
}
