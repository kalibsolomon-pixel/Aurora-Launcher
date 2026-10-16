"""P3-A offline quality gates. No capture, launch, network or configuration writes."""
import argparse
import csv
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import statistics
from datetime import datetime
from gc_metrics import parse as gc_metrics

P1_PATH = Path(__file__).resolve().parents[2] / 'docs/performance-p1/jvm-benchmark/analyze.py'
spec = importlib.util.spec_from_file_location('aurora_p1_analysis', P1_PATH)
p1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p1)
MAX_BYTES = 256 * 1024 * 1024
MAX_ROWS = 2_000_000
ID = re.compile(r'[A-Za-z0-9_-]{1,80}')
HASH = re.compile(r'[0-9a-f]{64}')
TOP = {'schemaVersion', 'runId', 'workloadId', 'configurationId', 'blockId', 'sampleKind',
       'status', 'failureReason', 'startUtc', 'endUtc', 'warmupSeconds', 'measurementSeconds',
       'window', 'process', 'runtime', 'minecraft', 'mods', 'graphics', 'scene', 'hardware',
       'environment', 'capture', 'clocks', 'jvmArguments', 'artifacts', 'startupSeconds',
       'qualityPolicy', 'dataQualityWarnings'}


class Invalid(ValueError):
    pass


def require(condition, reason):
    if not condition:
        raise Invalid(reason)


def number(value, positive=False):
    require(not isinstance(value, bool), 'boolean measurement')
    try:
        result = float(value)
    except (ValueError, TypeError, OverflowError) as exc:
        raise Invalid('invalid number') from exc
    require(math.isfinite(result) and result >= 0 and (not positive or result > 0), 'impossible measurement')
    return result


def integer(value, positive=False):
    require(type(value) is int and value >= (1 if positive else 0), 'invalid integer')
    return value


def text(value):
    require(isinstance(value, str) and 0 < len(value) <= 4096 and not any(ord(c) < 32 for c in value), 'invalid text')
    return value


def sha256(path):
    require(path.stat().st_size <= MAX_BYTES, 'artifact exceeds size bound')
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def safe_path(root, name):
    require(isinstance(name, str) and re.fullmatch(r'[A-Za-z0-9_.\-/]+', name) is not None,
            'artifact path must be a portable relative path')
    require(all(p not in ('', '.', '..') for p in name.split('/')), 'unsafe artifact path')
    result = root / name
    require(result.resolve().is_relative_to(root.resolve()), 'artifact escapes run root')
    return result


def read_rows(path, columns):
    require(path.stat().st_size <= MAX_BYTES, 'capture exceeds size bound')
    with path.open(encoding='utf-8-sig', newline='') as stream:
        reader = csv.DictReader(stream)
        require(reader.fieldnames is not None and len(set(reader.fieldnames)) == len(reader.fieldnames)
                and set(columns) <= set(reader.fieldnames), 'missing or duplicate capture columns')
        rows = []
        for row in reader:
            require(len(rows) < MAX_ROWS, 'capture row bound exceeded')
            require(None not in row and all(v is not None and len(v) <= 16384 for v in row.values()), 'malformed capture row')
            rows.append(row)
    require(rows, 'empty capture')
    return rows


def strictly_increasing(values):
    require(all(b > a for a, b in zip(values, values[1:])), 'duplicate or non-monotonic timestamps')


def frame_metrics(times, start, duration, tolerance):
    strictly_increasing(times)
    require(len(times) >= 2, 'insufficient display anchors')
    stop = start + duration
    intervals = [(b - a) * 1000 for a, b in zip(times, times[1:]) if a >= start and b <= stop]
    require(intervals, 'no complete frame intervals in window')
    inside = [t for t in times if start <= t <= stop]
    coverage = inside[0] - start <= tolerance and stop - inside[-1] <= tolerance
    stats = p1.distribution(intervals)
    stats.update(presentation_rate_hz=1000 / statistics.mean(intervals),
                 stutter_frames_over_50ms=sum(v > 50 for v in intervals),
                 stutter_events_over_50ms=sum(v > 50 and (i == 0 or intervals[i-1] <= 50) for i, v in enumerate(intervals)),
                 stutter_frames_per_minute=sum(v > 50 for v in intervals) * 60 / duration,
                 long_gaps_over_1000ms=sum(v > 1000 for v in intervals),
                 boundary_coverage=coverage, discarded_boundary_intervals=len(times)-1-len(intervals))
    return stats


def presentmon(path, capture, process, start, duration, tolerance):
    """Pinned 2.3.1 default CSV + --qpc_time. CPU start alone is NOT display time."""
    require(capture['tool'] == 'PresentMon' and capture['version'] == '2.3.1'
            and capture['format'] == 'presentmon-2.3.1-default-qpc'
            and capture['frameSemantic'] == 'etw-display-estimate', 'unsupported capture format or frame semantic')
    frequency = integer(capture['qpcFrequencyHz'], True)
    origin = integer(capture['originQpc'])
    rows = read_rows(path, ['ProcessID', 'SwapChainAddress', 'CPUStartQPC', 'DisplayLatency', 'DisplayedTime'])
    times, cpu_times, dropped = [], [], 0
    for row in rows:
        require(row['ProcessID'] == str(process['pid']), 'capture process mismatch')
        require(row['SwapChainAddress'] == capture['swapChain'], 'capture swap-chain mismatch')
        require(re.fullmatch(r'\d+', row['CPUStartQPC']) is not None, 'QPC must be integer ticks')
        ticks = int(row['CPUStartQPC'])
        cpu_times.append(ticks)
        if row['DisplayedTime'] == 'NA':
            dropped += 1
            continue
        number(row['DisplayedTime'], True)
        latency = number(row['DisplayLatency'])
        # Subtract integers before conversion to preserve large-QPC precision.
        times.append((ticks - origin) / frequency + latency / 1000)
    strictly_increasing(cpu_times)
    result = frame_metrics(times, start, duration, tolerance)
    result.update(raw_rows=len(rows), undisplayed_rows=dropped,
                  semantic='etw-display-estimate', display_time_estimate_warning=True)
    return result, times


def normalized_frames(path, capture, process, start, duration, tolerance):
    require(capture['format'] == 'normalized-timestamps-v1' and capture['units'] in ('s', 'ms', 'ns'), 'unexpected frame units or format')
    require(capture['frameSemantic'] in ('render-internal', 'synthetic-presentation'), 'normalized samples cannot attest displayed presentation')
    scale = {'s': 1, 'ms': .001, 'ns': 1e-9}[capture['units']]
    rows = read_rows(path, ['timestamp', 'pid'])
    require(all(r['pid'] == str(process['pid']) for r in rows), 'capture process mismatch')
    times = [number(r['timestamp']) * scale for r in rows]
    return frame_metrics(times, start, duration, tolerance), times


def clock_mapping(mapping):
    require(set(mapping) == {'source0', 'source1', 'run0', 'run1', 'uncertaintyMs', 'maxDriftPpm'}, 'clock mapping fields missing')
    a, b, x, y = [number(mapping[k]) for k in ('source0', 'source1', 'run0', 'run1')]
    require(b > a and y > x, 'clock anchors must increase')
    slope = (y-x)/(b-a)
    drift = abs(slope-1)*1_000_000
    require(drift <= number(mapping['maxDriftPpm']), 'clock drift exceeds declared limit')
    uncertainty = number(mapping['uncertaintyMs'])
    return lambda t: x+(t-a)*slope, drift, uncertainty


def process_metrics(path, identity, start, duration, tolerance):
    rows = read_rows(path, ['monotonic_ns', 'pid', 'creation_id', 'cpu_time_ns', 'rss_bytes',
                            'peak_rss_bytes', 'system_available_bytes', 'system_total_bytes',
                            'system_busy_ns', 'system_total_ns'])
    require(all(r['pid'] == str(identity['pid']) and r['creation_id'] == identity['creationId'] for r in rows), 'process identity changed')
    for row in rows:
        require(all(re.fullmatch(r'\d+', row[c]) is not None for c in rows[0] if c not in ('creation_id',)), 'process counters must be integer quantities')
        require(int(row['rss_bytes']) <= int(row['peak_rss_bytes']), 'RSS exceeds peak RSS')
        require(int(row['system_available_bytes']) <= int(row['system_total_bytes']), 'available memory exceeds total')
    origin = integer(identity['originMonotonicNs'])
    times = [(int(r['monotonic_ns'])-origin)/1e9 for r in rows]
    strictly_increasing(times)
    require(len(rows) >= 2, 'insufficient process samples')
    selected = [(a,b,ta,tb) for a,b,ta,tb in zip(rows,rows[1:],times,times[1:]) if tb > start and ta < start+duration]
    require(selected, 'no process intervals in window')
    cpu, background, intervals, busy_percent, weights = [], [], [], [], []
    for a,b,ta,tb in selected:
        elapsed = tb-ta
        dcpu = int(b['cpu_time_ns'])-int(a['cpu_time_ns'])
        dbusy = int(b['system_busy_ns'])-int(a['system_busy_ns'])
        dtotal = int(b['system_total_ns'])-int(a['system_total_ns'])
        require(dcpu >= 0 and 0 <= dbusy <= dtotal and dtotal > 0, 'process/system counter regressed')
        require(dcpu <= elapsed*1e9*identity['logicalProcessors']*1.1, 'impossible process CPU time')
        cpu.append(dcpu/(elapsed*1e9)*100)
        background.append(max(0, (dbusy-dcpu)/(elapsed*1e9)*100))
        busy_percent.append(dbusy/dtotal*100)
        intervals.append(elapsed)
        weights.append(min(tb,start+duration)-max(ta,start))
    weight = sum(weights)
    weighted = lambda values: sum(v*t for v,t in zip(values,weights))/weight
    memory_rows = [r for r,t in zip(rows,times) if start <= t <= start+duration]
    return {'cpu_percent_one_core_mean': weighted(cpu), 'cpu_percent_one_core_peak_sampled': max(cpu),
            'system_cpu_percent_machine_mean': weighted(busy_percent),
            'background_cpu_percent_one_core_estimate_mean': weighted(background),
            'rss_bytes_peak_sampled': max(int(r['rss_bytes']) for r in memory_rows),
            'peak_rss_bytes_os': max(int(r['peak_rss_bytes']) for r in memory_rows),
            'system_available_bytes_min': min(int(r['system_available_bytes']) for r in memory_rows),
            'sample_interval_seconds_max': max(intervals), 'covered_seconds': weight,
            'boundary_coverage': selected[0][2]-start <= tolerance and start+duration-selected[-1][3] <= tolerance,
            'boundary_method':'time-weighted counter intervals clipped at window edges; within-interval rate assumed uniform',
            'heap_used_bytes': None, 'heap_committed_bytes': None, 'paging_bytes_per_second': None,
            'gc_cpu_percent': None}


def validate_manifest(m):
    require(isinstance(m, dict) and set(m) <= TOP, 'unknown manifest fields')
    require(TOP - {'failureReason', 'startupSeconds'} <= set(m), 'missing run metadata')
    require(type(m['schemaVersion']) is int and m['schemaVersion'] == 1, 'unsupported schema')
    for key in ('runId','workloadId','configurationId','blockId'):
        require(isinstance(m[key], str) and ID.fullmatch(m[key]), 'invalid run/workload identifier')
    require(m['sampleKind'] in ('synthetic','measured'), 'invalid sample kind')
    require(m['status'] in ('complete','failed','interrupted'), 'invalid run status')
    for key in ('runtime','minecraft','graphics','scene','hardware','environment','capture','clocks','process','qualityPolicy','window'):
        require(isinstance(m[key],dict) and m[key], 'missing condition metadata')
    required = {
        'runtime': ('vendor','version','build','architecture','manifestSha1','executableSha256'),
        'minecraft': ('version','loader','loaderVersion','fabricApi'),
        'graphics': ('width','height','refreshHz','windowMode','renderDistance','simulationDistance','optionsSha256'),
        'scene': ('worldSha256','routeSha256','settingsSha256'),
        'hardware': ('cpu','gpu','gpuDriver','ramBytes','os','osBuild'),
        'environment': ('power','acConnected','thermals','background','display','quietPreflightSha256'),
        'capture': ('tool','version','format','frameSemantic','settingsSha256','toolSha256'),
        'process': ('pid','creationId','originMonotonicNs','logicalProcessors'),
        'window': ('startSeconds',),
        'qualityPolicy': ('minimumWarmupSeconds','minimumFrames','boundaryToleranceSeconds','maxProcessIntervalSeconds')}
    for obj, fields in required.items():
        require(set(fields) <= set(m[obj]), 'incomplete '+obj+' metadata')
    integer(m['process']['pid'],True); integer(m['process']['logicalProcessors'],True)
    integer(m['process']['originMonotonicNs']); text(m['process']['creationId'])
    integer(m['hardware']['ramBytes'],True)
    for key in ('cpu','gpu','gpuDriver','os','osBuild'):text(m['hardware'][key])
    for key in ('width','height','renderDistance','simulationDistance'):integer(m['graphics'][key],True)
    number(m['graphics']['refreshHz'],True);text(m['graphics']['windowMode'])
    require(m['environment']['acConnected'] is None or type(m['environment']['acConnected']) is bool, 'invalid AC state')
    for key in ('vendor','version','build','architecture'):
        text(m['runtime'][key])
    require(re.fullmatch('[0-9a-f]{40}', m['runtime']['manifestSha1']) is not None, 'missing runtime manifest SHA-1')
    hash_values = [m['runtime']['executableSha256'],m['graphics']['optionsSha256'],m['capture']['settingsSha256'],m['capture']['toolSha256'],m['environment']['quietPreflightSha256']]
    hash_values += [m['scene'][k] for k in ('worldSha256','routeSha256','settingsSha256')]
    require(all(isinstance(h,str) and HASH.fullmatch(h) for h in hash_values), 'missing lock hash')
    require(isinstance(m['mods'],list), 'missing mod inventory')
    identities = []
    for mod in m['mods']:
        require(isinstance(mod,dict) and set(mod)=={'id','version','algorithm','digest'}, 'invalid mod lock')
        identities.append(text(mod['id'])); text(mod['version'])
        require(mod['algorithm'] in ('sha1','sha256','sha512'), 'unknown mod digest algorithm')
        size={'sha1':40,'sha256':64,'sha512':128}[mod['algorithm']]
        require(isinstance(mod['digest'],str) and re.fullmatch('[0-9a-f]{'+str(size)+'}',mod['digest']), 'missing mod artifact digest')
    require(len(identities)==len(set(identities)), 'duplicate mod identity')
    require(isinstance(m['jvmArguments'],list) and m['jvmArguments'], 'missing sanitized JVM configuration')
    # Export source-reviewed JVM-only settings, never a full live process command line.
    for arg in m['jvmArguments']:
        text(arg)
        require(not any(s in arg.lower() for s in ('token','password','username','clientid','uuid','http://','https://'))
                and not re.search(r'[A-Za-z]:[\\/]|[\\/](?:Users|home)[\\/]',arg), 'sensitive JVM argument or private path')
    for key in ('startUtc','endUtc'):
        stamp=datetime.fromisoformat(text(m[key]).replace('Z','+00:00'))
        require(stamp.tzinfo is not None, 'UTC timestamps need timezone')
    require(datetime.fromisoformat(m['endUtc'].replace('Z','+00:00'))>datetime.fromisoformat(m['startUtc'].replace('Z','+00:00')), 'run end precedes start')
    number(m['warmupSeconds']); number(m['measurementSeconds'],True)
    number(m['window']['startSeconds'])
    for key in ('minimumWarmupSeconds','minimumFrames','boundaryToleranceSeconds','maxProcessIntervalSeconds'):
        number(m['qualityPolicy'][key],True)
    require(m['qualityPolicy']['boundaryToleranceSeconds'] <= .25, 'coverage tolerance exceeds 250 ms')
    require(isinstance(m['dataQualityWarnings'],list) and all(isinstance(v,str) and re.fullmatch('[a-z0-9_-]{1,80}',v) for v in m['dataQualityWarnings']), 'quality warnings must be nonsensitive machine codes')
    if m['sampleKind']=='measured':
        require(m['warmupSeconds'] >= 300 and m['measurementSeconds'] >= 600, 'measured gameplay requires reviewed 300/600-second protocol')
        require(m['qualityPolicy']['minimumFrames'] >= 1000 and m['qualityPolicy']['minimumWarmupSeconds'] >= 300, 'measured quality policy too weak')
        require(m['capture']['frameSemantic'] != 'synthetic-presentation', 'synthetic frames cannot be gameplay evidence')
        require(all(m['runtime'][k].lower() not in ('unknown','synthetic','fixture','pending') for k in ('vendor','version','build')),
                'measured runtime identity unresolved')
        require(all(h != '0'*len(h) for h in hash_values+[m['runtime']['manifestSha1']]), 'measured lock has placeholder digest')
    require(isinstance(m['artifacts'],dict) and set(m['artifacts'])=={'frames','gc','process'}, 'missing artifact inventory')
    for artifact in m['artifacts'].values():
        require(isinstance(artifact,dict) and set(artifact)=={'path','sha256'}, 'invalid artifact metadata')
        require(isinstance(artifact['sha256'],str) and HASH.fullmatch(artifact['sha256']), 'missing artifact hash')
    # Unknown optional nested fields cannot hide ordinary credential/account data.
    def privacy(value):
        if isinstance(value,dict):
            for key,child in value.items():
                require(not re.search(r'token|password|credential|account|username|email|serveraddress',key,re.I), 'private metadata field')
                privacy(child)
        elif isinstance(value,list):
            for child in value: privacy(child)
        elif isinstance(value,float):
            require(math.isfinite(value),'nonfinite metadata')
        elif isinstance(value,str):
            text(value)
    privacy(m)


def analyze(path):
    warnings, reasons, metrics = [], [], {}
    result={'runId':None,'classification':'INVALID','reasons':reasons,'warnings':warnings,
            'evidenceScope':'offline data-quality checks; not Minecraft capture validation','metrics':metrics}
    incomplete=False
    try:
        require(path.stat().st_size <= 1024*1024, 'manifest exceeds bound')
        m=json.loads(path.read_text(encoding='utf-8-sig'))
        validate_manifest(m)
        result.update(runId=m['runId'],sampleKind=m['sampleKind'],configurationId=m['configurationId'],
                      blockId=m['blockId'],manifestSha256=sha256(path))
        warnings.extend(m['dataQualityWarnings'])
        if m['status']!='complete':
            text(m.get('failureReason'))
            incomplete=True; reasons.append('run failed or interrupted; retained in denominator')
        sources={}
        for role,entry in m['artifacts'].items():
            p=safe_path(path.parent,entry['path'])
            if not p.is_file():
                incomplete=True; reasons.append('missing '+role+' capture'); continue
            require(sha256(p)==entry['sha256'], 'artifact hash mismatch')
            sources[role]=p
        if len(sources)<3:
            result['classification']='INCOMPLETE'; return result
        start=number(m['window']['startSeconds']); duration=number(m['measurementSeconds'],True)
        policy=m['qualityPolicy']; tolerance=number(policy['boundaryToleranceSeconds'])
        if m['warmupSeconds']<policy['minimumWarmupSeconds']:
            incomplete=True; reasons.append('insufficient warm-up')
        adapter=presentmon if m['capture']['format'].startswith('presentmon') else normalized_frames
        frame,times=adapter(sources['frames'],m['capture'],m['process'],start,duration,tolerance)
        metrics['frames_ms']=frame
        if frame['count']<policy['minimumFrames']:
            incomplete=True; reasons.append('insufficient frame samples')
        if not frame['boundary_coverage']:
            incomplete=True; reasons.append('partial frame capture or startup/shutdown gap')
        if frame['count']<10000: warnings.append('P99.9 unavailable below 10000 intervals; lower tails descriptive')
        if frame['long_gaps_over_1000ms']: warnings.append('long frame gaps retained; inspect capture loss versus actual stutter')
        if adapter is presentmon: warnings.append('ETW display estimate requires real Minecraft/OpenGL calibration')
        mapping,drift,uncertainty=clock_mapping(m['clocks']['gcUptimeToRun'])
        slope=(m['clocks']['gcUptimeToRun']['run1']-m['clocks']['gcUptimeToRun']['run0'])/(m['clocks']['gcUptimeToRun']['source1']-m['clocks']['gcUptimeToRun']['source0'])
        gc_start=m['clocks']['gcUptimeToRun']['source0']+(start-m['clocks']['gcUptimeToRun']['run0'])/slope
        metrics['gc'],gc_events=gc_metrics(sources['gc'],gc_start,duration/slope,p1)
        metrics['clock']={'driftPpm':drift,'uncertaintyMs':uncertainty,'causalityEstablished':False}
        # Record proximity only. End timestamps identify GC summary completion.
        events=[(mapping(a),mapping(b)) for a,b in gc_events]
        overlaps=sum(any(gc_start-uncertainty/1000<=frame_end and gc_end+uncertainty/1000>=frame_start
                         for gc_start,gc_end in events)
                     for frame_start,frame_end in zip(times,times[1:])
                     if start<=frame_start and frame_end<=start+duration and frame_end-frame_start>.05)
        metrics['gc_stutter_proximity']={'stutter_intervals_near_gc':overlaps,'causalityEstablished':False}
        metrics['process']=process_metrics(sources['process'],m['process'],start,duration,tolerance)
        if not metrics['process']['boundary_coverage']:
            incomplete=True; reasons.append('partial process capture')
        if metrics['process']['sample_interval_seconds_max']>policy['maxProcessIntervalSeconds']:
            incomplete=True; reasons.append('process sampling gap')
        if uncertainty>5: warnings.append('clock uncertainty exceeds 5 ms; proximity is coarse')
        if 'startupSeconds' in m:
            require(set(m['startupSeconds'])=={'jvm','mainMenu','worldLoad'}, 'invalid startup markers')
            for value in m['startupSeconds'].values():
                if value is not None: number(value)
            metrics['startup_seconds']={k:None if v is None else number(v) for k,v in m['startupSeconds'].items()}
        else: metrics['startup_seconds']=None
        cohort={k:m[k] for k in ('sampleKind','workloadId','runtime','minecraft','mods','graphics','scene','hardware','qualityPolicy','warmupSeconds','measurementSeconds')}
        # Origins, stream addresses and preflight hashes legitimately differ per run.
        cohort['capture']={k:v for k,v in m['capture'].items() if k not in ('originQpc','swapChain')}
        cohort['environment']={k:v for k,v in m['environment'].items() if k not in ('quietPreflightSha256','thermals')}
        result['cohortSha256']=hashlib.sha256(json.dumps(cohort,sort_keys=True).encode()).hexdigest()
        result['configurationSha256']=hashlib.sha256(json.dumps(m['jvmArguments']).encode()).hexdigest()
        result['classification']='INCOMPLETE' if incomplete else 'VALID WITH WARNINGS' if warnings else 'VALID'
    except (Invalid,p1.InvalidData) as error:
        reasons.append(str(error))
        result['classification']='INVALID'
    except (ValueError,KeyError,TypeError,OSError,OverflowError,csv.Error):
        # Never echo raw data, private absolute paths, GC lines or untrusted metadata.
        reasons.append('invalid metadata, clock, identity, hash or capture structure')
        result['classification']='INVALID'
    return result


def cohort_results(paths):
    results=[analyze(p) for p in paths]
    cohorts={r.get('cohortSha256') for r in results if r['classification'].startswith('VALID')}
    if len(cohorts)>1:
        for r in results:
            if r['classification'].startswith('VALID'):
                r['classification']='INVALID'; r['reasons'].append('inconsistent runtime, mods, workload, render settings or conditions across cohort')
    ids=[r['runId'] for r in results if r['runId']]
    if len(ids)!=len(set(ids)):
        for r in results:
            r['classification']='INVALID'; r['reasons'].append('duplicate run identifier')
    identities={};pairs=set()
    for r in results:
        if not r['classification'].startswith('VALID'):continue
        config=r['configurationId'];signature=r['configurationSha256'];pair=(config,r['blockId'])
        if pair in pairs or (config in identities and identities[config]!=signature):
            for candidate in results:
                if candidate.get('configurationId')==config:
                    candidate['classification']='INVALID';candidate['reasons'].append('configuration changed or configuration/block repeated')
        pairs.add(pair);identities[config]=signature
    return results


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifests',nargs='+',type=Path)
    args=parser.parse_args()
    results=cohort_results(args.manifests)
    print(json.dumps({'schemaVersion':1,'runs':results,'performanceConclusion':'NOT MEASURED by this tool'},indent=2,allow_nan=False))
    return 2 if any(r['classification'] in ('INVALID','INCOMPLETE') for r in results) else 0


if __name__=='__main__':
    raise SystemExit(main())
