"""Bounded PresentMon 2.3.1 default-QPC inspection, never a measured benchmark.

The default CSV uses TimeInQPC/MsUntilDisplayed, unlike explicit --v2_metrics.
See pinned PresentMon/CsvOutput.cpp and OutputThread.cpp at tag v2.3.1.
"""
import argparse
from datetime import datetime
import json
from pathlib import Path
import re

import quality as q
from readiness_checks import plain as no_links

FORMAT = 'presentmon-2.3.1-default-present-qpc'
FIELDS = {'schemaVersion', 'artifactKind', 'targetKind', 'status', 'startUtc',
          'endUtc', 'pid', 'creationId', 'executableSha256', 'toolSha256',
          'format', 'qpcFrequencyHz', 'originQpc', 'swapChain', 'presentRuntime',
          'windowStartSeconds', 'windowSeconds', 'framesSha256', 'diagnosticsSha256'}
TOOL_SHA256 = '364e5d98d4d134bd54dd25c22ed2ca2f4883f8bc3ed6502bee0c151e3436d30c'


def inspect(root, m):
    q.require(isinstance(m, dict) and set(m) == FIELDS, 'short calibration fields')
    q.require(type(m['schemaVersion']) is int and m['schemaVersion'] == 1, 'unsupported short schema')
    q.require(m['artifactKind'] == 'short-calibration', 'not a short calibration')
    q.require(m['targetKind'] in ('controlled-opengl', 'minecraft'), 'unknown target kind')
    q.require(m['status'] in ('complete', 'interrupted', 'failed'), 'unknown capture status')
    q.require(m['format'] == FORMAT and m['toolSha256'] == TOOL_SHA256, 'unpinned tool/format')
    q.integer(m['pid'], True); q.integer(m['qpcFrequencyHz'], True); q.integer(m['originQpc'])
    q.require(isinstance(m['creationId'], str) and re.fullmatch(r'[0-9]{1,20}', m['creationId']), 'invalid creation identity')
    q.require(isinstance(m['swapChain'], str) and re.fullmatch(r'0x[0-9a-fA-F]{1,16}', m['swapChain']), 'invalid stream')
    q.require(m['presentRuntime'] == 'Other', 'not the reviewed OpenGL runtime')
    for key in ('executableSha256', 'framesSha256', 'diagnosticsSha256'):
        q.require(isinstance(m[key], str) and q.HASH.fullmatch(m[key]) and m[key] != '0'*64, 'invalid artifact identity')
    start, end = (datetime.fromisoformat(q.text(m[k]).replace('Z', '+00:00')) for k in ('startUtc', 'endUtc'))
    q.require(start.utcoffset() is not None and end.utcoffset() is not None and 0 < (end-start).total_seconds() <= 90, 'unbounded capture time')
    offset = q.number(m['windowStartSeconds']); duration = q.number(m['windowSeconds'], True)
    q.require(1 <= duration <= 30, 'short window must be 1 to 30 seconds')
    no_links(root)
    frames, diagnostics = root/'frames.csv', root/'presentmon-stderr.txt'
    for path, key in ((frames, 'framesSha256'), (diagnostics, 'diagnosticsSha256')):
        no_links(path)
        q.require(q.sha256(path) == m[key], 'artifact digest mismatch')
    rows = q.read_rows(frames, ['ProcessID', 'SwapChainAddress', 'PresentRuntime',
                               'TimeInQPC', 'MsUntilDisplayed', 'MsBetweenDisplayChange'])
    ticks, displayed, unavailable, unpaired = [], [], 0, 0
    for r in rows:
        q.require(r['ProcessID'] == str(m['pid']) and r['SwapChainAddress'] == m['swapChain']
                  and r['PresentRuntime'] == m['presentRuntime'], 'target/stream mismatch')
        q.require(re.fullmatch(r'[0-9]+', r['TimeInQPC']) is not None, 'present QPC must be integer ticks')
        tick = int(r['TimeInQPC']); ticks.append(tick)
        # NA is an unavailable display estimate, not proof of lost ETW events.
        if r['MsUntilDisplayed'] == 'NA':
            unavailable += 1
            if r['MsBetweenDisplayChange'] != 'NA': q.number(r['MsBetweenDisplayChange'], True)
            continue
        latency = q.number(r['MsUntilDisplayed'], True)
        if r['MsBetweenDisplayChange'] == 'NA': unpaired += 1
        else: q.number(r['MsBetweenDisplayChange'], True)
        displayed.append((tick-m['originQpc'])/m['qpcFrequencyHz'] + latency/1000)
    q.strictly_increasing(ticks)
    metrics = q.frame_metrics(displayed, offset, duration, .25)
    q.require(metrics['count'] >= 10, 'insufficient short calibration samples')
    diagnostic = diagnostics.read_text(encoding='utf-8-sig')
    q.require(not re.search(r'\b(error|failed|denied|lost|dropped)\b', diagnostic, re.I), 'producer diagnostics need review')
    warnings = ['etw-display-estimate-not-physical-display-proof', 'producer-loss-counters-unavailable',
                'observer-overhead-unmeasured', 'short-calibration-not-performance-benchmark']
    if unavailable: warnings.append('unavailable-display-rows-retained')
    if unpaired: warnings.append('unpaired-display-change-rows-retained')
    if 'elevated privilege' in diagnostic: warnings.append('unelevated-process-query-limitation')
    state = 'VALID WITH WARNINGS' if metrics['boundary_coverage'] and m['status'] == 'complete' else 'INCOMPLETE'
    return {'artifactKind': 'short-calibration', 'quality': state, 'targetKind': m['targetKind'],
            'rawRows': len(rows), 'unavailableDisplayRows': unavailable, 'unpairedDisplayRows': unpaired,
            'metrics': metrics, 'warnings': warnings, 'fullMeasuredRunQuality': 'INCOMPLETE'}


def load(path):
    def pairs(items):
        result = {}
        for key, value in items:
            q.require(key not in result, 'duplicate manifest key'); result[key] = value
        return result
    no_links(path)
    q.require(path.stat().st_size <= 16384, 'manifest too large')
    return json.loads(path.read_text(encoding='utf-8-sig'), object_pairs_hook=pairs)


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('manifest', type=Path)
    args = parser.parse_args()
    try: result = inspect(args.manifest.parent, load(args.manifest))
    except (ValueError, OSError, KeyError, TypeError, OverflowError):
        result = {'artifactKind': 'short-calibration', 'quality': 'INVALID', 'reason': 'short-calibration-validation-failed'}
    print(json.dumps(result, indent=2))
    return 0 if result['quality'] == 'VALID WITH WARNINGS' else 2


if __name__ == '__main__': raise SystemExit(main())
