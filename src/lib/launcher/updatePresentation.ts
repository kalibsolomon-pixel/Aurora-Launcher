/**
 * Pure presentation helpers for Aurora's update surfaces.
 *
 * Everything here is plain, testable formatting and classification: no
 * network, no native calls, no fabricated progress. Release notes are
 * treated as untrusted plain text everywhere they render.
 */

import type {
  ProductUpdateAvailability,
  UpdateReleaseChannel,
} from "$lib/backend";

export interface AvailabilitySummary {
  tone: "accent" | "quiet" | "warn";
  title: string;
  detail: string;
}

/** The user-facing summary of one update domain's availability. */
export function summarizeAvailability(
  availability: ProductUpdateAvailability,
  domain: "launcher" | "client",
): AvailabilitySummary {
  const subject = domain === "launcher" ? "Aurora Launcher" : "Aurora Client";
  switch (availability.kind) {
    case "notChecked":
      return {
        tone: "quiet",
        title: "Not checked yet",
        detail: "Check for updates to see the current release.",
      };
    case "upToDate":
      return {
        tone: "quiet",
        title: `${subject} is up to date`,
        detail: "No newer eligible release is published.",
      };
    case "updateAvailable":
      return {
        tone: "accent",
        title: `${subject} ${availability.candidate} available`,
        detail: `Installed ${availability.current} · channel ${availability.channel}`,
      };
    case "notApplicable":
      return {
        tone: "quiet",
        title: "No update target",
        detail: availability.reason,
      };
    case "unavailable":
      return {
        tone: "warn",
        title: "Could not check for updates",
        detail: availability.reason,
      };
  }
}

/** Whether an availability should surface a notification card. */
export function isUpdateAvailable(
  availability: ProductUpdateAvailability | null | undefined,
): boolean {
  return availability?.kind === "updateAvailable";
}

/** The candidate version an availability offers, when any. */
export function offeredVersion(
  availability: ProductUpdateAvailability | null | undefined,
): string | null {
  return availability?.kind === "updateAvailable" ? availability.candidate : null;
}

/** Release notes rendered as plain text, bounded for presentation. */
export function presentationNotes(
  availability: ProductUpdateAvailability | null | undefined,
  candidateNotes: string | null | undefined,
  limit = 4000,
): string | null {
  const source = candidateNotes ?? offeredNotes(availability);
  if (source === null || source === undefined) return null;
  const text = source.replace(/\r\n/g, "\n").trim();
  if (text.length === 0) return null;
  return text.length > limit ? `${text.slice(0, limit - 1)}…` : text;
}

function offeredNotes(
  availability: ProductUpdateAvailability | null | undefined,
): string | null {
  return availability?.kind === "updateAvailable" ? availability.notes : null;
}

/** Human byte formatting for real download progress only. */
export function formatBytes(bytes: number | null | undefined): string | null {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes) || bytes < 0) return null;
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KiB", "MiB", "GiB"];
  let value = bytes;
  let unit = "B";
  for (const next of units) {
    if (value < 1024) break;
    value /= 1024;
    unit = next;
  }
  return `${value >= 10 ? Math.round(value) : Math.round(value * 10) / 10} ${unit}`;
}

/** The launcher update action label for a phase; never a fake percentage. */
export function launcherPhaseLabel(
  phase: string | null | undefined,
  downloaded: number | null,
  total: number | null,
): string {
  switch (phase) {
    case "checking":
      return "Checking…";
    case "downloading": {
      const done = formatBytes(downloaded);
      const whole = formatBytes(total);
      if (done && whole) return `Downloading ${done} of ${whole}`;
      if (done) return `Downloading ${done}`;
      return "Downloading…";
    }
    case "readyToInstall":
      return "Ready to install";
    case "installing":
      return "Installing — the launcher will restart";
    default:
      return "";
  }
}

/** The Aurora Client transaction phase label. */
export function clientPhaseLabel(phase: string | null | undefined): string {
  switch (phase) {
    case "acquiring":
      return "Downloading the verified release…";
    case "staging":
      return "Staging…";
    case "activating":
      return "Activating…";
    case "validating":
      return "Validating…";
    case "committing":
      return "Committing…";
    default:
      return "";
  }
}

/** What changing the channel means, in one honest sentence. */
export const channelDescriptions: Record<UpdateReleaseChannel, string> = {
  stable: "Stable releases only.",
  beta: "Stable and beta releases.",
  nightly: "Stable, beta and nightly releases.",
};

/** Changing the channel never changes installed software. */
export const channelNote =
  "Changing the channel affects future update checks only; nothing is installed automatically.";
