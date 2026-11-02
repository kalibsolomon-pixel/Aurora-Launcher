import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

out = Path('docs/ui-3-1')
font = ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf', 18)
small = ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf', 14)
before = json.loads((out/'before/geometry.json').read_text())['measurements']
after = json.loads((out/'after/geometry.json').read_text())['measurements']
sizes = ['1440x900','1280x800','1024x768','720x520','706x512']
sheet = Image.new('RGB',(1640,2800),'#101116')
d = ImageDraw.Draw(sheet)
d.text((20,15),'UI-3.1 / Modpacks / matched browser fixtures (not native)',font=font,fill='#edf1fb')
y = 52
for size in sizes:
    d.text((20,y),f'{size}: BEFORE — c0c9342 grid',font=font,fill='#b7c2d9')
    d.text((840,y),f'{size}: AFTER — approved compact row system',font=font,fill='#b7c2d9')
    height = 0
    for x,side in [(20,'before'),(840,'after')]:
        im = Image.open(out/side/f'modpacks-{size}.png').convert('RGB')
        im.thumbnail((780,500))
        sheet.paste(im,(x,y+30));height=max(height,im.height)
    y += height+50
sheet.crop((0,0,1640,y)).save(out/'modpacks-before-after.png')

# Row crops stay at the original CSS pixel scale. No decorative reconstruction.
crops=[]
for surface in ['mods','modpacks']:
    g=after[f'{surface}-1440x900'];r=g['rows'][0]
    im=Image.open(out/'after'/f'{surface}-1440x900.png').convert('RGB')
    crops.append(im.crop((int(r['x']),int(r['y'])-1,int(r['right'])+1,min(im.height,int(r['y'])+275))))
width=sum(im.width for im in crops)+60
row_sheet=Image.new('RGB',(width,max(im.height for im in crops)+95),'#101116');d=ImageDraw.Draw(row_sheet)
d.text((20,12),'Approved Mods and corrected Modpacks — browser fixtures, original CSS pixel scale',font=font,fill='#edf1fb')
x=20
for label,im in zip(['Mods reference','Modpacks: same 44px artwork, padding, type and separators; distinct pack action'],crops):
    d.text((x,45),label,font=small,fill='#b7c2d9');row_sheet.paste(im,(x,75));x+=im.width+20
row_sheet.save(out/'mods-modpacks-rows.png')

summary=[]
for size in sizes:
    b=before[f'modpacks-{size}'];a=after[f'modpacks-{size}']
    summary.append({'viewport':size,'columnsBefore':len(b['columns'].split()),'columnsAfter':len(a['columns'].split()),'firstRowBefore':b['rows'][0]['height'],'firstRowAfter':a['rows'][0]['height'],'artworkBefore':b['imageSize'],'artworkAfter':a['imageSize'],'overflow':a['documentOverflow'] or a['contentOverflow']})
(out/'geometry-comparison.json').write_text(json.dumps(summary,indent=2))
print('PASS two comparison sheets; original captures retained; geometry summary written')
