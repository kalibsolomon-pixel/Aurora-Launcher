"""Offline projection for the predeclared Java initiative; never launches a game.

Reuses the Client C-PERF.2B exact-format 60s CSV/process arithmetic. Does not
change historical strict/practical schemas or certify display presentation.
"""
import argparse
import collections
import datetime as dt
import hashlib
import json
import math
import re
import subprocess
import sys
from pathlib import Path

from collector_review import percentile
from world_scene import level


def instant(text):
    return dt.datetime.fromisoformat(text.replace('Z', '+00:00'))


def seconds(text):
    match = re.fullmatch(r'PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d+)?)S)?', text)
    assert match and any(match.groups()), 'Unsupported duration'
    return int(match[1] or 0) * 3600 + int(match[2] or 0) * 60 + float(match[3] or 0)


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def gc_review(directory, java_bin, start, end):
    raw = directory / 'gc.jfr'
    assert 0 < raw.stat().st_size < 32 * 1024**2
    summary_path = directory / 'jfr-summary.txt'
    events_path = directory / 'jfr-events.json'
    selected = 'jdk.GarbageCollection,jdk.GCPhasePause,jdk.GCHeapSummary,jdk.DataLoss,jdk.SafepointBegin,jdk.SafepointEnd'
    # Derived exports may be regenerated; raw recording is never changed.
    summary = subprocess.run([str(java_bin / 'jfr.exe'), 'summary', str(raw)],
                             capture_output=True, check=True, timeout=20).stdout
    summary_path.write_bytes(summary)
    data = subprocess.run([str(java_bin / 'jfr.exe'), 'print', '--json', '--events', selected, str(raw)],
                          capture_output=True, check=True, timeout=20).stdout
    events_path.write_bytes(data)
    text = summary.decode()
    counts = {m[1]: int(m[2]) for line in text.splitlines()
              if (m := re.match(r'\s+((?:jdk|minecraft)\.\w+)\s+(\d+)\s+\d+', line))}
    forbidden = ['JVMInformation', 'InitialSystemProperty', 'InitialEnvironmentVariable',
                 'InitialSecurityProperty', 'SystemProcess', 'ExecutionSample', 'ObjectAllocationSample']
    assert all(counts.get('jdk.' + name, 0) == 0 for name in forbidden)
    assert counts.get('jdk.DataLoss') == 0
    recording_start = dt.datetime.strptime(re.search(r'Start: (.+) \(UTC\)', text)[1],
                                          '%Y-%m-%d %H:%M:%S').replace(tzinfo=dt.timezone.utc)
    duration = int(re.search(r'Duration: (\d+) s', text)[1])
    # Summary start/duration are rounded. Require an extra second at both ends.
    assert recording_start + dt.timedelta(seconds=1) < start
    assert recording_start + dt.timedelta(seconds=duration - 1) > end
    events = json.loads(data)['recording']['events']
    chosen = []
    crossing = collections.Counter()
    for event in events:
        value = event['values']
        beginning = instant(value['startTime'])
        finish = beginning + dt.timedelta(seconds=seconds(value.get('duration', 'PT0S')))
        if start <= beginning and finish <= end:
            chosen.append(event)
        elif beginning < end and finish > start:
            crossing[event['type']] += 1
    values = lambda name: [e['values'] for e in chosen if e['type'] == 'jdk.' + name]
    pauses = [seconds(e['duration']) * 1000 for e in values('GCPhasePause')]
    collections_in_window = values('GarbageCollection')
    heaps = values('GCHeapSummary')
    return dict(rawSha256=sha(raw), rawBytes=raw.stat().st_size, eventCounts=counts,
                analysisWindow=dict(startUtc=start.isoformat(), endUtc=end.isoformat(),
                                    selection='Fully contained events; boundary crossings retained separately'),
                crossing=dict(crossing), collectionCount=len(collections_in_window),
                collectionNames=dict(collections.Counter(e['name'] for e in collections_in_window)),
                causes=dict(collections.Counter(e['cause'] for e in collections_in_window)),
                pauseCount=len(pauses), pauseTotalMs=math.fsum(pauses),
                pauseMedianMs=percentile(pauses, .5) if pauses else None,
                pauseP95Ms=percentile(pauses, .95) if pauses else None,
                pauseP99Ms=percentile(pauses, .99) if pauses else None,
                pauseMaxMs=max(pauses, default=None),
                heap={when: dict(samples=len(rows := [e for e in heaps if e['when'] == when]),
                    usedMinBytes=min((e['heapUsed'] for e in rows), default=None),
                    usedMaxBytes=max((e['heapUsed'] for e in rows), default=None),
                    committedMinBytes=min((e['heapSpace']['committedSize'] for e in rows), default=None),
                    committedMaxBytes=max((e['heapSpace']['committedSize'] for e in rows), default=None))
                    for when in ['Before GC', 'After GC']},
                preciseFrameAlignment=False, activeGcCpu=None)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('client_repo', type=Path)
    parser.add_argument('orders', type=int, nargs='+')
    parser.add_argument('--replacement-three', action='store_true',
                        help='Use the sole declared external-interruption replacement')
    args = parser.parse_args()
    client = args.client_repo.resolve()
    root = client / 'run/jvm-initiative'
    inputs = json.loads((root / 'inputs.json').read_text())
    sys.path.insert(0, str(client / 'tools/performance'))
    import cperf2b_review as existing
    # Only the relative-path presentation root changes. Math and validity checks
    # for this exact producer's CSV and explicit-process capture remain unchanged.
    existing.P = root
    results = []
    for order in args.orders:
        if order == 3 and args.replacement_three:
            declaration = json.loads((root / 'replacement-declaration.json').read_text())
            assert declaration['original'] == 'run-3' and declaration['replacement'] == 'replacement-3'
            directory = root / 'replacement-3'
        else:
            directory = root / f'run-{order}'
        expected = inputs['runs'][order - 1]
        runtime = (directory / 'runtime.log').read_text(errors='replace')
        assert 'BUILD SUCCESSFUL' in runtime and 'Stopping!' in runtime
        assert 'OutOfMemoryError' not in runtime and 'PARK diagnostics ACTIVE' not in runtime
        vm = json.loads((directory / 'effective-vm.json').read_text())
        assert int(vm['MaxHeapSize']) == expected['maxHeapMiB'] * 1024**2
        assert vm['UseZGC'] == vm['ZGenerational'] == (expected['label'] == 'C')
        assert vm['UseG1GC'] == (expected['label'] != 'C')
        player = level(directory / 'game/saves/Cperf1a/level.dat')['Player']
        assert player['Pos'] == inputs['sourcePose']['position']
        assert player['Rotation'] == inputs['sourcePose']['rotation']
        cfg = json.loads((directory / 'game/config/aurora.json').read_text())
        assert cfg['frameCapFps'] == 240 and cfg['framePacingStrategy'] == 'PARK'
        assert cfg['framePacerSpinThresholdMicros'] == 500 and not cfg['adaptiveRenderSleeping']
        files = list((directory / 'game/config/performanceoverlay/benchmarks').glob('*.csv'))
        assert len(files) == 1
        frames = existing.frames(files[0]); frames.pop('producer')
        assert frames['heapMaxMiB'] == [str(expected['maxHeapMiB'])]
        resources = existing.resources(directory / 'resources')
        event_log = [json.loads(line) for line in (directory / 'events.jsonl').read_text().splitlines()]
        times = {event['event']: instant(event['utc']) for event in event_log}
        assert (times['warmup-end'] - times['warmup-start']).total_seconds() >= 60
        assert times['primary-trigger-before'] <= instant(resources['startUtc'])
        assert instant(resources['endUtc']) <= times['after-primary-camera']
        gc = gc_review(directory, Path(inputs['javaExecutable']).parent,
                       instant(resources['startUtc']), instant(resources['endUtc']))
        result = dict(order=order, label=expected['label'], valid=True, vm=vm, frames=frames,
                      resources=resources, gc=gc, cameraExactMatch=True,
                      warmupSeconds=(times['warmup-end'] - times['warmup-start']).total_seconds())
        (directory / 'review.json').write_text(json.dumps(result, indent=2) + '\n')
        results.append(result)
    (root / 'review-summary.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps([dict(order=r['order'], label=r['label'], fps=r['frames']['fps'],
        p99Ms=r['frames']['p99Ms'], rssMiB=r['resources']['rssMiBMax'],
        gcPauses=r['gc']['pauseCount'], gcPauseTotalMs=r['gc']['pauseTotalMs']) for r in results], indent=2))


if __name__ == '__main__':
    main()
