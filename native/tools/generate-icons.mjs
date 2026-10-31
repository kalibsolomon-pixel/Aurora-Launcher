// Build-time only. The SVG is the owner-confirmed source, copied byte for byte.
// Usage: node native/tools/generate-icons.mjs <absolute path to sharp package>
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
const sharp = require(process.argv[2] || 'sharp');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../assets');
const svg = await fs.readFile(path.join(root, 'aurora.svg'));
// Transparent padding, with no new artwork or effects. SVG aspect ratio survives.
const master = await sharp(svg, { density: 288 }).resize(896, 896, { fit: 'inside' })
  .extend({ top: 64, bottom: 64, left: 64, right: 64, background: '#00000000' })
  .resize(1024, 1024, { fit: 'contain', background: '#00000000' }).png().toBuffer();
await fs.writeFile(path.join(root, 'icon.png'), master);
const sizes = [40, 16, 20, 24, 32, 48, 64, 128, 256];
const frames = [];
for (const size of sizes) {
  const frame = await sharp(master).resize(size, size).png().toBuffer();
  frames.push(frame);
  await fs.writeFile(path.join(root, `icon-${size}.png`), frame);
}
const header = Buffer.alloc(6 + 16 * frames.length);
header.writeUInt16LE(1, 2); header.writeUInt16LE(frames.length, 4);
let offset = header.length;
for (let i = 0; i < frames.length; i++) {
  const at = 6 + i * 16, size = sizes[i];
  header[at] = size % 256; header[at + 1] = size % 256;
  header.writeUInt16LE(1, at + 4); header.writeUInt16LE(32, at + 6);
  header.writeUInt32LE(frames[i].length, at + 8); header.writeUInt32LE(offset, at + 12);
  offset += frames[i].length;
}
await fs.writeFile(path.join(root, 'icon.ico'), Buffer.concat([header, ...frames]));
console.log('Native SVG, PNG and multi-resolution ICO assets generated.');
