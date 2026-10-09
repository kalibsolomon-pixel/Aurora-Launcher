"""Reduce private recordings to code symbols and numeric startup evidence only.

No raw JFR JSON, paths, thread names, log text, or process arguments are exported.
CPU counts are samples, never elapsed-time fractions. Inclusive spans overlap.
"""
from collections import Counter, defaultdict
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
import statistics as st
import subprocess
import sys
import xml.etree.ElementTree as ET

HERE = Path(__file__).resolve().parent

def sha(path):
    with path.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

def epoch(value): return datetime.fromisoformat(value).timestamp()

def seconds(value):
    if isinstance(value, (int, float)): return value
    match = re.fullmatch(r'PT(?:(\d+)H)?(?:(\d+)M)?([\d.]+)?S?', value)
    assert match, 'unknown JFR duration'
    h, m, s = match.groups()
    return int(h or 0) * 3600 + int(m or 0) * 60 + float(s or 0)

def stats(values):
    return dict(n=len(values), median=st.median(values), min=min(values), max=max(values),
                mean=st.mean(values), sampleSd=st.stdev(values) if len(values)>1 else 0)

def union_ms(intervals):
    end = None; total = 0
    for a, b in sorted(intervals):
        if end is None or a > end: total += b-a
        else: total += max(0, b-end)
        end = max(end if end is not None else b, b)
    return total * 1000

def validate_markers(marker):
    records = marker['records']
    assert marker['menuReady'] is True
    assert sum(r['boundary']=='menu.usable' for r in records)==1
    assert not any(r['boundary'].startswith('error.') for r in records)
    assert sum(r['boundary']=='jna.isolation-confirmed' for r in records)==1
    return records

def symbols(root):
    # Only unambiguous Mojang names; overloaded ambiguous method names stay intermediary.
    classes, methods, current = {}, defaultdict(set), None
    for line in (root/'client-mappings.txt').read_text().splitlines():
        if line and not line.startswith((' ', '#')) and ' -> ' in line:
            public, obf = line.rstrip(':').split(' -> '); current = obf; classes[obf] = public
        elif current and '(' in line and ' -> ' in line:
            before, obf = line.strip().split(' -> ')
            match = re.search(r'([^\s:]+)\([^)]*\)', before)
            if match: methods[current, obf].add(match[1])
    import zipfile
    jar=next((root/'managed/instances').glob('*/game/libraries/net/fabricmc/intermediary/1.21.11/*.jar'))
    with zipfile.ZipFile(jar) as z: lines=z.read('mappings/mappings.tiny').decode().splitlines()
    cm, mm = {}, {}
    for line in lines:
        parts=line.split('\t')
        if parts[0]=='CLASS': cm[parts[2].replace('/','.')]=classes.get(parts[1], parts[2].replace('/','.'))
        elif parts[0]=='METHOD':
            names=methods.get((parts[1],parts[3]),set())
            if len(names)==1: mm[parts[1],parts[4]]=next(iter(names))
    owners={p.split('\t')[2].replace('/','.'):p.split('\t')[1] for p in lines if p.startswith('CLASS\t')}
    def symbol(frame):
        method=frame.get('method',{}); cl=method.get('type',{}).get('name','').replace('/','.')
        name=method.get('name','')
        if not re.fullmatch(r'[\w.$]+',cl) or not re.fullmatch(r'[\w$<>]+',name): return None
        return cm.get(cl,cl)+'.'+mm.get((owners.get(cl),name),name)
    return symbol

def owner(symbol):
    if symbol.startswith(('p3.',)): return 'research'
    # Injected methods retain the Minecraft declaring class in JFR. The scan
    # method is proven against the exact Sodium jar with javap.
    if symbol=='net.minecraft.client.renderer.texture.SpriteContents.scanSpriteContents' or '$sodium$' in symbol or '.sodium$' in symbol:
        return 'Sodium (injected)'
    if '$aurora$' in symbol or '.aurora$' in symbol: return 'Aurora'
    if symbol.startswith(('com.aurora.',)): return 'Aurora'
    if symbol.startswith(('org.spongepowered.asm.',)): return 'Mixin'
    if symbol.startswith(('net.fabricmc.',)): return 'Fabric'
    if symbol.startswith(('net.minecraft.','com.mojang.')): return 'Minecraft/Mojang'
    if symbol.startswith(('java.','jdk.','sun.','com.sun.')): return 'JVM/JDK'
    if symbol.startswith(('org.lwjgl.','org.joml.')): return 'graphics/native support'
    return 'third-party mod/library'

def reduce_jfr(root, trial, marker, symbol):
    file=trial/'startup.jfr'
    summary=subprocess.check_output(['jfr','summary',str(file)],text=True)
    counts={n:int(c) for n,c in re.findall(r'^\s*([\w.]+)\s+(\d+)\s+\d+\s*$',summary,re.M)}
    config=ET.parse(root/'startup.jfc')
    allowed={e.attrib['name'] for e in config.getroot().findall('event') if e.find("setting[@name='enabled']").text=='true'}
    actual={n for n,c in counts.items() if c}
    # JFR's structural metadata/checkpoints are intrinsic, not configurable events.
    assert actual <= allowed | {'jdk.Checkpoint','jdk.Metadata'}, 'unexpected enabled event: STOP export'
    events=json.loads(subprocess.check_output(['jfr','print','--json','--events',','.join(sorted(allowed)),str(file)],text=True))['recording']['events']
    phases=[e['values'] for e in events if e['type']=='aurora.p3.Phase']
    menu=[v for v in phases if v['boundary']=='menu.usable']; assert len(menu)==1
    offsets=[epoch(v['startTime'])-v['elapsedNs']/1e9 for v in phases]
    origin=st.median(offsets); end=epoch(menu[0]['startTime'])
    assert abs(origin-marker['originUnixMs']/1000)<.025, 'clock bridge inconsistent'
    leaf=Counter(); inclusive=Counter(); leaves=Counter(); stacks=Counter(); mainleaves=Counter()
    durations=defaultdict(list); blocking=Counter(); files=Counter(); compilation=Counter(); allocations=Counter()
    native_count=0; jna_inside=False; startup_counts=Counter(); native_leaves=Counter()
    phase_leaves=defaultdict(Counter); cpu=[]; intervals=defaultdict(list)
    milestones={(r['boundary'],r['edge']):r['ns']/1e9 for r in marker['records']}
    partitions=[('beforeKnot',0,milestones['fabric.knot-init',1]),
        ('Knot',milestones['fabric.knot-init',1],milestones['fabric.knot-init',2]),
        ('beforeMinecraftMain',milestones['fabric.knot-init',2],milestones['minecraft.main',1]),
        ('MinecraftMainBeforeConstructor',milestones['minecraft.main',1],milestones['minecraft.constructor',1]),
        ('MinecraftConstructor',milestones['minecraft.constructor',1],milestones['minecraft.constructor',2]),
        ('afterConstructorToMenu',milestones['minecraft.constructor',2],milestones['menu.usable',0])]
    for e in events:
        kind=e['type']; v=e['values']; when=epoch(v['startTime'])
        if kind=='jdk.NativeLibrary':
            native_count+=1
            raw=v.get('name','').replace('\\','/').lower()
            if 'jnidispatch' in raw or '/jna/' in raw: jna_inside |= '/trials/'+trial.name+'/jna/' in raw
        if not origin <= when <= end: continue
        startup_counts[kind]+=1
        frames=(v.get('stackTrace') or {}).get('frames',[])
        chain=[s for f in frames if (s:=symbol(f))]
        thread=v.get('sampledThread') or v.get('eventThread') or {}
        main=thread.get('javaName') in ['main','Render thread']
        if kind=='jdk.ExecutionSample' and chain:
            leaf[owner(chain[0])]+=1; leaves[chain[0]]+=1; stacks[' <- '.join(chain[:8])]+=1
            for own in sorted(set(map(owner,chain))): inclusive[own]+=1
            if main: mainleaves[chain[0]]+=1
            for label,a,b in partitions:
                if a <= when-origin < b: phase_leaves[label][chain[0]]+=1
        if kind=='jdk.NativeMethodSample' and chain: native_leaves[chain[0]]+=1
        if kind=='jdk.CPULoad': cpu.append({k:v[k] for k in ['jvmUser','jvmSystem','machineTotal']})
        duration=min(seconds(v.get('duration','PT0S')),max(0,end-when))*1000
        if kind in ['jdk.GarbageCollection','jdk.GCPhasePause','jdk.ClassLoad','jdk.Compilation','jdk.CompilerPhase','jdk.FileRead','jdk.FileWrite']:
            durations[kind].append(duration)
        if main and kind in ['jdk.ThreadPark','jdk.ThreadSleep','jdk.JavaMonitorEnter','jdk.JavaMonitorWait']:
            blocking[(kind, ' <- '.join(chain[:6]))]+=duration
            intervals['mainRenderBlocking'].append((when,when+duration/1000))
        if kind=='jdk.GCPhasePause': intervals['gcPause'].append((when,when+duration/1000))
        if kind in ['jdk.FileRead','jdk.FileWrite']:
            files[kind]+=v.get('bytesRead',v.get('bytesWritten',0))
        if kind=='jdk.Compilation' and v.get('method'):
            s=symbol({'method':v['method']})
            if s: compilation[s]+=duration
        if kind=='jdk.ObjectAllocationSample':
            c=(v.get('objectClass') or {}).get('name','').replace('/','.')
            if re.fullmatch(r'[\w.$\[;/]+',c): allocations[c]+=v.get('weight',0)
    return dict(sha256=sha(file), bytes=file.stat().st_size, eventCounts=counts,
        unexpectedEnabledEvents=0, clockOffsetSpreadUs=(max(offsets)-min(offsets))*1e6,
        originUnixSeconds=origin, menuUnixSeconds=end,
        nativeLibrariesObserved=native_count, isolatedJnaLibraryObserved=jna_inside,
        preMenuEventCounts=dict(startup_counts),
        executionSamples=sum(leaf.values()), leafOwnership=dict(leaf), inclusiveOwnership=dict(inclusive),
        ownershipConvention='declaring-class prefix except proven Sodium scan and explicit mod injector labels; remaining injected origins may be unresolved',
        allLeafMethods=dict(leaves), nativeMethodSamples=native_leaves.most_common(25),
        partitionMs={label:(b-a)*1000 for label,a,b in partitions},
        phaseLeafMethods={k:v.most_common(12) for k,v in phase_leaves.items()},
        cpuLoad=cpu, unionDurationsMs={k:union_ms(v) for k,v in intervals.items()},
        leafMethods=leaves.most_common(35), mainRenderLeafMethods=mainleaves.most_common(25),
        stacks=stacks.most_common(20),
        durations={k:dict(count=len(v), totalMs=sum(v), maxMs=max(v)) for k,v in durations.items()},
        mainRenderBlocking=[dict(event=k[0], stack=k[1], inclusiveMs=v) for k,v in blocking.most_common(20)],
        slowIoBytes=dict(files), compilationMethodsMs=compilation.most_common(20), allocationWeights=allocations.most_common(20))

def analyze(root):
    symbol=symbols(root); samples=[]; excluded=[]; jfrs={}; private=[]
    for trial in sorted((root/'trials').iterdir()):
        if not (trial/'start-private.json').exists(): continue
        if not (trial/'markers-menu.json').exists():
            excluded.append(dict(trial=trial.name, reason='did not reach deterministic usable menu',
                exitCode=json.loads((trial/'exit.json').read_text())['exitCode'] if (trial/'exit.json').exists() else None))
            continue
        marker=json.loads((trial/'markers-menu.json').read_text()); records=validate_markers(marker)
        exitcode=json.loads((trial/'exit.json').read_text())['exitCode']; assert exitcode==0
        trace=json.loads((trial/'native-trace.json').read_text()); assert trace['dropped']==0
        rows=trace['records']; assert all(len(r)==11 and all(type(v) is int and v>=0 for v in r.values()) for r in rows)
        clock=json.loads((trial/'native-clock.json').read_text())
        native_origin=(clock['unixUs']-(clock['nativeUsBefore']+clock['nativeUsAfter'])/2)/1e6
        play=next(r for r in rows if r['event']==108)
        start={r['id']:r for r in rows if r['edge']==1}; spans=[]
        for r in rows:
            if r['edge']==2:
                a=start[r['id']]; spans.append(dict(event=r['event'],id=r['id'],parent=a['parent'],beginUs=a['us'],endUs=r['us'],ms=(r['us']-a['us'])/1000,hashUs=r['hash_us'],hashCalls=r['hash_calls']))
        prep=next(s for s in spans if s['event']==12)
        descendants={prep['id']}
        for s in sorted(spans,key=lambda s:s['beginUs']):
            if s['parent'] in descendants: descendants.add(s['id'])
        relevant=[s for s in spans if s['id'] in descendants]
        jfr=None
        if (trial/'startup.jfr').exists():
            jfr=reduce_jfr(root,trial,marker,symbol)
            origin=jfr.pop('originUnixSeconds'); menu=jfr.pop('menuUnixSeconds'); jfrs[trial.name]=jfr
            private.append(dict(trial=trial.name,path=str(trial/'startup.jfr'),sha256=jfr['sha256']))
        else:
            origin=marker['originUnixMs']/1000; menu=origin+next(r['ns'] for r in records if r['boundary']=='menu.usable')/1e9
        spawn=json.loads((trial/'spawn-private.json').read_text())
        creation=json.loads((trial/'os-private.json').read_text())['creationUnixUs']/1e6
        begin=native_origin+play['us']/1e6
        fixture={r['boundary']:r['ns']/1e6 for r in records if r['edge']==0}
        request_ms=json.loads((trial/'play-request.json').read_text())['requestUnixMs']
        timeline=dict(harnessPlayToMenuMs=menu*1000-request_ms,
            playToMenuMs=(menu-begin)*1000, playToCreationMs=(creation-begin)*1000,
            playToSpawnConfirmedMs=(spawn['confirmedUnixUs']/1e6-begin)*1000,
            creationToAgentOriginMs=(origin-creation)*1000,
            agentOriginToMenuMs=(menu-origin)*1000,
            nativePlayPreparationMs=prep['ms'], harnessInputToNativeReceiptMs=begin*1000-request_ms,
            nativeClockBracketUs=clock['nativeUsAfter']-clock['nativeUsBefore'],
            javaWallDriftMs=marker['savedUnixMs']-marker['originUnixMs']-marker['savedElapsedNs']/1e6)
        cohort='control' if trial.name.startswith('control') else ('pilot' if trial.name.startswith('pilot') else 'baseline')
        samples.append(dict(trial=trial.name,cohort=cohort,exitCode=exitcode,timeline=timeline,
            nativeSpans=relevant, nativeStartupAuditCount=sum(s['event']==39 for s in spans),
            markerRecords=records, totals=marker['totals'],
            jnaConfirmed=True, usableMenuConfirmed=True, jfrSha256=jfr['sha256'] if jfr else None))
    groups={}
    for cohort in ['pilot','baseline','control']:
        rows=[r for r in samples if r['cohort']==cohort]
        if rows: groups[cohort]={k:stats([r['timeline'][k] for r in rows]) for k in rows[0]['timeline']}
    out=HERE/'evidence';out.mkdir(exist_ok=True)
    for name,data in [('trials.json',samples),('statistics.json',groups),('jfr.json',jfrs),('exclusions.json',excluded)]:
        (out/name).write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')
    # Include failed pilots and fixtures, not just recordings used for inference.
    private=[dict(path=str(f),sha256=sha(f),bytes=f.stat().st_size,
        role='primary' if f.parent.name.startswith('baseline-') else
        'pilot' if f.parent.name.startswith('pilot-') else 'instrumentation fixture/tool')
        for f in sorted(root.rglob('*.jfr')) if f.is_file()]
    (root/'raw-index.json').write_text(json.dumps(private,indent=2)+'\n')
    print(json.dumps(dict(successful=len(samples),excluded=excluded,cohorts={k:v['playToMenuMs'] for k,v in groups.items()}),indent=2))

if __name__=='__main__': analyze(Path(sys.argv[1]).resolve(strict=True))
