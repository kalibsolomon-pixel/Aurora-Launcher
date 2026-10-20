import type {
  InstanceConfiguration,
  InstanceProgressEvent,
  InstanceSummary,
  InstanceValidationDto,
  LaunchProgressEvent,
  PlayReadiness,
  RuntimeProgressEvent,
  RuntimeStatusDto,
} from "$lib/backend";

/**
 * Pure presentation derivations over Rust-owned instance state, shared by
 * Home, the Instances list, and the instance workspace so every surface
 * makes the SAME decision for the SAME underlying state. No readiness is
 * reconstructed here beyond what these DTOs already carry: deep validation
 * outcomes come from `validate_instance`, and the configuration-versus-pin
 * comparison mirrors the backend's stale gate for display.
 *
 * Svelte-free and backend-import-free at runtime (erased `import type`
 * only), so the derivations are deterministically testable with the plain
 * Node test runner.
 */

export type InstanceStatusTone =
  | "status-success"
  | "status-warning"
  | "status-error"
  | "status-working"
  | "status-muted";

export interface InstanceStatusDecision {
  tone: InstanceStatusTone;
  label: string;
  detail: string | null;
  /** Raw native diagnostic (may contain paths); shown only behind a disclosure. */
  technical?: string | null;
}

/**
 * True when the saved desired configuration differs from the installed
 * release pin — the display half of the backend's stale gate. Mirrors the
 * comparison `validate_instance` performs; it decides emphasis and the
 * "install new configuration" action, never launchability (that is
 * `get_play_readiness`'s decision alone).
 */
export function configurationRequiresInstall(instance: InstanceSummary): boolean {
  if (instance.state !== "ready") return false;
  return (
    instance.configuration.minecraftVersion !== instance.minecraftVersion ||
    instance.configuration.loader.kind !== instance.platform.kind ||
    instance.configuration.auroraEnabled !== (instance.aurora !== null) ||
    ("policy" in instance.configuration.loader && instance.configuration.loader.policy.type === "pinned" &&
      instance.configuration.loader.policy.version !== ("version" in instance.platform ? instance.platform.version : undefined))
  );
}

const lifecyclePhaseLabels: Record<InstanceProgressEvent["phase"], string> = {
  resolvingRelease: "Resolving the Aurora release",
  resolvingGame: "Resolving Minecraft",
  installingGame: "Installing Minecraft",
  installingAurora: "Installing Aurora Client",
  validating: "Validating the instance",
  completing: "Finishing",
};
const gamePhaseLabels: Record<NonNullable<InstanceProgressEvent["game"]>["phase"], string> = {
  acquiring: "Downloading and verifying files",
  materializing: "Copying verified files",
  extractingNatives: "Extracting native libraries",
  processing: "Running loader processors",
  validating: "Validating game files",
  committing: "Committing the installation",
};

/**
 * Human wording for one native lifecycle progress event. Counts are the
 * native item counts verbatim; no percentage or estimate is derived.
 */
export function instanceProgressLabel(event: InstanceProgressEvent | null): string {
  if (!event) return "Working";
  const lifecycle = lifecyclePhaseLabels[event.phase] ?? "Working";
  const game = event.game;
  if (!game) return `${lifecycle}…`;
  const step = gamePhaseLabels[game.phase] ?? lifecycle;
  const count = game.totalItems > 0
    ? ` · ${game.completedItems.toLocaleString("en-US")} of ${game.totalItems.toLocaleString("en-US")}`
    : "";
  return `${lifecycle}: ${step}${count}`;
}

const runtimePhaseLabels: Record<RuntimeProgressEvent["phase"], string> = {
  acquiring: "Downloading and verifying Java",
  materializing: "Copying verified Java files",
  validating: "Validating Java",
  executing: "Checking the Java executable",
  committing: "Finishing the Java installation",
};

/** Human wording for managed-Java installation progress (native counts only). */
export function runtimeProgressLabel(event: RuntimeProgressEvent): string {
  const step = runtimePhaseLabels[event.phase] ?? "Installing Java";
  return event.totalItems > 0
    ? `${step} · ${event.completedItems.toLocaleString("en-US")} of ${event.totalItems.toLocaleString("en-US")}`
    : `${step}…`;
}

const launchPhaseLabels: Record<LaunchProgressEvent["phase"], string> = {
  checkingPreconditions: "Checking readiness…",
  resolvingLaunch: "Preparing the game…",
  restoringSession: "Restoring the Minecraft session…",
  assemblingArguments: "Assembling the launch…",
  startingProcess: "Starting Minecraft…",
};

/** Human wording for a native launch progress phase. */
export function launchProgressLabel(event: LaunchProgressEvent | null): string | null {
  return event ? launchPhaseLabels[event.phase] ?? "Preparing…" : null;
}

/**
 * Header badge for an instance workspace: content status, except that when
 * Rust has a play decision for this exact instance that blocks launch while
 * content itself is fine, the badge says so instead of claiming "Ready".
 */
export function workspaceBadge(content: InstanceStatusDecision, readiness: PlayReadiness | null, instanceId: string): InstanceStatusDecision {
  if (content.tone === "status-success" && readiness?.instanceId === instanceId && !readiness.ready && readiness.processStatus !== "running" && readiness.processStatus !== "starting") {
    return { tone: "status-warning", label: "Needs attention", detail: readiness.blockers[0]?.message ?? null };
  }
  return content;
}

/**
 * The one content-status decision every surface shows for an instance.
 * Inputs are Rust-owned state only; the installing phase is the live native
 * progress phase when one is running.
 */
export function instanceContentStatus(
  instance: InstanceSummary,
  validation: InstanceValidationDto | undefined,
  installingPhase: string | null,
): InstanceStatusDecision {
  // A held installation lock reads as recovery "busy"; while this launcher's
  // own creation reports live progress, that busy lock is the creation itself.
  const ownInstallation = instance.state === "installing" && installingPhase !== null;
  if (instance.recovery && !(ownInstallation && instance.recovery.status === "busy")) {
    const { status, message } = instance.recovery;
    const labels = { checking: "Checking recovery…", busy: "Installation busy", interrupted: "Installation interrupted", previousRestored: "Previous installation restored", completed: "Recovery completed", manualRepair: "Recovery required" };
    if (status !== "completed" || instance.state !== "ready") {
      return { tone: status === "manualRepair" ? "status-error" : status === "checking" || status === "busy" ? "status-working" : "status-warning", label: labels[status], detail: message };
    }
  }
  if (instance.state === "installing") {
    return {
      tone: "status-working",
      label: "Installing",
      detail: installingPhase,
    };
  }
  if (validation?.status === "damaged") {
    return {
      tone: "status-error",
      label: "Damaged",
      detail: validation.problems[0]
        ? `${validation.problems[0].component}: ${validation.problems[0].reason}`
        : "Deep validation found problems.",
    };
  }
  if (validation?.status === "stale" || configurationRequiresInstall(instance)) {
    return {
      tone: "status-warning",
      label: "Needs install",
      detail:
        "The saved configuration differs from the installed content — install the new configuration from the instance's Settings tab.",
    };
  }
  if (validation?.status === "ready") {
    return {
      tone: "status-success",
      label: "Ready",
      detail: "Deep validation passed.",
    };
  }
  if (validation?.status === "notInstalled") {
    return {
      tone: "status-muted",
      label: "Not installed",
      detail: "No installed game content was found for this instance.",
    };
  }
  return {
    tone: "status-success",
    label: "Ready",
    detail: "Installed and complete.",
  };
}

/** Quiet one-line description of an instance's desired configuration. */
export function configurationLabel(instance: InstanceSummary): string {
  const kind = instance.configuration.loader.kind === "fabric" ? "Fabric" : instance.configuration.loader.kind === "vanilla" ? "Vanilla" : instance.configuration.loader.kind === "neoForge" ? "NeoForge" : instance.configuration.loader.kind === "forge" ? "Forge" : instance.configuration.loader.kind;
  const loader = "policy" in instance.configuration.loader
    ? instance.configuration.loader.policy.type === "pinned"
      ? `${kind} ${instance.configuration.loader.policy.version}`
      : `${kind} (${instance.configuration.auroraEnabled ? "release version" : "automatic"})`
    : kind;
  return `Minecraft ${instance.configuration.minecraftVersion} · ${loader}`;
}

/**
 * True when the draft differs from the instance's saved desired
 * configuration. Deterministic structural comparison; the dirty flag can
 * therefore be derived instead of tracked, and it can never disagree with
 * what Save would actually change. Both sides serialize through the same
 * DTO key order, including through Svelte reactive proxies.
 */
export function draftIsDirty(
  draft: InstanceConfiguration,
  configuration: InstanceConfiguration,
): boolean {
  return JSON.stringify(draft) !== JSON.stringify(configuration);
}

/**
 * The managed Java status decision shown on Home and the workspace
 * Overview. The runtime status DTO belongs to one instance; callers pass
 * null when it belongs to a different instance or was never checked.
 */
export function javaRuntimeStatus(
  runtime: RuntimeStatusDto | null,
  busy: boolean,
  error: string | null,
  progressPhase: string | null,
): InstanceStatusDecision {
  if (busy) {
    return {
      tone: "status-working",
      label: "Checking…",
      detail: progressPhase,
    };
  }
  if (error) {
    return { tone: "status-error", label: "Status failed", detail: "Java could not be installed or verified. Try again; details are below.", technical: error };
  }
  if (!runtime) {
    return { tone: "status-muted", label: "Not checked", detail: null };
  }
  if (runtime.status === "ready") {
    return {
      tone: "status-success",
      label: "Ready",
      detail:
        `${runtime.component} · Java ${runtime.requiredMajorVersion}` +
        (runtime.runtimeVersion ? ` · ${runtime.runtimeVersion}` : "") +
        (runtime.reused === true ? " · reused verified runtime" : ""),
    };
  }
  if (runtime.status === "damaged") {
    return {
      tone: "status-error",
      label: "Damaged",
      detail: runtime.problems.some(problem => problem.includes("This Java build cannot start"))
        ? "Java cannot start at this data location. Use a shorter path with ASCII characters; reinstalling Java here will not help."
        : "The managed runtime failed validation. Repair Java to reinstall it.",
      technical: runtime.problems.length ? runtime.problems.join(" · ") : null,
    };
  }
  return {
    tone: "status-warning",
    label: "Not installed",
    detail: `${runtime.component} · Java ${runtime.requiredMajorVersion}`,
  };
}

export function installedConfigurationLabel(instance: InstanceSummary): string {
  const platform = instance.platform.kind === "fabric" ? "Fabric" : instance.platform.kind === "vanilla" ? "Vanilla" : instance.platform.kind === "neoForge" ? "NeoForge" : instance.platform.kind === "forge" ? "Forge" : instance.platform.kind;
  const loader = "version" in instance.platform ? ` ${instance.platform.version}` : "";
  const aurora = instance.aurora ? ` · Aurora ${instance.aurora.version}${instance.auroraContentState && instance.auroraContentState !== "active" ? ` (${instance.auroraContentState})` : ""}` : " · Aurora Off";
  return `Minecraft ${instance.minecraftVersion} · ${platform}${loader}${aurora}`;
}
