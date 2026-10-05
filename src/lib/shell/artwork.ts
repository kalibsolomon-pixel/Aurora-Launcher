/** Defense in depth for native-validated static PNG DTOs. No direct remote sources. */
export function artworkSource(value: string | null | undefined): string | null {
  if (!value || value.length > 700_000) return null;
  return /^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(value) ? value : null;
}

export function artworkSymbol(fallback: string): string {
  return ({ M: 'mod', J: 'file', R: 'resourcePack', S: 'shader', P: 'modpack', server: 'server' } as Record<string, string>)[fallback] ?? 'file';
}
