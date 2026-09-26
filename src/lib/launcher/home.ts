import type { LaunchProcess, PlayReadiness } from "../backend";

/** Presentation of the native decision, never a second readiness calculation. */
export function homeLaunchState(
  instanceId: string | null,
  readiness: PlayReadiness | null,
  process: LaunchProcess | null,
  preparing: boolean,
  checking: boolean,
  selecting: boolean,
) {
  const decision = readiness?.instanceId === instanceId ? readiness : null;
  const child = process?.instanceId === instanceId ? process : null;
  const phase = child?.status === "running" || decision?.processStatus === "running" ? "Running"
    : child?.status === "starting" || decision?.processStatus === "starting" || preparing ? "Starting…" : null;
  return {
    playLabel: phase ?? "Play",
    disabled: !instanceId || !!phase || checking || selecting || !decision?.ready,
    label: phase ?? (checking || selecting ? "Checking…" : decision?.ready ? "Ready" : "Needs attention"),
    tone: phase === "Running" || (!phase && !checking && !selecting && decision?.ready)
      ? "status-success" : phase || checking || selecting ? "status-working" : "status-warning",
    blockers: decision?.blockers ?? [],
    exitDetail: child?.status === "exited" && child.exitCode !== null ? `Last game exited with code ${child.exitCode}.` : null,
    failure: child?.status === "failed" ? child.message ?? "Minecraft exited unsuccessfully." : null,
  };
}
