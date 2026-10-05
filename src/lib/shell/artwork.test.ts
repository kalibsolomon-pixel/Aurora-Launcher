import assert from 'node:assert/strict';
import { it } from 'node:test';
import { readFileSync } from 'node:fs';
import { artworkSource, artworkSymbol } from './artwork.ts';

it('only native static PNG data is renderable, including in provider Browse', () => {
  const png = `data:image/png;base64,${readFileSync('static/favicon.png').toString('base64')}`;
  assert.equal(artworkSource(png), png);
  for (const format of ['jpeg', 'gif', 'webp']) assert.equal(artworkSource(`data:image/${format};base64,YWJjZA==`), null);
  const cdn = 'https://cdn.modrinth.com/data/AAAABBBB/icon.png';
  assert.equal(artworkSource(cdn), null);
  for (const source of [null, '', 'data:image/svg+xml;base64,YWJj', 'data:text/html;base64,YWJj',
    'data:image/png;base64,a b', 'file:///C:/secret.png', 'asset://secret.png',
    'https://example.com/icon.png', 'https://cdn.modrinth.com.evil/data/icon.png',
    'http://cdn.modrinth.com/data/icon.png', 'https://user@cdn.modrinth.com/data/icon.png',
    'https://cdn.modrinth.com:8443/data/icon.png', 'https://cdn.modrinth.com/private/icon.png',
    'data:image/png;base64,' + 'a'.repeat(700_000)]) assert.equal(artworkSource(source), null);
});

it('fallbacks are stable local vector symbols across all content types', () => {
  assert.deepEqual(['M','R','S','P','J','server'].map(artworkSymbol), ['mod','resourcePack','shader','modpack','file','server']);
});

it('production CSP permits native data images only in the image directive', () => {
  const config = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
  const directives = config.app.security.csp.split(';').map((part: string) => part.trim());
  assert.match(directives.find((part: string) => part.startsWith('img-src')), /(?:^| )data:(?: |$)/);
  assert(directives.filter((part: string) => !part.startsWith('img-src')).every((part: string) => !part.includes('data:')));
  assert.match(config.app.security.csp, /connect-src ipc: http:\/\/ipc\.localhost;/);
});
