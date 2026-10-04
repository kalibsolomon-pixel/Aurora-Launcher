import { createServer } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
const server = await createServer({ configFile: false, root: fileURLToPath(new URL('.', import.meta.url)), publicDir: `${root}/static`, plugins: [svelte()], resolve: { alias: { '@tauri-apps/api/core': `${root}/tools/visual/review-core.ts`, '@tauri-apps/api/event': `${root}/tools/visual/review-event.ts`, $lib: `${root}/src/lib` } }, server: { host: '127.0.0.1', port: 1422, strictPort: true, fs: { allow: [root] } } });
await server.listen();
server.printUrls();
