"""Deterministic numeric/code-symbol aggregate; run after analyze.py."""
from collections import Counter
import json
from pathlib import Path
import statistics as st
from analyze import stats, union_ms

p=Path(__file__).parent/'evidence'
trials=json.loads((p/'trials.json').read_text())
b=[r for r in trials if r['cohort']=='baseline']
j={k:v for k,v in json.loads((p/'jfr.json').read_text()).items() if k.startswith('baseline-')}
phase={k:stats([v['partitionMs'][k] for v in j.values()]) for k in next(iter(j.values()))['partitionMs']}
keys=sorted(set().union(*(r['totals'] for r in b)))
totals={k:stats([r['totals'].get(k,{}).get('inclusiveNs',0)/1e6 for r in b]) for k in keys}
native={}
for e in sorted({s['event'] for r in b for s in r['nativeSpans']}):
    native[str(e)]=dict(inclusiveMs=stats([sum(s['ms'] for s in r['nativeSpans'] if s['event']==e) for r in b]),
        hashMs=stats([sum(s['hashUs']/1000 for s in r['nativeSpans'] if s['event']==e) for r in b]))
leaf=Counter(); owners=Counter(); native_leaf=Counter()
for v in j.values():
    leaf.update(v['allLeafMethods']); owners.update(v['leafOwnership']); native_leaf.update(dict(v['nativeMethodSamples']))
resource=lambda k: any(x in k for x in ['TextureUtil','NativeImage','SpriteContents','MipmapGenerator','ItemModelGenerator','ImprovedItemModelBuilder','FaceBakery','UnihexProvider','CodepointMap'])
pair={}
for metric in ['harnessPlayToMenuMs','playToMenuMs','agentOriginToMenuMs','playToCreationMs']:
    diffs=[next(x for x in trials if x['trial']==f'baseline-{n:02}')['timeline'][metric]
        -next(x for x in trials if x['trial']==f'control-{n:02}')['timeline'][metric] for n in range(1,6)]
    pair[metric]=dict(profiledMinusControlMs=diffs,statistics=stats(diffs))
events=sorted(set().union(*(v['durations'] for v in j.values())))
union={k:stats([v['unionDurationsMs'].get(k,0) for v in j.values()]) for k in ['mainRenderBlocking','gcPause']}
data=dict(baselineCount=len(b),partitionMs=phase,methodInclusiveMs=totals,nativeEvents=native,
    pairedOverhead=pair,executionSamples=sum(leaf.values()),leafOwnership=dict(owners),
    resourceTextureModelFontLeafSamples=sum(n for k,n in leaf.items() if resource(k)),
    topLeafMethods=leaf.most_common(30),topNativeMethods=native_leaf.most_common(15),
    jfrInclusiveDurationsMs={k:stats([v['durations'].get(k,{}).get('totalMs',0) for v in j.values()]) for k in events},
    unionDurationsMs=union,
    cpuLoad={k:stats([u[k] for v in j.values() for u in v['cpuLoad']]) for k in ['jvmUser','jvmSystem','machineTotal']},
    nativeStartupAuditCounts=[r['nativeStartupAuditCount'] for r in b],
    maximumJfrClockSpreadUs=max(v['clockOffsetSpreadUs'] for v in j.values()))
(p/'attribution.json').write_text(json.dumps(data,indent=2)+'\n')
print('attribution reduced',len(b),'baseline trials')
