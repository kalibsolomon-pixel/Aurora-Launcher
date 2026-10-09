"""Analysis-only official mappings acquisition; never writes a product cache."""
import hashlib
import json
from pathlib import Path
import re
import sys
import urllib.parse
import urllib.request

root=Path(sys.argv[1]).resolve(strict=True)
assert root.name.startswith('aurora-p0-2-p3-') and (root/'baseline.json').is_file()

def fetch(url, limit):
    parsed=urllib.parse.urlsplit(url)
    assert parsed.scheme=='https' and parsed.hostname in ['piston-meta.mojang.com','piston-data.mojang.com']
    with urllib.request.urlopen(url,timeout=30) as response:
        final=urllib.parse.urlsplit(response.geturl())
        assert final.scheme=='https' and final.hostname in ['piston-meta.mojang.com','piston-data.mojang.com']
        data=response.read(limit+1)
    assert len(data)<=limit
    return data

def verified(url, expected, limit):
    assert re.fullmatch('[0-9a-f]{40}',expected)
    data=fetch(url,limit)
    assert hashlib.sha1(data).hexdigest()==expected, 'official SHA-1 mismatch'
    return data

manifest=json.loads(fetch('https://piston-meta.mojang.com/mc/game/version_manifest_v2.json',5_000_000))
versions=[v for v in manifest['versions'] if v['id']=='1.21.11']
assert len(versions)==1
v=versions[0]
version=json.loads(verified(v['url'],v['sha1'],5_000_000))
assert version['id']=='1.21.11'
m=version['downloads']['client_mappings']
data=verified(m['url'],m['sha1'],80_000_000)
target=root/'client-mappings.txt'
if target.exists(): assert target.read_bytes()==data, 'existing analysis input differs; preserve it'
else:
    with target.open('xb') as f: f.write(data)
receipt=dict(minecraft='1.21.11',versionDocumentExpectedSha1=v['sha1'],mappingExpectedSha1=m['sha1'],
    mappingSha256=hashlib.sha256(data).hexdigest(),source=m['url'])
path=root/'mapping-provenance.json'
if path.exists(): assert json.loads(path.read_text())==receipt
else:
    with path.open('x') as f: json.dump(receipt,f,indent=2)
print('official mapping SHA-1 verified; private analysis input ready')
