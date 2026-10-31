// Build-time only. Renders the stylized Borealis layer: a few broad,
// slightly irregular translucent bands in muted teal, cyan, blue and violet,
// pre-blurred once here so the native app never blurs at runtime.
// Usage: node native/tools/generate-borealis.mjs <absolute path to sharp package>
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
const sharp = require(process.argv[2] || 'sharp');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../assets');

// 960x600 is upscaled to the window; after blurring there is no detail to lose.
const W = 960, H = 600;
const band = (d, color, width, opacity) =>
  `<path d="${d}" fill="none" stroke="${color}" stroke-width="${width}" stroke-linecap="round" opacity="${opacity}"/>`;
const svg = Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<rect width="${W}" height="${H}" fill="#06070c"/>
${band('M -80 470 C 140 380, 260 430, 420 300 S 700 120, 1040 160', '#1d9e95', 150, 0.50)}
${band('M -60 520 C 180 470, 330 500, 520 380 S 780 250, 1040 270', '#2f9fc4', 90, 0.38)}
${band('M -100 610 C 220 560, 420 600, 640 500 S 900 420, 1060 450', '#3a55c2', 170, 0.34)}
${band('M 420 -40 C 560 60, 700 40, 820 120 S 980 200, 1060 150', '#6a48bd', 130, 0.36)}
${band('M 120 120 C 240 170, 330 120, 430 160', '#2a7fb0', 70, 0.22)}
</svg>`);
await sharp(svg)
  .blur(26)
  .png({ compressionLevel: 9 })
  .toFile(path.join(root, 'borealis-bands.png'));
console.log('Borealis layer generated.');
