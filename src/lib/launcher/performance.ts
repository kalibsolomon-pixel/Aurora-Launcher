/** P0.2 research build only. Numeric records and closed events; no identities. */
import { invoke } from "@tauri-apps/api/core";

const enabled = import.meta.env?.MODE === "performance-p0-2";
type Event = "initialize" | "runtimeRequest" | "runtimeComplete" | "readinessRequest" |
  "readinessComplete" | "checking" | "ready" | "readinessDiscarded" | "playRequest" |
  "playComplete" | "instanceGeneration" | "accountGeneration" | "ipcRequest" | "ipcComplete" | "frontendOverflow";
export interface Probe { correlation: number; instanceGeneration: number; accountGeneration: number }
let sequence = 0;
let instanceGeneration = 0;
let accountGeneration = 0;
let records = 0;
const limit = 2_048;
let previousInstance: string | undefined;
let previousAccount: string | null | undefined;

export function mark(event: Event, probe?: Probe): void {
  if (!enabled) return;
  if (records >= limit) {
    if (records++ === limit) {
      void invoke("performance_mark", {
        event: "frontendOverflow", correlation: 0, instanceGeneration,
        accountGeneration, clientMs: performance.now(),
      }).catch(() => {});
    }
    return;
  }
  records++;
  void invoke("performance_mark", {
    event, correlation: probe?.correlation ?? 0,
    instanceGeneration: probe?.instanceGeneration ?? instanceGeneration,
    accountGeneration: probe?.accountGeneration ?? accountGeneration,
    clientMs: performance.now(),
  }).catch(() => { /* Optional research failure never changes a decision. */ });
}

export function generation(kind: "instance" | "account"): void {
  if (!enabled) return;
  if (kind === "instance") { instanceGeneration++; mark("instanceGeneration"); }
  else { accountGeneration++; mark("accountGeneration"); }
}

export function startProbe(kind: "runtime" | "readiness" | "play", instance: string, account?: string | null): Probe | undefined {
  if (!enabled) return;
  // Compare existing presentation identities in memory only. No identifier
  // enters a record. Synchronize before dispatch, rather than a later effect.
  if (instance !== previousInstance) { previousInstance = instance; generation("instance"); }
  if (account !== undefined && account !== previousAccount) { previousAccount = account; generation("account"); }
  const probe = { correlation: ++sequence, instanceGeneration, accountGeneration };
  mark(kind === "runtime" ? "runtimeRequest" : kind === "readiness" ? "readinessRequest" : "playRequest", probe);
  return probe;
}

export function endProbe(kind: "runtime" | "readiness" | "play", probe: Probe | undefined): void {
  if (!enabled) return;
  mark(kind === "runtime" ? "runtimeComplete" : kind === "readiness" ? "readinessComplete" : "playComplete", probe);
}

/** Separate observer cohort only: real harmless IPC, bounded to 50 requests. */
export function ipcSeries(): void {
  if (!enabled || import.meta.env.VITE_AURORA_P0_2_IPC !== "1") return;
  let count = 0;
  const timer = setInterval(() => {
    const probe = { correlation: ++sequence, instanceGeneration, accountGeneration };
    mark("ipcRequest", probe);
    void invoke("get_application_status").finally(() => mark("ipcComplete", probe)).catch(() => {});
    if (++count === 50) clearInterval(timer);
  }, 100);
}
