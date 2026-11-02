"""Compose original captures into labeled review sheets; keep originals intact."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import json

root = Path(__file__).resolve().parents[2] / 'docs/ui-2b'
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
native = root/'native-final'
if (native/'cyan-borealis-top-client.png').exists():
    sheet('native-review.png',[(label,'native-final/'+path) for label,path in [
        ('NATIVE | default 1120x790 | 125% OS scale','default-desktop-client.png'),
        ('NATIVE | maximized 2048x1152 | 125% OS scale','maximized-client.png'),
        ('NATIVE | cyan / Borealis | 125% OS scale','cyan-borealis-top-client.png'),
        ('NATIVE | widget editor | OS window capture','widget-editor-window.jpg')]])
def luminance(rgb):
    v=[c/255 for c in rgb]
    v=[c/12.92 if c<=.04045 else ((c+.055)/1.055)**2.4 for c in v]
    return sum(c*w for c,w in zip(v,[.2126,.7152,.0722]))
image=Image.open(root/'after/home-1280x800.png').convert('RGB')
foreground=(195,204,217)  # Existing Borealis --color-text-secondary.
samples=[]
for point in [(172,174),(292,174),(420,174)]:
    background=image.getpixel(point)
    a,b=luminance(foreground),luminance(background)
    samples.append({'point':point,'background':background,'foreground':foreground,'ratio':round((max(a,b)+.05)/(min(a,b)+.05),2)})
(root/'contrast-samples.json').write_text(json.dumps({'scope':'1280x800 reduced-motion browser capture; background pixels adjacent to hero metadata; existing secondary text color; sampled contrast, not a whole-app WCAG audit','samples':samples},indent=2))
