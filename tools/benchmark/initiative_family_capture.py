"""Explicit Windows research capture of an authorized process family.

No process arguments, control, termination, secrets or production integration.
Retains exact read-only handles; membership churn is reported as partial.
"""
import argparse
import csv
import datetime as dt
import json
import os
import subprocess
import time
from pathlib import Path

from process_capture import Reader


def family(root_pid):
    command = ('Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId '
               '| ConvertTo-Json -Compress')
    result = subprocess.run(['powershell.exe', '-NoProfile', '-Command', command],
                            check=True, capture_output=True, text=True, timeout=15)
    rows = json.loads(result.stdout)
    if isinstance(rows, dict):
        rows = [rows]
    members = {root_pid}
    while True:
        expanded = members | {r['ProcessId'] for r in rows if r['ParentProcessId'] in members}
        if expanded == members:
            return sorted(members)
        members = expanded


def capture(root_pid, root_creation, seconds, directory):
    assert 0 < seconds <= 120 and seconds * 2 == int(seconds * 2), 'Use whole half-second windows'
    directory.mkdir(parents=False, exist_ok=False)
    members = family(root_pid)
    readers = []
    rows = []
    errors = []
    start_utc = dt.datetime.now(dt.timezone.utc).isoformat()
    try:
        for pid in members:
            reader = Reader(pid)
            readers.append(reader)
            if pid == root_pid:
                assert reader.creation_id == root_creation, 'Root creation identity mismatch'
        origin = time.perf_counter_ns()
        overhead = []
        with (directory / 'family.csv').open('x', newline='') as stream:
            writer = None
            for index in range(int(seconds * 2) + 1):
                time.sleep(max(0, index * .5 - (time.perf_counter_ns() - origin) / 1e9))
                before = time.perf_counter_ns()
                for reader in readers:
                    try:
                        row = dict(sample_index=index, **reader.sample())
                        assert row['creation_id'] == reader.creation_id
                        if writer is None:
                            writer = csv.DictWriter(stream, fieldnames=list(row))
                            writer.writeheader()
                        writer.writerow(row)
                        rows.append(row)
                    except (OSError, AssertionError):
                        errors.append(dict(sample=index, pid=reader.pid, reason='Exact process unavailable'))
                stream.flush()
                overhead.append(time.perf_counter_ns() - before)
        end_utc = dt.datetime.now(dt.timezone.utc).isoformat()
        ending = family(root_pid)
        complete = not errors and ending == members
        groups = {reader.pid: [r for r in rows if r['pid'] == reader.pid] for reader in readers}
        cpu = sum((r[-1]['cpu_time_ns'] - r[0]['cpu_time_ns']) / 1e9 for r in groups.values() if len(r) > 1)
        wall = (max(r['monotonic_ns'] for r in rows) - min(r['monotonic_ns'] for r in rows)) / 1e9
        rss = [sum(r['rss_bytes'] for r in rows if r['sample_index'] == i) / 1024**2
               for i in range(int(seconds * 2) + 1)]
        first, last = rows[0], rows[-1]
        result = dict(status='complete' if complete else 'partial', rootPid=root_pid,
                      startUtc=start_utc, endUtc=end_utc, seconds=wall, samplesPerProcess={str(k): len(v) for k, v in groups.items()},
                      identities=[dict(pid=r.pid, creationId=r.creation_id) for r in readers],
                      membersStart=members, membersEnd=ending, errors=errors,
                      processCpuSeconds=cpu, processCpuPercentOneCore=cpu / wall * 100,
                      processCpuPercentMachine=cpu / wall * 100 / os.cpu_count(),
                      summedRssMiBMin=min(rss), summedRssMiBMax=max(rss),
                      availableMiBMin=min(r['system_available_bytes'] for r in rows) / 1024**2,
                      systemBusyPercent=(last['system_busy_ns'] - first['system_busy_ns']) /
                      (last['system_total_ns'] - first['system_total_ns']) * 100,
                      maxFamilyReadOverheadNs=max(overhead),
                      limitations=['Summed RSS double-counts shared pages', 'Not Java heap or unique physical memory',
                                   'Family snapshots at endpoints may miss transient children',
                                   'Whole process CPU is not active render/GC CPU', 'Observer overhead uncalibrated'])
        (directory / 'capture.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2))
    finally:
        for reader in readers:
            reader.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root-pid', type=int, required=True)
    parser.add_argument('--creation-id', required=True)
    parser.add_argument('--seconds', type=float, default=60)
    parser.add_argument('--new-directory', type=Path, required=True)
    args = parser.parse_args()
    capture(args.root_pid, args.creation_id, args.seconds, args.new_directory)
