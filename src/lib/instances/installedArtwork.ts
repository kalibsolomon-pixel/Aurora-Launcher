import { getInstalledArtworkIdentities, type InstalledArtworkIdentities, type ContentType } from '../backend';
import { ResolutionCache } from './projectArtwork';
const identityCache = new ResolutionCache<InstalledArtworkIdentities>();
export function installedArtworkIdentities(instanceId: string, kind: ContentType, revision: string) {
  return identityCache.get(`${instanceId}:${kind}:${revision}`, () => getInstalledArtworkIdentities(instanceId, kind));
}
