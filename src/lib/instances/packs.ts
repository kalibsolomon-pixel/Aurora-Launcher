import type { ContentEntry } from "$lib/backend";

export type PackFilter = "all" | "enabled" | "disabled" | "warnings" | "managed";

export interface PackRemovalCandidate {
  entryId: string;
  displayName: string;
  fileName: string;
}

export interface PackRemovalPlan {
  /** "local" — entry-level file removal; "provider" — graph preview/transaction. */
  route: "local" | "provider";
  candidate: PackRemovalCandidate;
  blockedReason: string | null;
}

/**
 * Maps the backend-owned capability statement onto the row's removal action.
 * The native `management.removalPath` decides which command path owns the
 * artifact; this layer never guesses from ownership or provider strings.
 */
export function packRemovalPlan(entry: ContentEntry): PackRemovalPlan | null {
  const candidate = {
    entryId: entry.entryId,
    displayName: entry.displayName,
    fileName: entry.fileName,
  };
  switch (entry.management.removalPath) {
    case "localFile":
      return { route: "local", candidate, blockedReason: null };
    case "providerGraph":
      return { route: "provider", candidate, blockedReason: null };
    default:
      return {
        route: "local",
        candidate,
        blockedReason: entry.management.removalBlockedReason,
      };
  }
}

/** The concise at-a-glance state chip for one collapsed pack row. */
export function packStateLabel(entry: ContentEntry): string {
  const management = entry.management;
  if (management.activationManagedInGame) return "Activation managed in-game";
  if (management.active === true) return "Enabled";
  if (management.active === false) return "Disabled";
  return management.canToggle ? "Disabled" : "Activation unavailable";
}

/** Why a toggle is absent, in one short tooltip-safe line (null: show toggle). */
export function packToggleUnavailableReason(entry: ContentEntry): string | null {
  if (entry.management.canToggle) return null;
  if (entry.management.activationManagedInGame) {
    return "Shader packs are activated in-game with a compatible shader loader.";
  }
  return entry.management.toggleBlockedReason;
}

/** Reconciles an entry-level confirmation against the current snapshot. */
export function confirmedLocalRemoval(
  candidate: PackRemovalCandidate | null,
  entries: readonly ContentEntry[],
): string | null {
  if (!candidate) return null;
  const current = entries.find((entry) => entry.entryId === candidate.entryId);
  return current?.management.removalPath === "localFile" && current.management.canRemove
    ? current.entryId
    : null;
}

/** Pure client-side projection over one already-loaded native snapshot. */
export function visiblePacks(
  entries: readonly ContentEntry[],
  query: string,
  filter: PackFilter,
): ContentEntry[] {
  const needle = query.trim().toLocaleLowerCase();
  return entries.filter((entry) => {
    if (filter === "enabled" && entry.management.active !== true) return false;
    if (filter === "disabled" && entry.management.active !== false) return false;
    if (filter === "warnings" && entry.warnings.length === 0) return false;
    if (filter === "managed" && entry.ownership !== "providerManaged") return false;
    if (needle === "") return true;
    const searchable = [entry.displayName, entry.fileName, entry.provenance?.provider ?? ""]
      .join("\n")
      .toLocaleLowerCase();
    return searchable.includes(needle);
  });
}

/** The concise version/source segment for a collapsed row. */
export function packSourceLabel(entry: ContentEntry): string {
  const version =
    entry.provenance?.displayVersion
      ?? (entry.fileType === "directory" ? "Folder" : entry.fileType === "zip" ? "ZIP" : "File");
  const managed = entry.provenance ? `Managed · ${entry.provenance.provider}` : "Local";
  return `${managed} · ${version}`;
}

export function formatPackSize(bytes: number | null): string | null {
  if (bytes === null) return null;
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}
