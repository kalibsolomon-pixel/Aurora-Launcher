"""Summarize numeric observations; private PID correlations never enter exports."""
import hashlib
import json
import math
import pathlib
import statistics

base = pathlib.Path(__file__).resolve().parent
private = base / 'private/resume-2026-10-09'

def rows(path):
    return [json.loads(line) for line in path.read_text(encoding='utf-8-sig').splitlines() if line.strip()]

def stats(values):
    ordered = sorted(values)
    return {'n': len(values), 'firstMs': values[0], 'p50Ms': statistics.median(values),
            'empiricalP95Ms': ordered[math.ceil(.95 * len(values)) - 1] if len(values) >= 20 else None,
            'worstMs': max(values), 'worstRepeatMs': max(values[1:]) if len(values) > 1 else None,
            'sampleStdDevMs': statistics.stdev(values) if len(values) > 1 else None,
            'sampleVarianceMs2': statistics.variance(values) if len(values) > 1 else None}

ui = rows(base / 'evidence/as-is-ui-observations.jsonl')
public_processes = base / 'evidence/as-is-process-timings.json'
processes = json.loads(public_processes.read_text()) if public_processes.exists() else rows(private / 'process-observations.jsonl')
games = rows(base / 'evidence/as-is-game-observations.jsonl')
exits = json.loads((base / 'evidence/as-is-supervised-exits.json').read_text())
summary = {'durationInterpretation': 'automation-request to observed state; upper bounds include input and observation overhead',
           'ui': {}, 'games': [], 'idle': {}, 'sourceProvenanceOfInstalledBinary': 'unknown'}
safe_processes = []
for boundary in ('startup_home_ready', 'startup_ready_stable_1s', 'startup_navigation_response', 'mods_refresh_complete'):
    selected = [r for r in ui if r.get('phase') == 'measurement' and r['boundary'] == boundary and r.get('success')]
    summary['ui'][boundary] = stats([r['observedUpperMs'] for r in selected])
    widths = [r['observationWidthMs'] for r in selected if 'observationWidthMs' in r]
    if widths:
        summary['ui'][boundary]['medianObservationWidthMs'] = statistics.median(widths)
        summary['ui'][boundary]['maxObservationWidthMs'] = max(widths)

marker = next(r for r in ui if r['boundary'] == 'content_series_metadata')
end = next(r for r in ui if r['boundary'] == 'content_series_end')
for domain in ('mods', 'resource_packs', 'shaders'):
    selected = [r for r in ui if r.get('phase') == 'measurement' and r['boundary'] == domain + '_populated'
                and r.get('success') and marker['firstActionWallMs'] <= r['actionRequestWallMs'] < end['endedWallMs']]
    assert len(selected) == 21, (domain, len(selected))
    repeated = selected[1:]
    summary['ui'][domain + '_populated'] = {'firstInFinalProcessMs': selected[0]['observedUpperMs'],
        'repeats': stats([r['observedUpperMs'] for r in repeated]),
        'medianObservationWidthMs': statistics.median(r['observationWidthMs'] for r in repeated),
        'maxObservationWidthMs': max(r['observationWidthMs'] for r in repeated),
        'maxCaptureMs': max(r['lastCaptureMs'] for r in repeated),
        'expectedRows': selected[0]['expectedRows']}

for game in games:
    log = next(r for r in exits if r['trial'] == game['trial'])
    matches = [p for p in processes if p['kind'] == 'javaChild' and
               log['createdWallMs'] <= p['createdWallMs'] < log['createdWallMs'] + 1000]
    assert len(matches) == 1
    child = matches[0]
    safe_processes.append({'kind': 'javaChild', 'trialAlias': game['trial'],
                           'createdWallMs': child['createdWallMs'], 'observedWallMs': child['observedWallMs'],
                           'correlation': 'exact installed-launcher parentage retained privately; unique supervisor-log creation pairing'})
    spawn = child['createdWallMs'] - game['actionRequestWallMs']
    result = dict(game)
    result['playRequestToOsChildCreationMs'] = spawn
    result['childObservationDelayMs'] = child['observedWallMs'] - child['createdWallMs']
    result['childCreationAfterSupervisorLogMs'] = child['createdWallMs'] - log['createdWallMs']
    result['childAttribution'] = 'unique exact-parent Java child within 1s after independent supervisor-log creation; earlier diagnostic child excluded; native returned handle not instrumented'
    result['supervisedExitCode'] = log['exitCode']
    result['osChildCreationToMenuObservedUpperMs'] = game['menuWithoutOverlayObservedUpperMs'] - spawn
    if 'windowObservedUpperMs' in game:
        result['osChildCreationToWindowObservedUpperMs'] = game['windowObservedUpperMs'] - spawn
        result['osChildCreationToWindowObservedLowerMs'] = game['windowObservedLowerMs'] - spawn
    result['menuLowerObservationInterpretation'] = 'previous loading screenshot return time; not a strict present-time lower bound'
    summary['games'].append(result)

starts = [r for r in ui if r['boundary'] == 'startup_home_ready' and r.get('phase') == 'measurement' and r.get('success')]
paired = []
for start in starts:
    matches = [p for p in processes if p['kind'] == 'launcher' and
               start['actionRequestWallMs'] <= p['createdWallMs'] < start['observedWallMs']]
    assert len(matches) == 1, ('startup OS identity matches', start['trial'], len(matches))
    safe_processes.append({'kind': 'launcher', 'trialAlias': start['trial'],
                           'createdWallMs': matches[0]['createdWallMs'], 'observedWallMs': matches[0]['observedWallMs']})
    paired.append(start['observedUpperMs'] - (matches[0]['createdWallMs'] - start['actionRequestWallMs']))
summary['ui']['os_process_creation_to_first_ready_proxy'] = stats(paired)

for view in ('home', 'workspace'):
    samples = rows(base / ('evidence/as-is-idle-' + view + '.jsonl'))
    assert len(samples) == 6 and all(r['stableProcessTree'] and r['backgroundMinecraftProcesses'] == 0 for r in samples)
    summary['idle'][view] = {'samples': len(samples), 'seconds': sum(r['seconds'] for r in samples),
        'meanCpuOneCorePercent': sum(r['cpuOneCorePercent'] * r['seconds'] for r in samples) / sum(r['seconds'] for r in samples),
        'workingSetMiBRange': [min(r['workingSetMiB'] for r in samples), max(r['workingSetMiB'] for r in samples)],
        'privateMiBRange': [min(r['privateMiB'] for r in samples), max(r['privateMiB'] for r in samples)],
        'processes': sorted(set(r['processes'] for r in samples)),
        'handlesRange': [min(r['handles'] for r in samples), max(r['handles'] for r in samples)],
        'threadsRange': [min(r['threads'] for r in samples), max(r['threads'] for r in samples)]}
summary['observationOutcomes'] = {'pilotRecords': sum(r.get('phase') == 'pilot' for r in ui),
    'unsuccessfulMeasurementRecords': [{'boundary': r['boundary'], 'timeoutMs': r.get('timeoutMs'),
                                       'inconclusiveIfNoBusy': r.get('inconclusiveIfNoBusy', False)}
                                      for r in ui if r.get('phase') == 'measurement' and r.get('success') is False]}
summary['inputDigests'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in
                         sorted((base / 'evidence').glob('as-is-*.jsonl'))}
if not public_processes.exists():
    public_processes.write_text(json.dumps(safe_processes, indent=2), encoding='utf-8')
summary['inputDigests'][public_processes.name] = hashlib.sha256(public_processes.read_bytes()).hexdigest()
summary['inputDigests']['as-is-supervised-exits.json'] = hashlib.sha256((base / 'evidence/as-is-supervised-exits.json').read_bytes()).hexdigest()
(base / 'evidence/as-is-summary.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
print(json.dumps(summary, indent=2))
