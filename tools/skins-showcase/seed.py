"""Disposable, invented skin fixtures; never contains credentials."""
import hashlib, json, os, struct, sys, uuid, zlib
from pathlib import Path

root = Path(sys.argv[1])
root.mkdir(parents=True, exist_ok=False)
(root / 'AURORA_DISPOSABLE_DIAGNOSTIC').write_text('Disposable skins showcase; no credentials.\n')
launcher = root / 'launcher'
launcher.mkdir()
(launcher / 'instances.json').write_text(json.dumps(dict(schemaVersion=6, instances=[])))
(launcher / 'accounts.json').write_text(json.dumps(dict(schemaVersion=1, selectedAccountId=None, accounts=[])))
(launcher / 'config.json').write_text(json.dumps({
    'schemaVersion': 7, 'selectedInstanceId': None,
    'appearance': {'background': 'borealis', 'auroraMotionSpeed': 40, 'theme': 'aurora-dark',
                   'accent': {'type': 'preset', 'id': 'violet'}},
    'homeWidgets': {'widgets': []},
    'discord': {key: False for key in ('enabled', 'instanceName', 'minecraftVersion', 'platform',
                                     'auroraActive', 'elapsedTime', 'world', 'server', 'serverAddress')},
}))
folder = root / 'launcher' / 'skin-presets'
folder.mkdir()
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
presets = []
for i, (name, color, model) in enumerate([
    ('Forest ranger', (53, 110, 78), 'classic'),
    ('Winter coat', (94, 124, 173), 'slim'),
    ('Sunset explorer', (175, 102, 69), 'classic'),
    ('Violet hoodie', (121, 89, 155), 'slim'),
]):
    pixels = bytearray(64 * 64 * 4)
    for y in range(64):
        for x in range(64):
            # Only base texture faces are opaque; leave outer layers transparent.
            if y < 16 and x < 32 or 16 <= y < 32 and x < 56 or 16 <= x < 48 and y >= 48:
                rgb = (210, 168, 136) if y < 16 else color
                if y < 10:
                    rgb = (57, 43, 39)
                if y >= 20 and x < 16 or y >= 52 and 16 <= x < 32:
                    rgb = (45, 52, 67)
                if y in (11, 12) and x in (10, 13):
                    rgb = (38, 42, 50)
                start = (y * 64 + x) * 4
                pixels[start:start+4] = bytes((*rgb, 255))
    raw = b''.join(b'\0' + pixels[y*256:(y+1)*256] for y in range(64))
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 64, 64, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')
    skin_id = str(uuid.UUID(int=i+1))
    (folder / (skin_id + '.png')).write_bytes(png)
    presets.append(dict(id=skin_id, name=name, model=model, importedAt=1800000000+i, sha256=hashlib.sha256(png).hexdigest(), favorite=i==0))
(folder / 'presets.json').write_text(json.dumps(dict(schemaVersion=2, presets=presets)))
if os.environ.get('AURORA_SHOWCASE_SKINS') == 'empty':
    (folder / 'presets.json').write_text(json.dumps(dict(schemaVersion=2, presets=[])))
