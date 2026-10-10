import type { HeadAvatar, SkinPreset } from '../backend';

export function libraryEntries(presets: readonly SkinPreset[], query: string, favorites: boolean, sort: 'recent' | 'name'): SkinPreset[] {
  const search = query.trim().toLocaleLowerCase();
  return presets.filter(p => (!favorites || p.favorite) && (!search || p.name.toLocaleLowerCase().includes(search)))
    .sort((a,b) => (sort === 'name' ? a.name.localeCompare(b.name) : b.importedAt - a.importedAt) || a.id.localeCompare(b.id));
}
export function isCurrentSkin(preset: SkinPreset, current: HeadAvatar | null | undefined): boolean {
  return !!current && preset.sha256 === current.sha256 && preset.model === current.model;
}
export function validSkinName(name: string): boolean {
  const value = name.trim();
  return value.length > 0 && new TextEncoder().encode(value).length <= 80 && !/[\u0000-\u001f\u007f-\u009f]/u.test(value);
}
