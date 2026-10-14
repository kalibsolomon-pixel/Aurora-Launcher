"""Matched screenshot sheets; originals remain unmodified and separately labeled."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import json

root = Path(__file__).resolve().parents[2] / 'docs' / 'ui-3'
font = ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf', 18)
small = ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf', 14)

def sheet(name, pairs, title):
    width, thumb_w, gap, label_h = 1440, 696, 16, 32
    rows = []
    for label, before, after in pairs:
        images = [Image.open(p).convert('RGB') for p in [before, after]]
        thumbs = []
        for im in images:
            height = round(im.height * thumb_w / im.width)
            thumbs.append(im.resize((thumb_w, height), Image.Resampling.LANCZOS))
        rows.append((label, thumbs, max(i.height for i in thumbs)))
    canvas = Image.new('RGB', (width, 70 + sum(h + label_h + gap for _, _, h in rows)), '#101116')
    draw = ImageDraw.Draw(canvas)
    draw.text((16, 10), title, fill='#edf1fb', font=font)
    draw.text((16, 38), 'BEFORE - baseline 7c76626', fill='#a7b0c5', font=small)
    draw.text((728, 38), 'AFTER - UI-3', fill='#a7b0c5', font=small)
    y = 70
    for label, images, height in rows:
        draw.text((16, y), label, fill='#edf1fb', font=font)
        y += label_h
        canvas.paste(images[0], (16, y))
        canvas.paste(images[1], (728, y))
        y += height + gap
    canvas.save(root / name)

for surface in ['mods', 'modpacks', 'packs', 'installed-mods', 'installed-packs']:
    pairs = [(f'{surface} - {w}x{h} CSS viewport - deterministic browser fixture', root/'before'/f'{surface}-{w}x{h}.png', root/'after'/f'{surface}-{w}x{h}.png') for w,h in [(1440,900),(1280,800),(1024,768),(720,520)]]
    sheet(f'{surface}-comparison.png', pairs, f'UI-3 / {surface} / matched browser screenshots (not native)')
sheet('states-comparison.png', [(f'Mods {state} - 1280x800 - deterministic browser fixture',root/'before'/f'mods-{state}.png',root/'after'/f'mods-{state}.png') for state in ['empty','error','loading']], 'UI-3 / empty, error and loading / matched browser fixtures')

before = json.loads((root/'before/geometry.json').read_text())
after = json.loads((root/'after/geometry.json').read_text())
changes = {}
for key in before:
    a, b = before[key], after[key]
    changes[key] = {'beforeColumns': a['columns'], 'afterColumns': b['columns'], 'beforeFirstRowHeight': a['rows'][0]['height'] if a['rows'] else None, 'afterFirstRowHeight': b['rows'][0]['height'] if b['rows'] else None, 'beforeOverflow': a['documentOverflow'] or a['contentOverflow'], 'afterOverflow': b['documentOverflow'] or b['contentOverflow']}
(root/'geometry-comparison.json').write_text(json.dumps(changes,indent=2))
print('Created 6 matched comparison sheets and geometry comparison; originals unmodified.')
