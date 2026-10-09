"""Behavioral gate: overlay, two rendered frames, entrypoints, JFR privacy."""
import json
from pathlib import Path
import re
import subprocess
import sys

root = Path(sys.argv[1]).resolve(strict=True)
trial = root / 'trials' / (sys.argv[2] if len(sys.argv)>2 else 'agent-fixture')
trial.mkdir(parents=True, exist_ok=False)
sources = {
    'com/aurora/client/screen/AuroraTitleScreen.java': 'package com.aurora.client.screen; public class AuroraTitleScreen {}',
    'net/minecraft/class_310.java': '''package net.minecraft;
public class class_310 { public Object field_1755, field_18175; public void method_1523(boolean tick) {} }''',
    'ProbeMain.java': '''import java.nio.file.*;
public class ProbeMain {
 @jdk.jfr.Name("fixture.SecretEvent") @jdk.jfr.Enabled(true)
 public static class SecretEvent extends jdk.jfr.Event { public String secret = "P3-FAKE-SECRET"; }
 public void onInitializeClient() {}
 public static void main(String[] args) throws Exception {
  new ProbeMain().onInitializeClient();
  new SecretEvent().commit();
  var game = new net.minecraft.class_310();
  game.field_1755 = new com.aurora.client.screen.AuroraTitleScreen();
  game.field_18175 = new Object(); game.method_1523(true); game.method_1523(true);
  Path marker = Path.of(args[0], "markers-menu.json");
  if (Files.exists(marker)) throw new AssertionError("overlay counted as usable");
  game.field_18175 = null; game.method_1523(true);
  if (Files.exists(marker)) throw new AssertionError("one frame counted as two");
  game.method_1523(true);
  if (!Files.exists(marker)) throw new AssertionError("menu not recorded");
 }
}'''}
for name, content in sources.items():
    p = trial / name; p.parent.mkdir(parents=True, exist_ok=True); p.write_text(content)
subprocess.run(['javac', '--release', '21', '-d', str(trial), *[str(trial / p) for p in sources]], check=True)
java = next((root / 'managed/runtimes').glob('java-runtime-delta/*/bin/java.exe'))
asm = next((root / 'managed/instances').glob('*/game/libraries/org/ow2/asm/asm/*/*.jar'))
recording = trial / 'fixture.jfr'
result = subprocess.run([str(java), '-XX:StartFlightRecording=settings=' + str(root / 'startup.jfc') + ',filename=' + str(recording) + ',dumponexit=true',
    '-javaagent:' + str(root / 'agent.jar') + '=' + str(trial), '-cp', str(trial) + ';' + str(asm), 'ProbeMain', str(trial)], capture_output=True, timeout=30)
(trial / 'process-output-private.txt').write_bytes(result.stdout + result.stderr)
assert result.returncode == 0, 'agent fixture failed; inspect private fixture output'
markers = json.loads((trial / 'markers-exit.json').read_text())
assert sum(r['boundary'] == 'menu.usable' for r in markers['records']) == 1
assert markers['totals']['entrypoint.ProbeMain.onInitializeClient']['calls'] == 1
summary = subprocess.run(['jfr', 'summary', str(recording)], capture_output=True, text=True, check=True)
counts = {name:int(count) for name,count in re.findall(r'^\s*([\w.]+)\s+(\d+)\s+\d+\s*$', summary.stdout, re.M)}
denied = ['fixture.SecretEvent', 'jdk.JVMInformation', 'jdk.InitialSystemProperty',
          'jdk.InitialEnvironmentVariable', 'jdk.ProcessStart', 'jdk.SocketRead', 'jdk.SocketWrite']
assert not any(counts.get(name, 0) for name in denied)
assert counts.get('aurora.p3.Phase', 0) > 0
receipt = dict(exitCode=0, overlayRejected=True, firstFrameRejected=True, secondFrameAccepted=True,
    entrypointMeasured=True, deniedEventCounts={n:counts.get(n,0) for n in denied}, recordingValid=True)
(root / 'agent-tests.json').write_text(json.dumps(receipt, indent=2))
print(json.dumps(receipt))
