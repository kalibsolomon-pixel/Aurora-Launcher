"""Prepare the fixed six-process Java comparison from preserved Client inputs.
Explicit local research only; never launch, install or modify original evidence.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path


def inventory(root):
    rows = []
    for p in sorted(root.rglob('*')):
        if p.is_file():
            with p.open('rb') as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            rows.append(dict(path=p.relative_to(root).as_posix(), bytes=p.stat().st_size, sha256=digest))
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('client_repo', type=Path)
    parser.add_argument('java_bin', type=Path)
    args = parser.parse_args()
    client = args.client_repo.resolve()
    phase = client / 'run/jvm-initiative'
    assert not phase.exists(), 'Refuse to overwrite prior experiment'
    prior = json.loads((client / 'run/cperf2b-benchmark/inputs.json').read_text())
    source = client / 'run/cperf2b-benchmark/run-5/game'
    assert inventory(source / 'saves/Cperf1a') == prior['sourceWorldInventory']
    assert inventory(source / 'mods') == prior['runs'][4]['mods']
    assert inventory(source / 'config') == prior['runs'][4]['config']
    phase.mkdir()
    events = ['GarbageCollection', 'GCPhasePause', 'GCHeapSummary', 'DataLoss',
              'SafepointBegin', 'SafepointEnd']
    profile = '<?xml version="1.0" encoding="UTF-8"?><configuration version="2.0" label="Initiative GC only">'
    profile += ''.join(f'<event name="jdk.{event}"><setting name="enabled">true</setting></event>' for event in events)
    profile += '</configuration>\n'
    (phase / 'gc-only.jfc').write_text(profile)
    rows = []
    for order, label in enumerate(['A', 'B', 'C', 'C', 'B', 'A'], 1):
        directory = phase / f'run-{order}'
        shutil.copytree(source, directory / 'game')
        options = directory / 'game/options.txt'
        options.write_text(options.read_text().replace('onboardAccessibility:false', 'onboardAccessibility:true'))
        heap = 1536 if label == 'B' else 2048
        flags = ['-XX:+UseZGC', '-XX:+ZGenerational'] if label == 'C' else []
        probe = subprocess.run([str(args.java_bin / 'java.exe'), f'-Xmx{heap}m', *flags,
                                '-XX:+PrintFlagsFinal', '-version'], capture_output=True, text=True, timeout=15)
        assert probe.returncode == 0, 'Exact runtime rejects configuration'
        selected = [line.strip() for line in probe.stdout.splitlines()
                    if any((' ' + key + ' ') in line for key in
                           ['InitialHeapSize', 'MaxHeapSize', 'UseG1GC', 'UseZGC', 'ZGenerational'])]
        (directory / 'vm-preflight.json').write_text(json.dumps(selected, indent=2))
        rows.append(dict(order=order, label=label, maxHeapMiB=heap, flags=flags,
                         files=inventory(directory / 'game')))
    java = (args.java_bin / 'java.exe').resolve().as_posix()
    init = f'''gradle.projectsEvaluated {{
    def p = gradle.rootProject
    def ordinal = p.findProperty('jvmInitiativeRun')?.toString()
    assert ordinal in ['1','2','3','4','5','6']
    def heaps = [2048,1536,2048,2048,1536,2048]
    def root = p.file('run/jvm-initiative').absolutePath.replace('\\\\','/')
    p.loom.runs.client.runDir('run/jvm-initiative/run-' + ordinal + '/game')
    p.tasks.named('runClient') {{
        executable = '{java}'
        maxHeapSize = heaps[ordinal.toInteger()-1].toString() + 'm'
        minHeapSize = null
        if (ordinal in ['3','4']) jvmArgs '-XX:+UseZGC', '-XX:+ZGenerational'
        jvmArgs '-XX:StartFlightRecording=settings=' + root + '/gc-only.jfc,filename=' + root + '/run-' + ordinal + '/gc.jfr,disk=true,dumponexit=true,maxsize=16m,duration=600s'
        args '--username','JvmInitiative','--quickPlaySingleplayer','Cperf1a','--quickPlayPath','logs/initiative-quickplay.json'
    }}
}}
'''
    (phase / 'manual.init.gradle').write_text(init)
    (phase / 'inputs.json').write_text(json.dumps(dict(worldIdentity=prior['worldIdentity'],
        sourcePose=prior['sourcePose'], javaExecutable=java, runs=rows), indent=2))
    print(json.dumps(dict(runs=len(rows), root=str(phase))))


if __name__ == '__main__':
    main()
