/**
 * Instance icon vocabulary: Google Material Symbols glyph names. Pure and
 * Svelte-free so it is testable with the plain Node runner. Names are
 * presentation data only; they never reach a path, URL or command beyond the
 * validated icon store.
 */

/** Shown first in the picker: game-flavoured, distinct silhouettes. */
export const DEFAULT_INSTANCE_ICONS = [
  "deployed_code",
  "sports_esports",
  "swords",
  "shield",
  "diamond",
  "forest",
  "landscape",
  "castle",
  "cottage",
  "rocket_launch",
  "bolt",
  "local_fire_department",
  "water_drop",
  "ac_unit",
  "hiking",
  "science",
  "pets",
  "explore",
] as const;

/** Rendered when an instance has no chosen icon. */
export const FALLBACK_INSTANCE_ICON = "deployed_code";

/** Mirrors the Rust validator: 1–64 lowercase letters, digits or underscores. */
export function isValidIconName(name: string): boolean {
  return /^[a-z0-9_]{1,64}$/.test(name);
}

/** "rocket_launch" → "Rocket launch" for accessible names and tooltips. */
export function iconLabel(name: string): string {
  const words = name.replace(/_/g, " ").trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/**
 * Filter glyph names by a free-text query. Every query word must appear in
 * the name; names starting with the first word rank first. Bounded output.
 */
/**
 * Material Symbols names carry no tags, so a few everyday Minecraft words map
 * to the glyph vocabulary that depicts them ("tree" → forest, park…).
 */
const SYNONYMS: Record<string, readonly string[]> = {
  tree: ["forest", "park", "nature", "eco", "grass"],
  wood: ["forest", "park", "carpenter"],
  sword: ["swords", "sports_martial_arts"],
  pvp: ["swords", "sports_martial_arts", "shield"],
  house: ["home", "cottage", "house", "cabin"],
  base: ["home", "cottage", "castle", "fort"],
  fire: ["fire", "local_fire_department", "whatshot", "mode_heat"],
  lava: ["local_fire_department", "volcano", "whatshot"],
  ice: ["ac_unit", "severe_cold", "snowflake"],
  snow: ["ac_unit", "snowing", "severe_cold"],
  mine: ["construction", "hardware", "landslide"],
  ore: ["diamond", "deployed_code", "token"],
  gem: ["diamond", "token"],
  magic: ["auto_fix_high", "wand_stars", "star", "flare"],
  space: ["rocket", "rocket_launch", "planet", "public"],
  build: ["construction", "architecture", "foundation", "handyman"],
  creative: ["palette", "brush", "draw"],
  survival: ["hiking", "camping", "explore"],
  ocean: ["water", "waves", "sailing", "scuba_diving"],
  sea: ["water", "waves", "sailing"],
  farm: ["agriculture", "grass", "eco", "potted_plant"],
  animal: ["pets", "cruelty_free", "flutter_dash"],
  game: ["sports_esports", "videogame_asset", "stadia_controller"],
  speed: ["bolt", "speed", "electric_bolt"],
  performance: ["speed", "bolt", "rocket_launch"],
};

export function searchIcons(names: readonly string[], query: string, limit = 240): string[] {
  const words = query.toLowerCase().trim().split(/[\s_-]+/).filter(Boolean);
  if (words.length === 0) return names.slice(0, limit);
  const matchesWord = (name: string, word: string) =>
    name.includes(word) || (SYNONYMS[word]?.some((alias) => name.includes(alias)) ?? false);
  const matches = names.filter((name) => words.every((word) => matchesWord(name, word)));
  const first = words[0];
  matches.sort((a, b) => {
    const rank = (name: string) => (name === words.join("_") ? 0 : name.startsWith(first) ? 1 : 2);
    return rank(a) - rank(b) || a.localeCompare(b);
  });
  return matches.slice(0, limit);
}
