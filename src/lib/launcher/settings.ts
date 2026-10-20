/** UI-only category keys; existing deep links keep their established keys. */
export const settingsCategories = [
  { id: 'Appearance', label: 'Appearance', detail: 'Theme, Borealis and accent' },
  { id: 'Home', label: 'Home & Widgets', detail: 'Visibility and layout' },
  { id: 'General', label: 'Launcher', detail: 'Desktop integration' },
  { id: 'Updates', label: 'Updates', detail: 'Launcher and Aurora Client' },
  { id: 'Discord', label: 'Discord', detail: 'Activity' },
  { id: 'Accessibility', label: 'Accessibility', detail: 'System visual preferences' },
  { id: 'About', label: 'About', detail: 'Version and platform' },
] as const;
export type SettingsCategory = typeof settingsCategories[number]['id'];

export function categoryAtKey(current: SettingsCategory, key: string): SettingsCategory | null {
  const index = settingsCategories.findIndex(item => item.id === current);
  if (key === 'Home') return settingsCategories[0].id;
  if (key === 'End') return settingsCategories.at(-1)!.id;
  const direction = key === 'ArrowDown' || key === 'ArrowRight' ? 1 : key === 'ArrowUp' || key === 'ArrowLeft' ? -1 : 0;
  return direction ? settingsCategories[(index + direction + settingsCategories.length) % settingsCategories.length].id : null;
}
