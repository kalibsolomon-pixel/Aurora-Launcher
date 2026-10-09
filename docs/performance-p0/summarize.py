"""Summarize observed samples only; retain first calls apart from repeated calls."""
import collections, json, math, pathlib, statistics
base = pathlib.Path(__file__).resolve().parent / 'evidence'
groups = collections.defaultdict(list)
for path in base.glob('native-*.jsonl'):
    for line in path.read_text(encoding='utf-8-sig').splitlines():
        row = json.loads(line)
        if 'milliseconds' in row:
            groups[(path.stem,row['instance'],row['stage'])].append(row)
summary = []
for (run,instance,stage), rows in sorted(groups.items()):
    repeated = [r['milliseconds'] for r in rows if r['sample'] > 0]
    first = next((r['milliseconds'] for r in rows if r['sample']==0),None)
    ordered = sorted(repeated)
    summary.append({'run':run,'instance':instance,'stage':stage,'firstObservedMs':first,
        'nRepeated':len(repeated),'p50Ms':statistics.median(repeated),
        'p95Ms':ordered[math.ceil(.95*len(ordered))-1] if len(ordered)>=20 else None,
        'worstMs':max(repeated),'minMs':min(repeated),
        'sampleStdDevMs':statistics.stdev(repeated) if len(repeated)>1 else None,
        'sampleVarianceMs2':statistics.variance(repeated) if len(repeated)>1 else None,
        'facts':rows[-1]['facts']})
(base/'summary.json').write_text(json.dumps(summary,indent=2),encoding='utf-8')
print(json.dumps(summary,indent=2))
