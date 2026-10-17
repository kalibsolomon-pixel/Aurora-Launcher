"""Offline, short-only submission fidelity review. Never certifies physical display."""
import argparse
from collections import Counter
from datetime import datetime
import json
import math
from pathlib import Path
import re
import statistics

import quality as q
import short_calibration as short
from calibration_review import loss_state


def interval_metrics(times, start, duration):
    result = q.frame_metrics(times, start, duration, .25)
    # The measured analyzer's historical keys say "stutter". This separate
    # unvalidated ETW channel must describe observed gaps without that claim.
    result['intervalsOver50Ms'] = result.pop('stutter_frames_over_50ms')
    result['gapRunsOver50Ms'] = result.pop('stutter_events_over_50ms')
    result.pop('stutter_frames_per_minute')
    result['intervalsOver1000Ms'] = result.pop('long_gaps_over_1000ms')
    return result


def reviewed_loss(snapshots, endpoint_ns):
    if snapshots is None: return 'UNKNOWN'
    q.require(isinstance(snapshots, list), 'loss snapshots must be a list')
    successful = []
    for snapshot in snapshots:
        q.require(isinstance(snapshot, dict), 'invalid loss snapshot')
        q.integer(snapshot.get('statusCode'))
        if snapshot['statusCode'] == 0:
            q.integer(snapshot.get('qpcNs'))
            for key in ('eventsLost', 'logBuffersLost', 'realTimeBuffersLost'):
                q.integer(snapshot.get(key))
            successful.append(snapshot)
    # An unavailable query cannot erase loss positively observed by another one.
    if any(s[k] for s in successful for k in ('eventsLost', 'logBuffersLost', 'realTimeBuffersLost')):
        return 'CONFIRMED LOSS'
    if len(successful) != len(snapshots): return 'UNKNOWN'
    return loss_state(snapshots, endpoint_ns)


def inspect(root, manifest, snapshots=None, claim='submission', identity=None):
    q.require(claim == 'submission', 'unsupported physical-display or rendered-frame claim')
    root, rows, diagnostic = short.read_capture(root, manifest)
    if identity is not None:
        q.require(isinstance(identity, dict), 'invalid independent process identity')
        q.integer(identity.get('pid'), True)
        q.require(identity.get('pid') == manifest['pid'] and
                  identity.get('creationId') == manifest['creationId'], 'independent process identity mismatch')
    frequency, origin = manifest['qpcFrequencyHz'], manifest['originQpc']
    start, duration = manifest['windowStartSeconds'], manifest['windowSeconds']
    wall = (datetime.fromisoformat(manifest['endUtc'].replace('Z', '+00:00')) -
            datetime.fromisoformat(manifest['startUtc'].replace('Z', '+00:00'))).total_seconds()
    q.require(start + duration <= wall, 'inner window exceeds capture lifetime')
    ticks, display, missing, pairs, previous = [], [], 0, [], None
    for row in rows:
        q.require(row['ProcessID'] == str(manifest['pid']) and
                  row['SwapChainAddress'] == manifest['swapChain'] and
                  row['PresentRuntime'] == manifest['presentRuntime'], 'target/stream mismatch')
        q.require(re.fullmatch(r'[0-9]+', row['TimeInQPC']) is not None, 'invalid QPC ticks')
        tick = int(row['TimeInQPC']); ticks.append(tick)
        if 'MsBetweenPresents' in row:
            exported = q.number(row['MsBetweenPresents'])
            if previous is not None:
                difference = abs(exported - (tick - previous) / frequency * 1000)
                q.require(difference <= .0002, 'QPC/export interval mismatch')
                pairs.append(difference)
        previous = tick
        if row['MsBetweenDisplayChange'] != 'NA': q.number(row['MsBetweenDisplayChange'], True)
        if row['MsUntilDisplayed'] == 'NA': missing += 1
        else:
            latency = q.number(row['MsUntilDisplayed'], True)
            display.append((tick - origin) / frequency + latency / 1000)
    q.strictly_increasing(ticks)
    q.require(0 < (ticks[-1] - ticks[0]) / frequency <= wall + .5, 'QPC units/capture span mismatch')
    times = [(tick - origin) / frequency for tick in ticks]
    calls = interval_metrics(times, start, duration)
    q.require(calls['count'] >= 10, 'insufficient submission samples')
    display_metrics, display_reason = None, 'unavailable or insufficient display estimates'
    if len(display) >= 2:
        try:
            display_metrics = interval_metrics(display, start, duration)
            display_reason = 'available estimates only; missingness is not optical drop evidence'
        except ValueError:
            display_reason = 'display estimates lack ordered complete inner intervals'
    fields = {}
    for name in rows[0]:
        if name in ('Application', 'ProcessID', 'TimeInQPC', 'CPUStartQPC'): continue
        values = [r[name] for r in rows]
        if name.startswith('Ms') or name == 'AnimationTime':
            available = []
            for value in values:
                if value == 'NA': continue
                number = float(value); q.require(math.isfinite(number), 'nonfinite diagnostic field')
                available.append(number)
            distribution = None
            if available:
                # AnimationError and some latency diagnostics are signed. Preserve them.
                shift = min(0, min(available))
                distribution = q.p1.distribution([v - shift for v in available])
                for key in ('mean', 'median', 'p95', 'p99', 'p999', 'min', 'max'):
                    if distribution.get(key) is not None: distribution[key] += shift
                distribution['mean'] = statistics.mean(available)
            fields[name] = dict(unavailable=values.count('NA'), zero=sum(v == 0 for v in available),
                                distribution=distribution)
        else:
            counts = Counter(values)
            q.require(len(counts) <= 64, 'unexpected diagnostic vocabulary cardinality')
            fields[name] = dict(counts)
    loss = reviewed_loss(snapshots, (origin * 1_000_000_000) // frequency + int((start + duration) * 1e9))
    state = 'VALID WITH WARNINGS' if calls['boundary_coverage'] and manifest['status'] == 'complete' and loss != 'CONFIRMED LOSS' else 'INCOMPLETE'
    return dict(artifactKind='short-submission-fidelity', quality=state,
                semantic='etw-application-submission-proxy', rawRows=len(rows),
                submissionIntervals=calls, unavailableDisplayRows=missing,
                unavailableDisplayFraction=missing / len(rows), displayEstimateIntervals=display_metrics,
                displayInterpretation=display_reason, fields=fields,
                exportedIntervalMaxDisagreementMs=max(pairs) if pairs else None,
                lossStatus=loss,
                uncertainty=dict(qpcTickMs=1000 / frequency, displayLatencyRoundingMs=.00005,
                                 etwToApiEntryMs=None, physicalScanoutMs=None,
                                 clockDriftPpm=None, shutdownTailLoss='UNKNOWN'),
                metricClassification=dict(submission='DIAGNOSTIC ONLY; independent event completeness and overhead required',
                                          display='DIAGNOSTIC ONLY', gpu='DIAGNOSTIC ONLY',
                                          physicalDisplay='UNAVAILABLE'),
                captureCompleteness='offline consistency only; row completeness requires independent evidence',
                independentCreationIdentityMatched=identity is not None,
                fullMeasuredRunQuality='INCOMPLETE',
                warnings=['short-only; no performance conclusion', 'Other runtime is not API-entry instrumentation',
                          'display estimates are not physical scanout', 'observer interference needs independent review'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--loss', type=Path)
    parser.add_argument('--identity', type=Path, help='Separate retained-handle or stimulus PID/creation identity record')
    args = parser.parse_args()
    try:
        snapshots = short.load(args.loss) if args.loss else None
        identity = short.load(args.identity) if args.identity else None
        result = inspect(args.manifest.parent, short.load(args.manifest), snapshots, identity=identity)
    except (ValueError, OSError, KeyError, TypeError, OverflowError):
        result = dict(artifactKind='short-submission-fidelity', quality='INVALID', reason='fidelity-validation-failed')
    print(json.dumps(result, indent=2))
    return 0 if result['quality'] == 'VALID WITH WARNINGS' else 2


if __name__ == '__main__': raise SystemExit(main())
