import { skinPresetPreview, type HeadAvatar, type SkinPreset } from '$lib/backend';
const textures = new Map<string, Promise<HeadAvatar>>();
/** Bounded presentation reuse. Native revalidates every acquired object. */
export function savedSkinTexture(preset: SkinPreset, refresh = false): Promise<HeadAvatar> {
  const key = `${preset.id}:${preset.sha256}:${preset.model}`;
  if (refresh) textures.delete(key);
  let pending = textures.get(key);
  if (!pending) {
    pending = skinPresetPreview(preset.id).catch(error => { textures.delete(key); throw error; });
    textures.set(key, pending);
    if (textures.size > 64) textures.delete(textures.keys().next().value!);
  }
  return pending;
}
