"""Verify only recorded baselines. No cleanup, restoration or mutation is performed."""
import hashlib,json,os,pathlib,subprocess
base=pathlib.Path(__file__).resolve().parent/'evidence'
def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''): h.update(block)
    return h.hexdigest()
results=[]
for label,root in [('launcher',pathlib.Path(r'C:\Dev\aurora-launcher')),('client',pathlib.Path(r'C:\Dev\Aurora-Client'))]:
    baseline=json.loads((base/(label+'-baseline.json')).read_text())
    differences=[]
    for item in baseline['files']:
        path=root/item['path']
        if item.get('missing'): continue
        if not path.is_file() or sha(path)!=item['sha256']: differences.append(item['path'])
    current=subprocess.check_output(['git','-C',str(root),'ls-files','-z']).decode().split('\0')
    removed=[x['path'] for x in baseline['files'] if x['kind']=='tracked' and x['path'] not in current]
    results.append({'repository':label,'checkedFiles':len(baseline['files']),
                    'changedOrMissing':differences,'trackedRemoved':removed})
root=pathlib.Path(os.environ['LOCALAPPDATA'])/'com.aurora.launcher'
protected=json.loads((base/'instance-protection.json').read_text())
differences=[x['path'] for x in protected if not (root/x['path']).is_file() or sha(root/x['path'])!=x['sha256']]
results.append({'repository':'managed-content','checkedFiles':len(protected),'changedOrMissing':differences})
(base/'protection-verification.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
print(json.dumps(results,indent=2))
assert not any(r['changedOrMissing'] or r.get('trackedRemoved') for r in results)
