"""Opt-in bounded native WGL stimulus, never a Minecraft/game launcher.

Creates one controlled window and records independent pre/post SwapBuffers QPC.
No global timer, priority, power, display or input changes. A gate file lets the
operator start the pinned capture first. All output uses a fresh exclusive root.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import time, os, json, csv
import re
from pathlib import Path
from datetime import datetime, timezone
from process_capture import Reader
from readiness_checks import plain, read_json


def validate_plan(plan):
    if not isinstance(plan, list) or not plan or len(plan) > 8:
        raise ValueError('phase plan')
    total = 0
    for phase in plan:
        if not isinstance(phase, dict) or set(phase) != {'label', 'seconds', 'hz'} or not isinstance(phase['label'], str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,80}', phase['label']):
            raise ValueError('phase fields')
        if type(phase['seconds']) not in (int, float) or not 1 <= phase['seconds'] <= 30:
            raise ValueError('phase duration')
        if type(phase['hz']) is not int or not 1 <= phase['hz'] <= 240:
            raise ValueError('phase cadence')
        total += phase['seconds']
    if total > 90 or len({p['label'] for p in plan}) != len(plan):
        raise ValueError('plan bound or duplicate labels')
    return total


def run(out, plan):
    duration = validate_plan(plan)
    out = Path(out).absolute()
    plain(out.parent)
    out.mkdir(exist_ok=False)
    u=c.WinDLL('user32',use_last_error=True);g=c.WinDLL('gdi32');gl=c.WinDLL('opengl32');k=c.WinDLL('kernel32')
    LRESULT=c.c_ssize_t;WNDPROC=c.WINFUNCTYPE(LRESULT,w.HWND,w.UINT,w.WPARAM,w.LPARAM)
    class WC(c.Structure):
     _fields_=[('style',w.UINT),('proc',WNDPROC),('cbClsExtra',c.c_int),('cbWndExtra',c.c_int),('hInstance',w.HINSTANCE),('hIcon',w.HICON),('hCursor',w.HANDLE),('hbrBackground',w.HBRUSH),('menu',w.LPCWSTR),('name',w.LPCWSTR)]
    class PFD(c.Structure):
     _fields_=[('size',w.WORD),('version',w.WORD),('flags',w.DWORD),('pixelType',c.c_byte),('color',c.c_byte),('red',c.c_byte),('redShift',c.c_byte),('green',c.c_byte),('greenShift',c.c_byte),('blue',c.c_byte),('blueShift',c.c_byte),('alpha',c.c_byte),('alphaShift',c.c_byte),('accum',c.c_byte),('accumRed',c.c_byte),('accumGreen',c.c_byte),('accumBlue',c.c_byte),('accumAlpha',c.c_byte),('depth',c.c_byte),('stencil',c.c_byte),('aux',c.c_byte),('layer',c.c_byte),('reserved',c.c_byte),('layerMask',w.DWORD),('visibleMask',w.DWORD),('damageMask',w.DWORD)]
    u.DefWindowProcW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM];u.DefWindowProcW.restype=LRESULT
    u.CreateWindowExW.argtypes=[w.DWORD,w.LPCWSTR,w.LPCWSTR,w.DWORD,c.c_int,c.c_int,c.c_int,c.c_int,w.HWND,w.HMENU,w.HINSTANCE,c.c_void_p];u.CreateWindowExW.restype=w.HWND
    u.GetDC.argtypes=[w.HWND];u.GetDC.restype=w.HDC
    k.GetModuleHandleW.argtypes=[w.LPCWSTR];k.GetModuleHandleW.restype=w.HMODULE
    g.ChoosePixelFormat.argtypes=[w.HDC,c.POINTER(PFD)];g.SetPixelFormat.argtypes=[w.HDC,c.c_int,c.POINTER(PFD)];g.SwapBuffers.argtypes=[w.HDC]
    gl.wglCreateContext.argtypes=[w.HDC];gl.wglCreateContext.restype=w.HANDLE
    gl.wglMakeCurrent.argtypes=[w.HDC,w.HANDLE];gl.wglDeleteContext.argtypes=[w.HANDLE]
    gl.glClearColor.argtypes=[c.c_float]*4;gl.glClear.argtypes=[w.UINT]
    gl.wglGetProcAddress.argtypes=[c.c_char_p];gl.wglGetProcAddress.restype=c.c_void_p
    u.DestroyWindow.argtypes=[w.HWND];u.ReleaseDC.argtypes=[w.HWND,w.HDC]
    u.ShowWindow.argtypes=[w.HWND,c.c_int]
    u.PeekMessageW.argtypes=[c.POINTER(w.MSG),w.HWND,w.UINT,w.UINT,w.UINT]
    u.TranslateMessage.argtypes=[c.POINTER(w.MSG)]
    u.DispatchMessageW.argtypes=[c.POINTER(w.MSG)];u.DispatchMessageW.restype=LRESULT
    done=False
    @WNDPROC
    def proc(hwnd,msg,wp,lp):
     nonlocal done
     if msg==0x10: done=True;return 0
     return u.DefWindowProcW(hwnd,msg,wp,lp)
    instance=k.GetModuleHandleW(None);name='AuroraP3A3ControlledGL'
    wc=WC(0x20,proc,0,0,instance,None,None,None,None,name)
    assert u.RegisterClassW(c.byref(wc))
    hwnd=u.CreateWindowExW(0,name,'Aurora P3-A.3 controlled OpenGL calibration',0x10cf0000,100,100,960,600,None,None,instance,None);assert hwnd
    dc=u.GetDC(hwnd);pfd=PFD();pfd.size=c.sizeof(PFD);pfd.version=1;pfd.flags=0x25;pfd.color=32;pfd.depth=24
    pf=g.ChoosePixelFormat(dc,c.byref(pfd));assert pf and g.SetPixelFormat(dc,pf,c.byref(pfd))
    rc=gl.wglCreateContext(dc);assert rc and gl.wglMakeCurrent(dc,rc)
    swapinterval=gl.wglGetProcAddress(b'wglSwapIntervalEXT')
    if swapinterval: c.WINFUNCTYPE(c.c_int,c.c_int)(swapinterval)(0)
    reader=Reader(os.getpid());identity=reader.creation_id;reader.close()
    freq=c.c_longlong();k.QueryPerformanceFrequency(c.byref(freq))
    
    meta={'pid':os.getpid(),'creationId':identity,'qpcFrequencyHz':freq.value,'phases':plan,'durationSeconds':duration,'swapIntervalRequested':0,'source':'real native WGL SwapBuffers controlled stimulus'}
    (out/'identity.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')
    print('Controlled native OpenGL ready',os.getpid(),flush=True)
    msg=w.MSG();rows=[];deadline=time.perf_counter()+60
    while not (out/'start.gate').exists() and time.perf_counter()<deadline and not done:
     while u.PeekMessageW(c.byref(msg),hwnd,0,0,1):u.TranslateMessage(c.byref(msg));u.DispatchMessageW(c.byref(msg))
     time.sleep(.01)
    assert (out/'start.gate').exists() and not done,'No start gate'
    start=time.perf_counter();origin=c.c_longlong();k.QueryPerformanceCounter(c.byref(origin));meta.update(startUtc=datetime.now(timezone.utc).isoformat(),originQpc=origin.value)
    (out/'started.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')
    nextframe=start;phase_index=0;phase_end=plan[0]['seconds']
    while not done and time.perf_counter()-start<duration:
     while u.PeekMessageW(c.byref(msg),hwnd,0,0,1):u.TranslateMessage(c.byref(msg));u.DispatchMessageW(c.byref(msg))
     now=time.perf_counter();elapsed=now-start
     if elapsed>=duration:break
     while elapsed>=phase_end and phase_index<len(plan)-1:
      phase_index+=1;phase_end+=plan[phase_index]['seconds'];nextframe=now
     phase=plan[phase_index]
     # Deadlines advance from the prior deadline; work time is not added to every period.
     # Missed deadlines rebase without a burst of catch-up swaps. Actual timing is retained.
     delay=nextframe-time.perf_counter()
     if delay>0:time.sleep(delay)
     gl.glClearColor((len(rows)%60)/60,.2,.6,1);gl.glClear(0x4000)
     before=c.c_longlong();after=c.c_longlong();k.QueryPerformanceCounter(c.byref(before));assert g.SwapBuffers(dc);k.QueryPerformanceCounter(c.byref(after))
     rows.append([before.value,after.value,phase['label'],phase['hz']])
     nextframe+=1/phase['hz']
     if nextframe<time.perf_counter():nextframe=time.perf_counter()
    gl.wglMakeCurrent(None,None);gl.wglDeleteContext(rc);u.ReleaseDC(hwnd,dc);u.DestroyWindow(hwnd)
    with (out/'swaps.csv').open('x',newline='',encoding='utf-8') as f:
     writer=csv.writer(f);writer.writerow(['beforeQpc','afterQpc','phase','requestedHz']);writer.writerows(rows)
    meta.update(endUtc=datetime.now(timezone.utc).isoformat(),swaps=len(rows),status='cancelled' if done else 'complete')
    (out/'result.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output',type=Path)
    parser.add_argument('plan',type=Path)
    args=parser.parse_args()
    run(args.output,read_json(args.plan.absolute()))
