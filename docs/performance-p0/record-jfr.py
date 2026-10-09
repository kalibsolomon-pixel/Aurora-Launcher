"""Bounded existing-child sampling; no environment/arguments/files/sockets events."""
import json, pathlib, re, subprocess, sys
target = int(sys.argv[1])
base = pathlib.Path(__file__).resolve().parent/'evidence'
local = pathlib.Path((base/'small-root.local.txt').read_text())
recording = local/'allowlisted-steady-state.jfr'
jdk = pathlib.Path(r'C:\Program Files\Eclipse Adoptium\jdk-25.0.4.101-hotspot\bin')
args=[str(jdk/'jcmd.exe'),str(target),'JFR.start','name=AuroraP0Bounded',
      'settings=none','duration=45s','disk=true','maxsize=32m','filename='+str(recording),
      '+jdk.ExecutionSample#enabled=true','+jdk.ExecutionSample#period=20ms',
      '+jdk.ObjectAllocationSample#enabled=true','+jdk.ObjectAllocationSample#throttle=50/s',
      '+jdk.GarbageCollection#enabled=true','+jdk.GCPhasePause#enabled=true']
# settings=none does not guarantee application-defined events are disabled.
# Explicitly exclude every registered custom type found by the local event audit.
audit = base/'jfr-event-summary.txt'
assert audit.is_file(), 'audit custom event types locally before attachment'
custom = sorted(set(re.findall(r'^\s*((?!jdk\.)[\w.]+)\s+\d+\s+\d+\s*$',
                              audit.read_text(), re.MULTILINE)))
assert 'minecraft.NetworkSummary' in custom
args += ['+'+name+'#enabled=false' for name in custom]
p=subprocess.run(args,capture_output=True,text=True,timeout=25)
# No raw diagnostic output is persisted or printed; JFR.start can mention local paths.
success=p.returncode==0 and 'Started recording' in p.stdout
(base/'jfr-start.json').write_text(json.dumps({'started':success,'exitCode':p.returncode,
    'durationSeconds':45,'toolMajor':25,'settings':'none','enabledEvents':[
        'jdk.ExecutionSample','jdk.ObjectAllocationSample','jdk.GarbageCollection','jdk.GCPhasePause'],
    'disabledCustomEvents':custom,
    'rawRecording':'<disposable-small-root>/allowlisted-steady-state.jfr'}),encoding='utf-8')
print(json.dumps({'jfrStarted':success,'exitCode':p.returncode}))
