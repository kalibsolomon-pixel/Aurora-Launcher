"""Independent marker reconciliation for bounded calibration, not measured runs."""
from bisect import bisect_left
import statistics

from readiness_checks import require


def loss_state(snapshots, window_end_ns):
    """A zero counter snapshot covers only its observed time, never shutdown."""
    require(type(window_end_ns) is int and window_end_ns >= 0, 'window endpoint')
    if not snapshots or any(s.get('statusCode') != 0 for s in snapshots):
        return 'UNKNOWN'
    for s in snapshots:
        for key in ('eventsLost', 'logBuffersLost', 'realTimeBuffersLost'):
            require(type(s.get(key)) is int and s[key] >= 0, 'invalid loss counter')
    if any(s[k] for s in snapshots for k in ('eventsLost', 'logBuffersLost', 'realTimeBuffersLost')):
        return 'CONFIRMED LOSS'
    if max(s['qpcNs'] for s in snapshots) < window_end_ns:
        return 'UNKNOWN'
    return 'ZERO OBSERVED THROUGH WINDOW'


def reconcile(rows, markers, manifest, stimulus):
    require(manifest['pid'] == stimulus['pid'] and manifest['creationId'] == stimulus['creationId'], 'stimulus process identity mismatch')
    frequency = manifest['qpcFrequencyHz']
    require(type(frequency) is int and frequency == stimulus['qpcFrequencyHz'] > 0, 'QPC frequency mismatch')
    require(manifest['originQpc'] == stimulus['originQpc'], 'QPC origin mismatch')
    require(stimulus['status'] == 'complete', 'incomplete stimulus')
    origin = manifest['originQpc']
    start = origin + manifest['windowStartSeconds'] * frequency
    end = start + manifest['windowSeconds'] * frequency
    before = [int(m['beforeQpc']) for m in markers]
    after = [int(m['afterQpc']) for m in markers]
    require(before and all(a >= b for a, b in zip(after, before))
            and all(b > a for a, b in zip(before, before[1:])), 'marker order')
    observed_span = (before[-1] - before[0]) / frequency
    require(abs(observed_span - stimulus['durationSeconds']) < .25, 'QPC units or stimulus duration mismatch')
    ticks = [int(r['TimeInQPC']) for r in rows if start <= int(r['TimeInQPC']) <= end]
    require(ticks and all(r['ProcessID'] == str(manifest['pid']) for r in rows), 'presentation identity')
    expected = {i for i, tick in enumerate(before) if start <= tick <= end}
    used, errors, bracketed, duplicates, matched, signed = set(), [], 0, 0, [], []
    for tick in ticks:
        index = bisect_left(before, tick)
        candidates = [i for i in (index - 1, index) if 0 <= i < len(before)]
        nearest = min(candidates, key=lambda i: abs(before[i] - tick))
        error = abs(before[nearest] - tick) / frequency * 1000
        # Retain discrepancies instead of silently dropping problematic rows.
        errors.append(error)
        signed.append((tick - before[nearest]) / frequency * 1000)
        matched.append((nearest, tick))
        bracketed += before[nearest] <= tick <= after[nearest]
        duplicates += nearest in used
        used.add(nearest)
    intervals = [(b - a) / frequency * 1000 for a, b in zip(before, before[1:])
                 if start <= a and b <= end]
    require(intervals, 'no independent inner intervals')
    unmatched = expected - used
    missing = len(unmatched)
    boundary_only = all(min(abs(before[i] - start), abs(before[i] - end)) / frequency * 1000 <= 5 for i in unmatched)
    agreement = [abs((b[1] - a[1]) - (before[b[0]] - before[a[0]])) / frequency * 1000
                 for a, b in zip(matched, matched[1:]) if b[0] == a[0] + 1]
    require(agreement, 'no matched interval pairs')
    return {'presentRowsInWindow': len(ticks), 'independentMarkersInWindow': len(expected),
            'unmatchedMarkers': missing, 'duplicateNearestMarkers': duplicates,
            'meanPresentToPreSwapMs': statistics.mean(errors), 'maxPresentToPreSwapMs': max(errors),
            'meanSignedPresentMinusPreSwapMs': statistics.mean(signed),
            'meanIntervalDisagreementMs': statistics.mean(agreement), 'maxIntervalDisagreementMs': max(agreement),
            'unmatchedOnlyAtBoundary': boundary_only,
            'presentInsideSwapBracket': bracketed,
            'independentMeanIntervalMs': statistics.mean(intervals),
            'independentRateHz': 1000 / statistics.mean(intervals),
            'markerConfidence': 'RECONCILED' if missing <= 1 and boundary_only and duplicates == 0 and max(errors) <= 5 else 'REVIEW REQUIRED',
            'boundaryAllowance': 'at most one marker on the exact inner-window edge',
            'scope': 'call/row reconciliation; unavailable display estimates and physical scanout are separate'}
