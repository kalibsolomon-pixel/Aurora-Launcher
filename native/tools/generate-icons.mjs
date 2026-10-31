// Build-time only. The SVG is the owner-confirmed source, copied byte for byte.
// Usage: node native/tools/generate-icons.mjs <absolute path to sharp package>
//
// Outputs (native assets only; the published WebView icons are untouched):
// - aurora-mark-216.png: the sidebar mark, pre-rasterized at 4x its 54 px
//   display width so it stays crisp at every Windows display scale.
// - icon.png / icon-<size>.png / icon.ico: the application icon, the ribbon
//   centered on a simple black-gradient squircle with transparent outer
//   bounds (NATIVE-8).
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
const sharp = require(process.argv[2] || 'sharp');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../assets');
const svg = await fs.readFile(path.join(root, 'aurora.svg'));

// Sidebar mark: 640x512 viewBox -> 216x173.
await sharp(svg, { density: 288 })
  .resize(216, 173, { fit: 'contain', background: '#00000000' })
  .png()
  .toFile(path.join(root, 'aurora-mark-216.png'));

// Squircle (superellipse, n = 5) on a 1024 canvas with a 40 px transparent
// margin, filled with a restrained top-to-bottom black gradient.
const size = 1024, margin = 40, half = (size - 2 * margin) / 2, center = size / 2, n = 5;
const points = [];
for (let i = 0; i < 720; i++) {
  const t = (i / 720) * 2 * Math.PI;
  const c = Math.cos(t), s = Math.sin(t);
  const x = Math.sign(c) * Math.pow(Math.abs(c), 2 / n) * half + center;
  const y = Math.sign(s) * Math.pow(Math.abs(s), 2 / n) * half + center;
  points.push(`${x.toFixed(2)},${y.toFixed(2)}`);
}
const squircle = Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1">
<stop offset="0" stop-color="#25262e"/><stop offset="1" stop-color="#060609"/>
</linearGradient></defs>
<polygon points="${points.join(' ')}" fill="url(#g)"/>
</svg>`);
// The ribbon keeps its silhouette; 600 px wide leaves balanced inset.
const ribbon = await sharp(svg, { density: 288 })
  .resize(600, 480, { fit: 'inside' })
  .png()
  .toBuffer();
const meta = await sharp(ribbon).metadata();
const master = await sharp(squircle)
  .composite([{ input: ribbon, left: Math.round((size - meta.width) / 2), top: Math.round((size - meta.height) / 2) }])
  .png()
  .toBuffer();
await fs.writeFile(path.join(root, 'icon.png'), master);
const sizes = [40, 16, 20, 24, 32, 48, 64, 128, 256];
const frames = [];
for (const frameSize of sizes) {
  const frame = await sharp(master).resize(frameSize, frameSize, { kernel: 'lanczos3' }).png().toBuffer();
  frames.push(frame);
  await fs.writeFile(path.join(root, `icon-${frameSize}.png`), frame);
}
const header = Buffer.alloc(6 + 16 * frames.length);
header.writeUInt16LE(1, 2); header.writeUInt16LE(frames.length, 4);
let offset = header.length;
for (let i = 0; i < frames.length; i++) {
  const at = 6 + i * 16, frameSize = sizes[i];
  header[at] = frameSize % 256; header[at + 1] = frameSize % 256;
  header.writeUInt16LE(1, at + 4); header.writeUInt16LE(32, at + 6);
  header.writeUInt32LE(frames[i].length, at + 8); header.writeUInt32LE(offset, at + 12);
  offset += frames[i].length;
}
await fs.writeFile(path.join(root, 'icon.ico'), Buffer.concat([header, ...frames]));
console.log('Native sidebar mark, squircle app icon PNGs and multi-resolution ICO generated.');
