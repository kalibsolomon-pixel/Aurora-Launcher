import type { ClientUpdatePreview, UpdateOverview } from "../backend.ts";

export type UpdateActionState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "noUpdate" }
  | { kind: "available" }
  | { kind: "updating"; domain: "client" | "launcher"; phase: "updating" | "downloading" | "installing" }
  | { kind: "error"; operation: "check" | "update"; recheck?: boolean };

export interface UpdateOperations {
  check(): Promise<UpdateOverview>;
  startup(): Promise<UpdateOverview>;
  preview(instanceId: string): Promise<ClientUpdatePreview>;
  apply(instanceId: string, fingerprint: string): Promise<unknown>;
  download(version: string): Promise<unknown>;
  install(version: string): Promise<unknown>;
  afterClient(): Promise<void>;
}

export interface UpdateSnapshot {
  action: UpdateActionState;
  overview: UpdateOverview | null;
  context: string;
}

export function hasUpdate(overview: UpdateOverview | null): boolean {
  return overview?.launcher.availability.kind === "updateAvailable" ||
    overview?.client.kind === "updateAvailable";
}

export function actionLabel(state: UpdateActionState): string {
  switch (state.kind) {
    case "idle": return "Check For Updates";
    case "checking": return "Checking…";
    case "noUpdate": return "None Available";
    case "available": return "Update";
    case "updating":
      return { updating: "Updating…", downloading: "Downloading…", installing: "Installing…" }[state.phase];
    case "error": return state.operation === "check" ? "Check Failed — Retry" : "Update Failed — Retry";
  }
}

export function actionDisabled(state: UpdateActionState): boolean {
  return state.kind === "checking" || state.kind === "noUpdate" || state.kind === "updating";
}

/** One presentation state machine over two independent native engines.
 * The only timer resets a manual no-update confirmation locally. No polling.
 * Snapshot notifications let the Svelte adapter stay thin and tests drive the
 * very same orchestration with deterministic native operations and timers. */
export class UpdateController {
  snapshot: UpdateSnapshot = { action: { kind: "idle" }, overview: null, context: "" };
  private timer: ReturnType<typeof setTimeout> | null = null;
  private viewOpen = false;
  private inFlight: Promise<void> | null = null;
  private startupStarted = false;
  private startupPending = false;
  private queuedCheck: Promise<void> | null = null;

  private operations: UpdateOperations;
  private changed: (state: UpdateSnapshot) => void;
  constructor(operations: UpdateOperations, changed: (state: UpdateSnapshot) => void) {
    this.operations = operations;
    this.changed = changed;
  }

  private publish(patch: Partial<UpdateSnapshot>): void {
    this.snapshot = { ...this.snapshot, ...patch };
    this.changed(this.snapshot);
  }

  private clearTimer(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }

  openView(): void { this.viewOpen = true; }
  closeView(): void {
    this.viewOpen = false;
    this.clearTimer();
    if (this.snapshot.action.kind === "noUpdate") this.publish({ action: { kind: "idle" } });
  }

  receive(overview: UpdateOverview): void {
    const active = this.snapshot.action.kind;
    this.publish({ overview });
    if (["idle", "available", "noUpdate"].includes(active) && hasUpdate(overview)) {
      this.clearTimer();
      this.publish({ action: { kind: "available" } });
    } else if (active === "available" && !hasUpdate(overview)) {
      this.publish({ action: { kind: "idle" } });
    }
  }

  startup(): Promise<void> {
    if (this.startupStarted) return this.inFlight ?? Promise.resolve();
    this.startupStarted = true;
    // A fresh manual check already satisfies the startup discovery intent.
    if (this.inFlight) return this.inFlight;
    this.startupPending = true;
    return this.exclusive(async () => {
      try { this.receive(await this.operations.startup()); }
      catch { /* Startup failures never interrupt Play or become a notice. */ }
      finally { this.startupPending = false; }
    });
  }

  private exclusive(work: () => Promise<void>): Promise<void> {
    if (this.inFlight) return this.inFlight;
    const operation = work();
    this.inFlight = operation.finally(() => { this.inFlight = null; });
    return this.inFlight;
  }

  check(): Promise<void> {
    if (this.queuedCheck) return this.queuedCheck;
    if (this.inFlight && this.startupPending) {
      this.publish({ action: { kind: "checking" }, context: "" });
      this.queuedCheck = this.inFlight.then(() => this.performCheck()).finally(() => { this.queuedCheck = null; });
      return this.queuedCheck;
    }
    if (this.inFlight) return this.inFlight;
    if (actionDisabled(this.snapshot.action)) return Promise.resolve();
    return this.performCheck();
  }

  private performCheck(): Promise<void> {
    return this.exclusive(async () => {
      this.clearTimer();
      this.publish({ action: { kind: "checking" }, context: "" });
      try {
        const overview = await this.operations.check();
        this.publish({ overview });
        const failed = [overview.launcher.availability, overview.client].some(
          (domain) => domain.kind === "unavailable" || domain.kind === "notChecked",
        );
        if (hasUpdate(overview)) {
          // One unavailable service must not prevent the independently
          // verified offer from the other domain being explicitly updated.
          const context = overview.launcher.availability.kind === "unavailable" ? "Launcher could not be checked." :
            overview.client.kind === "unavailable" ? "Aurora Client could not be checked." : "";
          this.publish({ action: { kind: "available" }, context });
        } else if (failed) {
          this.publish({ action: { kind: "error", operation: "check" }, context: "Could not check for Aurora updates." });
        } else if (this.viewOpen) {
          this.publish({ action: { kind: "noUpdate" } });
          this.timer = setTimeout(() => {
            this.timer = null;
            this.publish({ action: { kind: "idle" } });
          }, 5000);
        } else {
          this.publish({ action: { kind: "idle" } });
        }
      } catch {
        this.publish({ action: { kind: "error", operation: "check" }, context: "Could not check for Aurora updates." });
      }
    });
  }

  activate(): Promise<void> {
    const state = this.snapshot.action;
    if (state.kind === "checking" && this.queuedCheck) return this.queuedCheck;
    if (actionDisabled(state)) return this.inFlight ?? Promise.resolve();
    if (state.kind === "available" || (state.kind === "error" && state.operation === "update" && !state.recheck)) return this.update();
    return this.check();
  }

  private update(): Promise<void> {
    return this.exclusive(async () => {
      const offered = this.snapshot.overview;
      if (!offered || !hasUpdate(offered)) return;
      this.clearTimer();
      let clientSucceeded = false;
      let domain: "client" | "launcher" = "client";
      let recheck = false;
      this.publish({ context: "", action: { kind: "updating", domain, phase: "updating" } });
      try {
        if (offered.client.kind === "updateAvailable") {
          const id = offered.clientInstanceId;
          if (!id) throw new Error("Missing Client target");
          const preview = await this.operations.preview(id);
          // The user approved this exact displayed release and instance.
          // New metadata cannot silently substitute another candidate.
          if (preview.instanceId !== id || preview.candidate?.version !== offered.client.candidate ||
              preview.outcome !== "updateAvailable") {
            recheck = true;
            throw new Error("Client offer changed");
          }
          if (preview.blockers.length) throw new Error("Client update blocked");
          await this.operations.apply(id, preview.fingerprint);
          clientSucceeded = true;
          this.publish({ overview: { ...offered, client: { kind: "upToDate" },
            clientInstalledVersion: preview.candidate.version }, context: "Aurora Client updated." });
          // Refresh local native state only; no second check or transaction.
          await this.operations.afterClient();
        }
        if (offered.launcher.availability.kind === "updateAvailable") {
          domain = "launcher";
          const version = offered.launcher.availability.candidate;
          this.publish({ action: { kind: "updating", domain, phase: "downloading" } });
          await this.operations.download(version);
          this.publish({ action: { kind: "updating", domain, phase: "installing" } });
          await this.operations.install(version);
          // Windows exits within the official installer. If another platform
          // returns, do not claim that the running launcher changed version.
          this.publish({ action: { kind: "idle" }, context: "Launcher update installed. Restart Aurora to finish." });
        } else {
          this.publish({ action: { kind: "idle" }, context: "Aurora Client updated." });
        }
      } catch (cause) {
        // Native stale-offer failures require fresh truth and a new explicit
        // Update action. Retry must never authorize a different version.
        if (cause && typeof cause === "object" && "code" in cause && cause.code === "update_stale") recheck = true;
        this.publish({ action: { kind: "error", operation: "update", recheck },
          context: `${clientSucceeded ? "Aurora Client updated. " : ""}${domain === "launcher" ? "Launcher" : "Aurora Client"} update failed.` });
      }
    });
  }
}
