// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const ssr = false;

// Load and apply the persisted appearance before any component renders, so
// the user's theme and accent are in place for the first paint of the shell
// (no default-theme flash). The load is await-ed by SvelteKit; a failure
// falls back to the default look without blocking the launcher.
export const load = async () => {
  const { appearance } = await import("$lib/launcher/appearance.svelte");
  await appearance.initialize();
  // Connects the native minimize signal used to pause decorative playback;
  // the subscription itself is failure-proof and defaults to "not minimized".
  const { windowActivity } = await import("$lib/shell/windowActivity");
  await windowActivity.initialize();
};
