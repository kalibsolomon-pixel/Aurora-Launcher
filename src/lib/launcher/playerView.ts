// Kept dependency-free for the plain Node test runner. Pixel-equivalence tests
// bind these defaults to the shared Home renderer's canonical projection.
export const DEFAULT_VIEW = { yaw: -.42, pitch: .10, zoom: 1 };
export const MIN_ZOOM = .75, MAX_ZOOM = 1.15;
const clamp = (value: number, min: number, max: number, fallback: number) => Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
/** Appearance-only camera policy. No change to the Home projection defaults. */
export function playerView(yaw: number, pitch: number, zoom: number) {
  return { yaw: Number.isFinite(yaw) ? ((yaw + Math.PI) % (2 * Math.PI) + 2 * Math.PI) % (2 * Math.PI) - Math.PI : DEFAULT_VIEW.yaw,
    pitch: clamp(pitch, -.35, .4, DEFAULT_VIEW.pitch), zoom: clamp(zoom, MIN_ZOOM, MAX_ZOOM, DEFAULT_VIEW.zoom) };
}
