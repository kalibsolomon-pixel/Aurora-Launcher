// Fresh disposable optimizer cache; never removes or reuses developer caches.
import {createServer} from 'vite';
import {svelte} from '@sveltejs/vite-plugin-svelte';
import {fileURLToPath} from 'node:url';
import {mkdtempSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
const root=fileURLToPath(new URL('../../',import.meta.url));
const cacheDir=mkdtempSync(join(tmpdir(),'aurora-p2-vite-'));
const server=await createServer({configFile:false,cacheDir,root:fileURLToPath(new URL('.',import.meta.url)),publicDir:`${root}/static`,plugins:[svelte()],resolve:{alias:{'@tauri-apps/api/core':`${root}/tools/visual/review-core.ts`,'@tauri-apps/api/event':`${root}/tools/visual/review-event.ts`,$lib:`${root}/src/lib`}},server:{host:'127.0.0.1',port:1422,strictPort:true,fs:{allow:[root,cacheDir]}}});
await server.listen();server.printUrls();
