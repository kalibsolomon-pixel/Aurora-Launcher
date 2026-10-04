/** Defense in depth for native cosmetic DTOs. No file paths, SVG or arbitrary hosts. */
export function artworkSource(value: string | null | undefined, provider = false): string | null {
  if (!value || value.length > 700_000) return null;
  if (/^data:image\/(?:png|jpeg|gif|webp);base64,[A-Za-z0-9+/]+={0,2}$/.test(value)) return value;
  if (!provider) return null;
  try {
    const url = new URL(value);
    return url.protocol === 'https:' && url.hostname === 'cdn.modrinth.com' && !url.port &&
      !url.username && !url.password && !url.hash && url.pathname.startsWith('/data/') ? url.href : null;
  } catch { return null; }
}

export function artworkSymbol(fallback: string): string {
  return ({ M: 'mod', J: 'file', R: 'resourcePack', S: 'shader', P: 'modpack', server: 'server' } as Record<string, string>)[fallback] ?? 'file';
}
