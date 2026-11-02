"""Summarize Chromium trace evidence; never equate rAF callbacks with presented frames."""
import collections
import json
import pathlib
import statistics
import sys

root = pathlib.Path(sys.argv[1])
trials = json.loads((root / sys.argv[2]).read_text())

def summary(values):
    values = sorted(values)
    return {"n": len(values), **{key: round(values[min(len(values)-1, int(len(values)*p))], 3) if values else None
                              for key, p in [("median", .5), ("p95", .95), ("p99", .99)]}}

results = []
for trial in trials:
    events = json.loads((root / f"trace-{trial['surface']}-{trial['variant']}-{trial['run']}.json").read_text())["traceEvents"]
    drawing = [e["dur"]/1000 for e in events if e["name"] == "Display::DrawAndSwap" and e.get("ph") == "X"]
    scroll = []
    janky = []
    for e in events:
        if e["name"] == "InputLatency::GestureScrollUpdate" and e["ph"] == "b":
            components = {c["component_type"]: c["time_us"] for c in e.get("args", {}).get("chrome_latency_info", {}).get("component_info", [])}
            start = components.get("COMPONENT_INPUT_EVENT_LATENCY_ORIGINAL")
            end = components.get("COMPONENT_INPUT_EVENT_LATENCY_FRAME_SWAP")
            if start and end: scroll.append((end-start)/1000)
        if e["name"] == "EventLatency" and e["ph"] == "b":
            latency = e.get("args", {}).get("event_latency", {})
            if "is_janky_scrolled_frame" in latency: janky.append(latency["is_janky_scrolled_frame"])
    results.append({"surface": trial["surface"], "variant": trial["variant"], "run": trial["run"],
                    "callbackIntervalMs": summary(trial["frames"]), "compositorDrawCpuMs": summary(drawing),
                    "inputToFrameSwapMs": summary(scroll), "chromiumJanky": sum(janky), "chromiumScrollFrames": len(janky),
                    "longTasks": trial["longTasks"], "domMutations": trial["mutations"], "domNodes": trial["nodes"],
                    "loadedImages": trial["images"], "viewport": trial["viewport"], "devicePixelRatio": trial["dpr"],
                    "rendererMetrics": trial["metrics"], "videoState": trial["video"]})
output = {"method": "3 repeated trials per variant/surface, interleaved when multiple variants; 60 wheel inputs, 100 illustrated entries; rAF is callback timing, DrawAndSwap is CPU submission work, frame-swap latency excludes final screen presentation", "trials": results}
(root / (sys.argv[3] if len(sys.argv)>3 else 'summary.json')).write_text(json.dumps(output, indent=2))
for surface in dict.fromkeys(r["surface"] for r in results):
    for variant in dict.fromkeys(r["variant"] for r in results):
        rows = [r for r in results if r["surface"]==surface and r["variant"]==variant]
        if rows: print(surface, variant, 'draw medians', [r['compositorDrawCpuMs']['median'] for r in rows], 'input p95', [r['inputToFrameSwapMs']['p95'] for r in rows], 'janky', sum(r['chromiumJanky'] for r in rows))
