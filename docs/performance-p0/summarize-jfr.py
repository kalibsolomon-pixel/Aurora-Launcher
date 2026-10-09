"""Export aggregate static class/method facts; never export thread names or dynamic strings."""
import collections,json,pathlib,subprocess
base=pathlib.Path(__file__).resolve().parent/'evidence'
raw=pathlib.Path((base/'small-root.local.txt').read_text())/'allowlisted-steady-state.jfr'
jfr=r'C:\Program Files\Eclipse Adoptium\jdk-25.0.4.101-hotspot\bin\jfr.exe'
assert raw.is_file(), 'bounded recording has not completed'
summary=subprocess.run([jfr,'summary',str(raw)],capture_output=True,text=True,timeout=25)
assert summary.returncode==0
# Reject unexpected custom event payloads rather than exporting their fields.
import re
assert not any(int(count) for name,count in re.findall(
    r'^\s*((?!jdk\.)[\w.]+)\s+(\d+)\s+\d+\s*$',summary.stdout,re.MULTILINE))
(base/'jfr-event-summary.txt').write_text(summary.stdout,encoding='utf-8')
proc=subprocess.run([jfr,'print','--json','--stack-depth','64','--events',
    'jdk.ExecutionSample,jdk.ObjectAllocationSample,jdk.GarbageCollection,jdk.GCPhasePause',str(raw)],
    capture_output=True,text=True,timeout=25)
assert proc.returncode==0
events=json.loads(proc.stdout)['recording']['events']
counts=collections.Counter(); leaves=collections.Counter(); aurora=collections.Counter(); allocations=collections.Counter()
allocation_counts=collections.Counter(); allocation_frames=collections.Counter(); gc_pauses=[]
for event in events:
    kind=event['type']; values=event['values']; counts[kind]+=1
    frames=(values.get('stackTrace') or {}).get('frames',[])
    methods=[]
    for frame in frames:
        method=frame.get('method',{}); cls=method.get('type',{}).get('name','')
        name=cls+'.'+method.get('name','')
        methods.append(name)
    if kind=='jdk.ExecutionSample':
        if methods: leaves[methods[0]]+=1
        for name in set(methods):
            if name.startswith('com/aurora/'): aurora[name]+=1
    if kind=='jdk.ObjectAllocationSample':
        allocations[values.get('objectClass',{}).get('name','unknown')]+=values.get('weight',0)
        allocation_counts[values.get('objectClass',{}).get('name','unknown')]+=1
        # Innermost Aurora frame, if any; counts are sampled events, not bytes.
        caller=next((m for m in methods if m.startswith('com/aurora/')),None)
        if caller: allocation_frames[caller]+=1
    if kind=='jdk.GCPhasePause': gc_pauses.append(values.get('duration'))
result={'durationSeconds':45,'workload':'existing supervised Minecraft child; UI state uncontrolled',
    'counts':dict(counts),'topExecutionLeaves':leaves.most_common(15),'auroraInclusiveSamples':aurora.most_common(15),
    'allocationSampleWeightsByClass':allocations.most_common(15),
    'allocationSampleCountsByClass':allocation_counts.most_common(15),
    'innermostAuroraAllocationSamples':allocation_frames.most_common(15),'gcPauseDurations':gc_pauses,
    'allocationWeightsAreEstimates':True,'rawBytes':raw.stat().st_size}
(base/'jfr-aggregate.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
