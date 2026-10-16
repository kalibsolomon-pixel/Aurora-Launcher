"""Bounded, read-only Windows telemetry for one explicitly authorized PID/creation ID.

No process-name lookup, command-line reading, Java attachment or termination.
Test with this script's own short-lived child before any authorized game capture.
"""
import argparse
import csv
import ctypes
from ctypes import wintypes as w
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import time


class FILETIME(ctypes.Structure):
    _fields_=[('low',w.DWORD),('high',w.DWORD)]


class MEMORY(ctypes.Structure):
    _fields_=[('length',w.DWORD),('load',w.DWORD)]+[(name,ctypes.c_ulonglong) for name in
              ('total','available','page_total','page_available','virtual_total','virtual_available','extended')]


class COUNTERS(ctypes.Structure):
    _fields_=[('cb',w.DWORD),('faults',w.DWORD)]+[(name,ctypes.c_size_t) for name in
              ('peak_rss','rss','paged_peak','paged','nonpaged_peak','nonpaged','pagefile','pagefile_peak')]


def ticks(value):
    return (value.high<<32)|value.low


class Reader:
    def __init__(self,pid):
        if os.name!='nt': raise ValueError('Windows adapter only; Linux capture remains untested')
        if os.cpu_count() is None or os.cpu_count()>64:
            raise ValueError('Windows processor-group accounting needs a separately validated adapter')
        self.kernel=ctypes.WinDLL('kernel32',use_last_error=True)
        self.psapi=ctypes.WinDLL('psapi',use_last_error=True)
        self.kernel.OpenProcess.argtypes=[w.DWORD,w.BOOL,w.DWORD]
        self.kernel.OpenProcess.restype=w.HANDLE
        self.kernel.GetProcessTimes.argtypes=[w.HANDLE]+[ctypes.POINTER(FILETIME)]*4
        self.kernel.GetSystemTimes.argtypes=[ctypes.POINTER(FILETIME)]*3
        self.kernel.GlobalMemoryStatusEx.argtypes=[ctypes.POINTER(MEMORY)]
        self.kernel.CloseHandle.argtypes=[w.HANDLE]
        self.kernel.GetExitCodeProcess.argtypes=[w.HANDLE,ctypes.POINTER(w.DWORD)]
        self.psapi.GetProcessMemoryInfo.argtypes=[w.HANDLE,ctypes.POINTER(COUNTERS),w.DWORD]
        self.handle=self.kernel.OpenProcess(0x0400|0x0010,False,pid) # QUERY_INFORMATION | VM_READ
        if not self.handle: raise OSError('cannot open the authorized process read-only')
        self.pid=pid
        try: self.creation_id=self.sample()['creation_id']
        except Exception:
            self.close();raise

    def close(self):
        if self.handle:
            self.kernel.CloseHandle(self.handle); self.handle=None

    def sample(self):
        created,exited,kernel,user=(FILETIME() for _ in range(4))
        idle,system_kernel,system_user=(FILETIME() for _ in range(3))
        memory=MEMORY(); memory.length=ctypes.sizeof(memory)
        counters=COUNTERS(); counters.cb=ctypes.sizeof(counters)
        status=w.DWORD()
        calls=[self.kernel.GetProcessTimes(self.handle,ctypes.byref(created),ctypes.byref(exited),ctypes.byref(kernel),ctypes.byref(user)),
               self.kernel.GetSystemTimes(ctypes.byref(idle),ctypes.byref(system_kernel),ctypes.byref(system_user)),
               self.kernel.GlobalMemoryStatusEx(ctypes.byref(memory)),
               self.psapi.GetProcessMemoryInfo(self.handle,ctypes.byref(counters),counters.cb),
               self.kernel.GetExitCodeProcess(self.handle,ctypes.byref(status))]
        if not all(calls): raise OSError('telemetry read failed')
        if status.value!=259: raise OSError('authorized process exited; capture is incomplete')
        return {'monotonic_ns':time.perf_counter_ns(),'pid':self.pid,'creation_id':str(ticks(created)),
                'cpu_time_ns':100*(ticks(kernel)+ticks(user)),'rss_bytes':counters.rss,
                'peak_rss_bytes':counters.peak_rss,'system_available_bytes':memory.available,
                'system_total_bytes':memory.total,'system_busy_ns':100*(ticks(system_kernel)+ticks(system_user)-ticks(idle)),
                'system_total_ns':100*(ticks(system_kernel)+ticks(system_user))}


def capture(pid,creation_id,seconds,interval,directory):
    if not math.isfinite(seconds) or not math.isfinite(interval) or not 0<seconds<=660 or not .1<=interval<=5:
        raise ValueError('duration/interval outside bounded capture policy')
    # Caller chooses a new directory, never an existing owner or run directory.
    directory.mkdir(parents=False,exist_ok=False)
    reader=Reader(pid)
    try:
        if reader.creation_id!=creation_id: raise ValueError('PID creation identity mismatch')
        first=reader.sample(); origin=first['monotonic_ns']; start_utc=datetime.now(timezone.utc).isoformat()
        rows=[first]; overhead=[]
        status='complete'
        with (directory/'process.csv').open('x',newline='') as f:
            writer=csv.DictWriter(f,fieldnames=list(first));writer.writeheader();writer.writerow(first);f.flush()
            try:
                while (time.perf_counter_ns()-origin)/1e9<seconds:
                    target=min(seconds,len(rows)*interval)
                    time.sleep(max(0,target-(time.perf_counter_ns()-origin)/1e9))
                    before=time.perf_counter_ns();row=reader.sample();overhead.append(time.perf_counter_ns()-before)
                    if row['creation_id']!=creation_id: raise ValueError('process identity changed')
                    rows.append(row);writer.writerow(row);f.flush()
            except (OSError,ValueError,KeyboardInterrupt):
                status='incomplete'
        metadata={'schemaVersion':1,'sampleKind':'process-plumbing','pid':pid,'creationId':creation_id,
                  'originMonotonicNs':origin,'logicalProcessors':os.cpu_count(),'startUtc':start_utc,
                  'endUtc':datetime.now(timezone.utc).isoformat(),'requestedSeconds':seconds,
                  'elapsedSeconds':(rows[-1]['monotonic_ns']-origin)/1e9,'intervalSeconds':interval,
                  'samples':len(rows),'status':status,'maxReadOverheadNs':max(overhead,default=0),
                  'clock':'Windows QPC via Python perf_counter_ns; sample timestamp taken after bounded OS reads',
                  'unknowns':['Java used/committed heap','GC CPU','paging rate','GPU usage','thermals'],
                  'target':'explicit authorized process; retained read-only handle; no command-line access'}
        (directory/'capture.json').write_text(json.dumps(metadata,indent=2))
        if status!='complete': raise OSError('partial capture preserved')
        return metadata
    finally: reader.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pid',type=int,required=True)
    parser.add_argument('--creation-id',required=True)
    parser.add_argument('--seconds',type=float,default=30)
    parser.add_argument('--interval',type=float,default=.5)
    parser.add_argument('--new-directory',type=Path,required=True)
    args=parser.parse_args()
    try:
        print(json.dumps(capture(args.pid,args.creation_id,args.seconds,args.interval,args.new_directory),indent=2))
        return 0
    except (OSError,ValueError):
        print('Capture failed; preserve partial directory. Check PID identity, permissions and bounds.')
        return 2


if __name__=='__main__': raise SystemExit(main())
