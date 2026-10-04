import type { AuroraCompatibility } from '../backend';

/** Use the native release selected for this configuration, never a UI version constant. */
export function auroraCreationCopy(compatibility: AuroraCompatibility | null): string {
  if (!compatibility) return 'Checking Aurora compatibility…';
  return compatibility.available && compatibility.version
    ? `Aurora Client ${compatibility.version} is available for this configuration. Included by default when compatible; you can turn it off.`
    : compatibility.reason;
}
