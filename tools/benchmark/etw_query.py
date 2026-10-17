"""Query-only ETW loss statistics for this phase's uniquely named sessions.

There is deliberately no start, stop, flush, update or privilege operation.
Snapshots cover events up to their query time, never an unobserved shutdown tail.
"""
import ctypes as c
from ctypes import wintypes as w
import re
import time


def query(name):
    if not isinstance(name, str) or not re.fullmatch(r'Aurora-P3A3-[A-Za-z0-9-]{1,100}', name):
        raise ValueError('not a phase-owned trace name')

    class Wnode(c.Structure):
        _fields_ = [('BufferSize', w.ULONG), ('ProviderId', w.ULONG),
                    ('HistoricalContext', c.c_ulonglong), ('TimeStamp', c.c_longlong),
                    ('Guid', c.c_byte * 16), ('ClientContext', w.ULONG), ('Flags', w.ULONG)]

    class Properties(c.Structure):
        _fields_ = [('Wnode', Wnode)] + [(key, w.ULONG) for key in
                   ('BufferSize', 'MinimumBuffers', 'MaximumBuffers', 'MaximumFileSize',
                    'LogFileMode', 'FlushTimer', 'EnableFlags', 'AgeLimit', 'NumberOfBuffers',
                    'FreeBuffers', 'EventsLost', 'BuffersWritten', 'LogBuffersLost',
                    'RealTimeBuffersLost')] + [('LoggerThreadId', w.HANDLE),
                                              ('LogFileNameOffset', w.ULONG), ('LoggerNameOffset', w.ULONG)]

    buffer = c.create_string_buffer(c.sizeof(Properties) + 8192)
    properties = Properties.from_buffer(buffer)
    properties.Wnode.BufferSize = len(buffer)
    properties.LoggerNameOffset = c.sizeof(Properties)
    properties.LogFileNameOffset = c.sizeof(Properties) + 4096
    control = c.WinDLL('advapi32', use_last_error=True).ControlTraceW
    control.argtypes = [c.c_ulonglong, w.LPCWSTR, c.POINTER(Properties), w.ULONG]
    control.restype = w.ULONG
    before = time.perf_counter_ns()
    # EVENT_TRACE_CONTROL_QUERY = 0. Never pass another control code.
    status = control(0, name, c.byref(properties), 0)
    return {'statusCode': status, 'qpcNs': before, 'queryDurationNs': time.perf_counter_ns() - before,
            'lossStatus': 'COUNTER SNAPSHOT' if status == 0 else 'UNKNOWN',
            'eventsLost': properties.EventsLost if status == 0 else None,
            'logBuffersLost': properties.LogBuffersLost if status == 0 else None,
            'realTimeBuffersLost': properties.RealTimeBuffersLost if status == 0 else None,
            'buffersWritten': properties.BuffersWritten if status == 0 else None}
