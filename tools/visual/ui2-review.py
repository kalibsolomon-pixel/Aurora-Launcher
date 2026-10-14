"""Build labeled review sheets from the unmodified acceptance captures."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parents[2] / 'docs/ui-2'
font = ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf', 16)

def sheet(name, entries, columns=2, thumb=620):
    cells = []
    for label, path in entries:
        image = Image.open(root / path).convert('RGB')
        image.thumbnail((thumb, 470))
        cells.append((label, image))
    height = max(image.height for _, image in cells) + 44
    out = Image.new('RGB', (columns * (thumb + 16), ((len(cells)+columns-1)//columns) * height), '#101116')
    draw = ImageDraw.Draw(out)
    for i, (label, image) in enumerate(cells):
        x, y = (i % columns) * (thumb+16), (i // columns) * height
        draw.text((x+8, y+10), label, fill='#edf1fb', font=font)
        out.paste(image, (x+8, y+36))
    out.save(root / name)

sheet('home-comparison.png', [(f'{phase.upper()} | {size} | browser fixture', f'{phase}/home-{size}.png') for size in ['1440x900','1280x800','1024x768','720x520'] for phase in ['before','after']])
sheet('states-review.png', [(f'{state} | AFTER | browser fixture', f'after/{state}-1280x800.png') for state in ['sparse','dense','dense-bottom','long','blocked','running','missing-skin','empty','error','loading','oled-simple']])
sheet('accessibility-review.png', [(f'200% text | {width}px | browser fixture', f'states/text-200-{width}.png') for width in [1440,1024,900,720]] + [('OLED cyan | browser fixture','states/oled-cyan.png')])
for path in (root / 'native').glob('*-window.png'):
    # The desktop capture API returns JPEG data URLs. Preserve the captured
    # pixels/dimensions while making the saved file's format match its suffix.
    with Image.open(path) as image:
        if image.format != 'PNG':
            pixels = image.copy()
            pixels.save(path, 'PNG')
print('Created Home, state and accessibility review sheets; originals unchanged.')
