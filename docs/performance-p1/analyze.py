"""Validate P0.2 numeric traces and derive P1 matched results; never opens apps."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('p02', HERE.parent / 'performance-p0-2/analyze.py')
p02 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p02)

def analyze(root, evidence):
    evidence.mkdir(exist_ok=True)
    trials, traces = [], {}
    for folder in sorted((root / 'trials').iterdir()):
        if not (folder / 'ui.json').exists():
            raise AssertionError('trial without UI outcome')
        row = json.loads((folder / 'ui.json').read_text(encoding='utf-8-sig'))
        assert row['success'], 'failed UI trial requires explicit exclusion'
        assert json.loads((folder / 'close.json').read_text()) == {'normalWindowClose': True}
        row['phase'] = 'pilot' if row['trial'] == 0 else 'matched' if row['cohort'] in ['baseline', 'candidate'] and 1 <= row['trial'] <= 20 else 'diagnostic'
        row['closedNormally'] = True # independently confirmed by close tool and complete exit trace
        trace = folder / 'native-trace.json'
        if row['cohort'].endswith('control'):
            assert not trace.exists(), 'feature-off build produced a research trace'
        else:
            assert trace.exists(), 'complete exit-written trace required'
            result = p02.trace(trace)
            name = 'trace-' + folder.name + '.json'
            traces[name] = result
            destination = evidence / name
            if destination.exists(): assert destination.read_bytes() == trace.read_bytes()
            else: destination.write_bytes(trace.read_bytes())
        trials.append(row)
    groups = {}
    for cohort in sorted({r['cohort'] for r in trials}):
        rows = [r for r in trials if r['cohort'] == cohort and r['phase'] == ('matched' if cohort in ['baseline', 'candidate'] else 'diagnostic')]
        groups[cohort] = {'n': len(rows), 'firstReadyUpper': p02.stats([r['firstReadyUpperMs'] for r in rows]),
                          'stableReadyUpper': p02.stats([r['stableReadyUpperMs'] for r in rows]),
                          'worstCaptureMs': max(r['maxCaptureMs'] for r in rows),
                          'worstAttachMs': max(r['observerAttachedMs'] for r in rows)}
        selected = [traces[f'trace-{cohort}-{r["trial"]:02}.json'] for r in rows if not cohort.endswith('control')]
        if selected:
            groups[cohort].update(
                nativeFirstReady=p02.stats([t['firstReadyAfterNativeOriginMs'] for t in selected]),
                nativeFinalReady=p02.stats([t['lastReadyAfterNativeOriginMs'] for t in selected]),
                frontendFirstReady=p02.stats([t['firstReadyAfterFrontendInitMs'] for t in selected]),
                frontendFinalReady=p02.stats([t['lastReadyAfterFrontendInitMs'] for t in selected]),
                readyCheckingReadyRuns=sum(bool(t['readyFlashMs']) for t in selected),
                countsPerRun={key: sorted({t['counts'][key] for t in selected}) for key in selected[0]['counts']},
                backgroundAuditCounts=sorted({t['unattributedGameAudits'] for t in selected}),
                ipcPerRun=[t['ipcRtt'] for t in selected if t['ipcRtt']['n']])
    pairs = []
    for baseline in [r for r in trials if r['cohort'] == 'baseline' and r['phase'] == 'matched']:
        candidate = next(r for r in trials if r['cohort'] == 'candidate' and r['trial'] == baseline['trial'])
        a, b = [traces[f'trace-{label}-{baseline["trial"]:02}.json'] for label in ['baseline', 'candidate']]
        pairs.append({'trial': baseline['trial'],
                      'firstReadyUpperDeltaMs': candidate['firstReadyUpperMs'] - baseline['firstReadyUpperMs'],
                      'stableReadyUpperDeltaMs': candidate['stableReadyUpperMs'] - baseline['stableReadyUpperMs'],
                      'nativeFirstReadyDeltaMs': b['firstReadyAfterNativeOriginMs'] - a['firstReadyAfterNativeOriginMs'],
                      'nativeFinalReadyDeltaMs': b['lastReadyAfterNativeOriginMs'] - a['lastReadyAfterNativeOriginMs'],
                      'frontendFinalReadyDeltaMs': b['lastReadyAfterFrontendInitMs'] - a['lastReadyAfterFrontendInitMs']})
    summary = {'cohorts': groups, 'pairedCandidateMinusBaseline': {
        key: p02.stats([r[key] for r in pairs]) for key in pairs[0] if key != 'trial'},
        'pairs': pairs, 'traces': traces, 'gameLaunches': 0,
        'backgroundOwnership': 'parent-zero background audit; command ownership remains unproved',
        'notes': ['UI metrics are observation upper bounds; stable includes a one-second hold.',
                  'Eight-second observation prevents early close after a transient Ready.',
                  'Native-origin and frontend-init clocks are separate from OS process creation.',
                  'No p95 is emitted for fewer than 20 samples.']}
    # Adjacent feature-on/off diagnostics use the same source and balance order.
    # These are kept out of the primary optimization cohort.
    overhead = {}
    for source in ['baseline', 'candidate']:
        deltas = []
        for control in [r for r in trials if r['cohort'] == source + '-control']:
            profile = next(r for r in trials if r['cohort'] == source and r['trial'] == control['trial'] + 20)
            deltas.append(profile['stableReadyUpperMs'] - control['stableReadyUpperMs'])
        if deltas: overhead[source] = p02.stats(deltas)
    summary['pairedInstrumentationMinusFeatureOffStableReady'] = overhead
    (evidence / 'trials.json').write_text(json.dumps(trials, indent=2), encoding='utf-8')
    (evidence / 'summary.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
    print(json.dumps({'cohorts': groups, 'paired': summary['pairedCandidateMinusBaseline']}, indent=2))

if __name__ == '__main__':
    analyze(Path(sys.argv[1]), HERE / 'evidence')
