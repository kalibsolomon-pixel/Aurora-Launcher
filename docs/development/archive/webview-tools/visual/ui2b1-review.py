"""Compose original captures into labeled review sheets; keep originals intact."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import json

root = Path(__file__).resolve().parents[2] / 'docs/ui-2b-1'
font = ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf', 18)

def sheet(name, entries, thumb=680):
    cells = []
    for label, path in entries:
        image = Image.open(root / path).convert('RGB')
        image.thumbnail((thumb, 500))
        cells.append((label, image))
    height = max(im.height for _, im in cells) + 48
    out = Image.new('RGB', (2 * (thumb + 16), ((len(cells)+1)//2) * height), '#101116')
    draw = ImageDraw.Draw(out)
    for i, (label, image) in enumerate(cells):
        x, y = (i % 2) * (thumb+16), (i // 2) * height
        draw.text((x+8,y+10),label,fill='#edf1fb',font=font)
        out.paste(image,(x+8,y+38))
    out.save(root / name)

sheet('home-comparison.png',[(f'{phase.upper()} | {size} | browser fixture',f'{phase}/home-{size}.png') for size in ['1440x900','1280x800','1024x768','720x520'] for phase in ['before','after']])
sheet('states-comparison.png',[(f'{phase.upper()} | {state} | 1280x800 fixture',f'{phase}/{state}-1280x800.png') for state in ['blocked','running','missing-skin','sparse','dense','oled-simple','cyan'] for phase in ['before','after']])
before = json.loads((root/'before/geometry.json').read_text())['records']
after = json.loads((root/'after/geometry.json').read_text())['records']
rows = []
for b, a in zip(before[:4],after[:4]):
    rows.append({'viewport':b['viewport'],'heroHeight':[b['home']['height'],a['home']['height']], 'firstWidgetY':[b['firstWidgetY'],a['firstWidgetY']], 'scrollExtent':[b['scrollExtent'],a['scrollExtent']], 'launch':[a['instance']['width'],a['instance']['height']], 'overflow':a['documentOverflow'] or a['contentOverflow']})
(root/'geometry-comparison.json').write_text(json.dumps(rows,indent=2))
print('Created matched Home/state sheets and geometry comparison; originals retained.')
