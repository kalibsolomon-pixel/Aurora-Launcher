from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parents[2] / 'docs' / 'ui-5'
font = ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf', 22)
small = ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf', 16)
for category in ['general', 'appearance', 'home', 'updates', 'discord']:
    rows = []
    for size in ['1440x900', '1280x800', '1024x768', '720x520']:
        pair = [Image.open(root / side / f'{category}-{size}.png').convert('RGB') for side in ['before', 'after']]
        for image in pair:
            image.thumbnail((640, 450))
        height = max(image.height for image in pair) + 42
        row = Image.new('RGB', (1304, height), '#101116')
        draw = ImageDraw.Draw(row)
        for index, image in enumerate(pair):
            x = index * 652
            draw.text((x + 12, 8), f'{size} — {"Before" if index == 0 else "After"}', font=small, fill='#dfe3ee')
            row.paste(image, (x + 12, 38))
        rows.append(row)
    sheet = Image.new('RGB', (1304, sum(row.height for row in rows) + 70), '#101116')
    draw = ImageDraw.Draw(sheet)
    draw.text((14, 10), f'UI-5 · {category.title()} · Matched browser fixtures', font=font, fill='white')
    draw.text((14, 40), 'Identical dimensions and preference values; synthetic data; no owner aesthetic approval claimed.', font=small, fill='#c3ccd9')
    y = 70
    for row in rows:
        sheet.paste(row, (0, y))
        y += row.height
    sheet.save(root / f'comparison-{category}.png')
print('Created five matched browser comparison sheets')
