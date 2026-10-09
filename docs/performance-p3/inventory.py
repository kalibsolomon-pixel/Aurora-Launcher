"""Export an allowlisted inventory; do not export user filenames or config text."""
import hashlib
import json
import os
from pathlib import Path
import sys
import zipfile

def sha(p):
    with p.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()

root=Path(sys.argv[1]).resolve(strict=True)
managed=root/'managed'
# This study has exactly one registered ready instance. Never choose an arbitrary directory.
registries=list((managed/'launcher').rglob('*instances*.json'))
instance=managed/'instances'/sys.argv[2]
assert instance.parent==managed/'instances' and instance.is_dir()
out=Path(__file__).parent/'evidence';out.mkdir(exist_ok=True)
options=dict(line.split(':',1) for line in (instance/'options.txt').read_text().splitlines() if ':' in line)
mods=[]
for p in sorted((instance/'mods').glob('*.jar')):
    with zipfile.ZipFile(p) as z:
        m=json.loads(z.read('fabric.mod.json'))
    mods.append(dict(id=m['id'],version=m['version'],sha256=sha(p),bytes=p.stat().st_size,entrypoints=m.get('entrypoints',{})))
packs=[]
selected=json.loads(options.get('resourcePacks','[]'))
for folder in ['resourcepacks','shaderpacks']:
    for n,p in enumerate(sorted((instance/folder).glob('*'))):
        if p.is_file(): packs.append(dict(alias=folder+'-'+str(n+1),sha256=sha(p),bytes=p.stat().st_size,
            selectedInOptions=('file/'+p.name) in selected if folder=='resourcepacks' else None))
baseline=json.loads((root/'baseline.json').read_text())
safe=['enableVsync','fullscreen','guiScale','maxFps','mipmapLevels','particles','renderDistance','simulationDistance','useNativeTransport']
data=dict(launcherHead=baseline['repositories']['launcher']['head'].strip(),clientHead=baseline['repositories']['client']['head'].strip(),
    artifacts=json.loads((root/'artifacts.json').read_text()),java='Microsoft OpenJDK 21.0.7+6 LTS',minecraft='1.21.11',fabric='0.19.5',aurora='3.0.0',
    mods=mods,packs=packs,resourceSelection=dict(vanillaSelected='vanilla' in selected,
        unavailableFileSelections=sum(x.startswith('file/') and not (instance/'resourcepacks'/x[5:]).exists() for x in selected),
        directoryPacksPresent=sum(p.is_dir() for p in (instance/'resourcepacks').iterdir()),
        directoryPacksSelected=sum(p.is_dir() and ('file/'+p.name) in selected for p in (instance/'resourcepacks').iterdir())),
    graphics={k:options[k] for k in safe if k in options},
    irisPresent=any(m['id']=='iris' for m in mods),irisConfigurationPresent=(instance/'config/iris.properties').exists(),
    memoryMiB=6144,additionalUserJvmArguments=[],installedLauncherSha256=next(x['sha256'] for x in baseline['files'] if x['kind']=='installed'))
(out/'inputs.json').write_text(json.dumps(data,indent=2)+'\n')
print('sanitized inputs',len(mods),'mods',len(packs),'pack objects')
