import type { RecentGameplayTarget, RecentServerMotdSegment, RecentServerPresentation } from "$lib/backend";

/**
 * Pure presentation merging for the enriched Recent Servers widget. History
 * entries are always renderable on their own; a presentation only overlays
 * display facts and never influences the Quick Launch target id.
 */

export type PresentationMap = Map<string, RecentServerPresentation>;

export function presentationsById(presentations: RecentServerPresentation[]): PresentationMap {
  return new Map(presentations.map(presentation => [presentation.targetId, presentation]));
}

/** The strongest truthful row name: the enriched identity, else the history label. */
export function rowName(entry: RecentGameplayTarget, presentation: RecentServerPresentation | undefined): string {
  if (presentation?.name) return presentation.name;
  return entry.displayName;
}

/** Plain unstyled text of one MOTD, for tooltips and a11y labels. */
export function motdPlainText(motd: RecentServerMotdSegment[][]): string {
  return motd.map(line => line.map(segment => segment.text).join("")).filter(line => line.trim().length > 0).join(" · ");
}

/** Restrained status text appended to the meta line, empty when uninformative. */
export function statusLabel(presentation: RecentServerPresentation | undefined): string {
  if (!presentation) return "";
  if (presentation.status === "online") {
    if (presentation.playersOnline == null) return "Online";
    // Some networks publish a zero maximum; only the real count is useful.
    if (!presentation.playersMax) return `${presentation.playersOnline} online`;
    return `${presentation.playersOnline}/${presentation.playersMax} online`;
  }
  return "Offline";
}

/** Tooltip detail: MOTD, version and latency when available. */
export function rowDetail(presentation: RecentServerPresentation | undefined): string {
  if (!presentation) return "";
  const parts: string[] = [];
  const motd = motdPlainText(presentation.motd);
  if (motd) parts.push(motd);
  if (presentation.versionText) parts.push(presentation.versionText);
  if (presentation.latencyMs != null) parts.push(`${presentation.latencyMs} ms`);
  return parts.join(" · ");
}

/** Favicon tile letter for rows without a real icon. */
export function fallbackLetter(name: string): string {
  const found = name.trim().match(/[a-zA-Z0-9]/);
  return (found ? found[0] : "?").toUpperCase();
}

/** Inline style for one sanitized MOTD segment; colors arrive CSS-hex-validated from Rust. */
export function segmentStyle(segment: RecentServerMotdSegment): string {
  const styles: string[] = [];
  if (segment.color) styles.push(`color:${segment.color}`);
  if (segment.bold) styles.push("font-weight:600");
  if (segment.italic) styles.push("font-style:italic");
  if (segment.underline) styles.push("text-decoration:underline");
  return styles.join(";");
}
