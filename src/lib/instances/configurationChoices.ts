import type { AuroraCompatibility, AuroraTransitionPreview, InstanceLoader, PlatformCapability } from "../backend.ts";
export function creationPlatforms(capabilities: readonly PlatformCapability[]): PlatformCapability[] {
  return capabilities.filter(capability => capability.canCreate && capability.canInstall && capability.canValidate && capability.canLaunch);
}
export function requiresLoaderVersion(loader: InstanceLoader): boolean { return "policy" in loader && loader.policy.type === "pinned"; }
export function creationBlocked(auroraRequested: boolean, compatibility: AuroraCompatibility | null): boolean { return auroraRequested && compatibility?.available !== true; }
export function transitionRows(preview: AuroraTransitionPreview): {action: string; path: string; reason: string}[] {
  return [
    ...preview.install.map(file => ({action: "Install / reconcile", path: file.relativePath, reason: file.reason})),
    ...preview.remove.map(file => ({action: "Remove", path: file.relativePath, reason: file.reason})),
    ...preview.retain.map(file => ({action: "Retain", path: file.relativePath, reason: file.reason})),
  ];
}
export async function executeApprovedTransition<T>(preview: AuroraTransitionPreview, apply: (id: string, enabled: boolean, fingerprint: string) => Promise<T>, refresh: () => Promise<void>): Promise<T> {
  if (preview.blockers.length) throw new Error(preview.blockers.join(" "));
  const result = await apply(preview.instanceId, preview.target.aurora !== null, preview.fingerprint);
  await refresh();
  return result;
}
