import {readFile} from 'node:fs/promises';
// Execute the approved UI-1 suite unchanged except its output destination.
// Preserve its historical screenshots and functional report byte-for-byte.
const source = (await readFile(new URL('./ui1.test.mjs',import.meta.url),'utf8'))
  .replaceAll("'./ui1-fixture.mjs'", JSON.stringify(new URL('./ui1-fixture.mjs',import.meta.url).href))
  .replaceAll('docs/ui-1/', 'docs/ui-2/ui1-regression/');
await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
