"""Strict numeric-only P2 attribution and matched-trial analysis; no app I/O."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import statistics
import sys

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE / 'evidence'
spec = importlib.util.spec_from_file_location('p02', HERE.parent / 'performance-p0-2/analyze.py')
p02 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p02)
EXTRA = {39: 'startupUpdateCheck', 40: 'manualUpdateCheck', 41: 'clientPreviewCommand',
         42: 'clientDiscovery', 43: 'clientPreview', 44: 'instanceValidation',
         45: 'clientFingerprint', 46: 'launcherDiscovery', 47: 'clientManifestHttp', 48: 'updatePublication'}
p02.NAMES.update(EXTRA)

def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))

def attributed(path):
    # Reuse all P0.2 bounds, numeric fields, balanced-span, no-Play and
    # frontend-overflow checks, extending only the closed event vocabulary.
    ordinary = p02.trace(path)
    doc = read(path)
    starts = {r['id']: r for r in doc['records'] if r['edge'] == 1}
    ends = {r['id']: r for r in doc['records'] if r['edge'] == 2}
    assert starts.keys() == ends.keys()
    def chain(s):
        result, seen = [], set()
        while s['id']:
            assert s['id'] not in seen
            seen.add(s['id'])
            result.append(s)
            if s['parent'] == 0: break
            parent = starts[s['parent']]
            assert parent['us'] <= s['us'] <= ends[s['id']]['us'] <= ends[parent['id']]['us']
            s = parent
        return result
    for s in starts.values(): chain(s)
    def select(event): return [r for r in starts.values() if r['event'] == event]
    def duration(s): return (ends[s['id']]['us'] - s['us']) / 1000
    def descendants(s): return [r for r in starts.values() if s['id'] in [p['id'] for p in chain(r)]]
    def only(event, rows):
        result = [s for s in rows if s['event'] == event]
        assert len(result) == 1, (event, len(result))
        return result[0]
    ready = [r['us'] / 1000 for r in doc['records'] if r['event'] == 106]
    checking = [r['us'] / 1000 for r in doc['records'] if r['event'] == 105]
    assert not any(r > ready[0] for r in checking), 'later Checking must be investigated'
    result = dict(ordinary=ordinary)
    if select(39):
        root = only(39, starts.values())
        assert root['parent'] == 0
        work = descendants(root)
        audit = only(20, work)
        lineage = chain(audit)
        assert [r['event'] for r in lineage] == [20, 44, 43, 42, 39]
        fingerprint = only(45, work)
        preview = only(43, work)
        validation = only(44, work)
        mods = [r for r in work if r['event'] == 23]
        assert len(mods) == 2
        assert sorted(r['parent'] for r in mods) == sorted([validation['id'], fingerprint['id']])
        assert not any(r['event'] in (21, 22, 36, 37, 38) for r in work)
        assert not select(40) and not select(41)
        assert ordinary['unattributedGameAudits'] == 0
        a, b = audit['us'] / 1000, ends[audit['id']]['us'] / 1000
        readiness = only(11, starts.values())
        ra, rb = readiness['us'] / 1000, ends[readiness['id']]['us'] / 1000
        result['attribution'] = dict(rootCommandEvent=39, rootRequestId=root['id'],
            auditId=audit['id'], auditToRoot=[dict(event=r['event'], id=r['id'], parent=r['parent']) for r in lineage],
            rootInclusiveMs=duration(root), clientDiscoveryInclusiveMs=duration(only(42, work)),
            launcherDiscoveryInclusiveMs=duration(only(46, work)), clientManifestHttpInclusiveMs=duration(only(47, work)),
            previewInclusiveMs=duration(preview), validationInclusiveMs=duration(validation),
            fingerprintInclusiveMs=duration(fingerprint), auditInclusiveMs=duration(audit),
            auditHashCalls=ends[audit['id']]['hash_calls'], auditHashIoDigestMs=ends[audit['id']]['hash_us'] / 1000,
            auditStartNativeMs=a, auditEndNativeMs=b,
            readyDuringAudit=a <= ready[0] <= b, readinessRequestOverlapMs=max(0, min(b, rb) - max(a, ra)),
            auditEndMinusFirstReadyMs=b - ready[0],
            updateEndMinusFirstReadyMs=ends[root['id']]['us'] / 1000 - ready[0],
            updateEndsAfterReady=ends[root['id']]['us'] / 1000 > ready[0],
            correlatedModScans=[dict(id=r['id'], parent=r['parent'], inclusiveMs=duration(r), hashCalls=ends[r['id']]['hash_calls']) for r in mods],
            correlatedMetadataResolutions=0,
            publicationId=only(48, work)['id'])
    else:
        audits = [r for r in select(20) if r['parent'] == 0]
        assert len(audits) == 1
        result['baselineUnattributedAuditInclusiveMs'] = duration(audits[0])
    assert ordinary['counts']['gameIntegrity'] == 3
    assert ordinary['counts']['readiness'] == ordinary['counts']['runtimeStatus'] == 1
    assert ordinary['counts']['modInventory'] == 5
    assert ordinary['counts']['gameMetadata'] == ordinary['counts']['runtimeMetadata'] == 2
    return result

def main(root=None):
    if root:
        trials = []
        for folder in sorted((root / 'trials').iterdir()):
            row = read(folder / 'ui.json')
            close = read(folder / 'close.json')
            assert row['success'] and close['normalWindowClose']
            row.update(normalWindowClose=True, phase='pilot' if row['trial'] == 0 else 'control' if row['cohort'] == 'control' else 'matched')
            row['artifactSha256'] = read(EVIDENCE / (row['cohort'] + '-artifact.json'))['executableSha256']
            trace = folder / 'native-trace.json'
            if row['cohort'] == 'control': assert not trace.exists()
            else:
                attributed(trace)
                shutil.copy2(trace, EVIDENCE / ('trace-' + folder.name + '.json'))
            trials.append(row)
        (EVIDENCE / 'trials.json').write_text(json.dumps(trials, indent=2))
    trials = read(EVIDENCE / 'trials.json')
    traces = {p.name: attributed(p) for p in sorted(EVIDENCE.glob('trace-*.json'))}
    primary = [r for r in trials if r['phase'] == 'matched']
    assert len(primary) == 12
    attr = [traces[f"trace-attribution-{r['trial']:02}.json"]['attribution'] for r in primary if r['cohort'] == 'attribution']
    cohorts = {}
    for cohort in ['baseline', 'attribution', 'control']:
        rows = [r for r in trials if r['cohort'] == cohort and r['phase'] != 'pilot']
        cohorts[cohort] = dict(firstReadyUpperMs=p02.stats([r['firstReadyUpperMs'] for r in rows]),
                               sustainedReadyUpperMs=p02.stats([r['stableReadyUpperMs'] for r in rows]),
                               worstCaptureMs=max(r['maxCaptureMs'] for r in rows),
                               worstObserverAttachmentMs=max(r['observerAttachedMs'] for r in rows))
    pairs = []
    for number in range(1, 7):
        a = next(r for r in primary if r['cohort'] == 'attribution' and r['trial'] == number)
        b = next(r for r in primary if r['cohort'] == 'baseline' and r['trial'] == number)
        pairs.append(dict(pair=number, firstReadyDeltaMs=a['firstReadyUpperMs']-b['firstReadyUpperMs'],
                          sustainedReadyDeltaMs=a['stableReadyUpperMs']-b['stableReadyUpperMs']))
    metrics = ['rootInclusiveMs', 'clientDiscoveryInclusiveMs', 'launcherDiscoveryInclusiveMs', 'clientManifestHttpInclusiveMs',
               'previewInclusiveMs', 'validationInclusiveMs', 'fingerprintInclusiveMs', 'auditInclusiveMs',
               'auditHashIoDigestMs', 'readinessRequestOverlapMs', 'auditEndMinusFirstReadyMs', 'updateEndMinusFirstReadyMs']
    result = dict(primaryPairs=6, primaryStarts=12, cohorts=cohorts, pairs=pairs,
                  pairedAttributionMinusP1={key:p02.stats([r[key] for r in pairs]) for key in ['firstReadyDeltaMs', 'sustainedReadyDeltaMs']},
                  attributedMetrics={key:p02.stats([r[key] for r in attr]) for key in metrics},
                  readyDuringAudit=sum(r['readyDuringAudit'] for r in attr),
                  updateEndsAfterReady=sum(r['updateEndsAfterReady'] for r in attr),
                  correlatedModScansMs=p02.stats([s['inclusiveMs'] for r in attr for s in r['correlatedModScans']]),
                  directReadinessWaitForUpdate=False, optimizationImplemented=False, traces=traces)
    (EVIDENCE / 'summary.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k:v for k,v in result.items() if k != 'traces'}, indent=2))

if __name__ == '__main__': main(Path(sys.argv[1]) if sys.argv[1:] else None)
