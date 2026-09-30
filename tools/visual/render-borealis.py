"""Offline 1080p light-transport loop from the project's original Borealis still.

Requires Python, Pillow, NumPy and an explicit FFmpeg executable. No runtime
dependency, camera movement, full-image deformation, downloaded footage or
AI-video service. Local aurora emission fluctuates inside a fixed silhouette;
landscape, stars and the overall curtains stay in place.
"""
import argparse
import json
import math
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

p = argparse.ArgumentParser()
p.add_argument('--ffmpeg', required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--evidence', type=Path, required=True)
args = p.parse_args()
source = Path(__file__).resolve().parents[2] / 'static/backgrounds/borealis.webp'
width, height, fps, seconds = 1920, 1080, 30, 20
im = Image.open(source).convert('RGB').resize((width, height), Image.Resampling.LANCZOS)
base = np.asarray(im, dtype=np.float32)
# Remove tiny star peaks from the changing emission, without altering the base.
soft = np.asarray(im.filter(ImageFilter.GaussianBlur(2)), dtype=np.float32)
y, x = np.mgrid[0:height, 0:width].astype(np.float32)
x /= width
y /= height
# A continuous dark-sky floor avoids leaving black silhouettes where a bright
# curtain used to be. Tiny star peaks remain in the fixed high-frequency base.
sky = np.stack([7 + 10 * y, 18 + 14 * y, 28 + 16 * y], axis=2)
mask = np.clip((.80 - y) / .20, 0, 1)
emission = np.maximum(soft - sky, 0) * mask[:, :, None]
fixed = base - emission
# Broad, irregular light patches replace linear phase bands. Smooth random
# fields have no preferred stripe direction. Only their emission changes;
# neither the artwork nor the curtain silhouette is displaced.
rng = np.random.default_rng(29092026)

def light_field(columns, rows, blur):
    samples = rng.integers(0, 256, (rows, columns), dtype=np.uint8)
    patch = Image.fromarray(samples).resize((width, height), Image.Resampling.BICUBIC)
    values = np.asarray(patch.filter(ImageFilter.GaussianBlur(blur)), dtype=np.float32)
    return (values - values.mean()) / max(float(values.std()), 1)

# Integer temporal frequencies make values and derivatives meet at the seam.
terms = [(light_field(9, 6, 65), light_field(9, 6, 65), .72, 1),
         (light_field(16, 10, 45), light_field(16, 10, 45), .38, 2)]

def frame(t):
    phase = 2 * math.pi * (t % seconds) / seconds
    gain = np.zeros((height, width), dtype=np.float32)
    for sine, cosine, amplitude, frequency in terms:
        q = phase * frequency
        gain += amplitude * (sine * (math.cos(q) - 1) + cosine * math.sin(q))
    # Keep dim curtains at least 68% luminous; favor a stronger brightening
    # response so the changing light remains visible behind launcher glass.
    response = np.tanh(gain)
    intensity = 1 + .32 * response + .42 * np.maximum(response, 0) ** 2
    return np.clip(np.rint(fixed + emission * intensity[:, :, None]), 0, 255).astype(np.uint8)

args.output.parent.mkdir(parents=True, exist_ok=True)
args.evidence.mkdir(parents=True, exist_ok=True)
command = [args.ffmpeg, '-hide_banner', '-loglevel', 'warning', '-y',
           '-f', 'rawvideo', '-pixel_format', 'rgb24', '-video_size', f'{width}x{height}',
           '-framerate', str(fps), '-i', '-', '-an', '-c:v', 'libx264',
           '-preset', 'slow', '-crf', '19', '-pix_fmt', 'yuv420p',
           '-movflags', '+faststart', '-g', str(fps * 2), str(args.output)]
encoder = subprocess.Popen(command, stdin=subprocess.PIPE)
try:
    for i in range(seconds * fps):
        pixels = frame(i / fps)
        encoder.stdin.write(pixels.tobytes())
        if i % (fps * 6) == 0:
            Image.fromarray(pixels).save(args.evidence / f'render-{i // fps:02d}s.jpg', quality=90)
            print(f'Rendered {i // fps}/{seconds} seconds', flush=True)
finally:
    encoder.stdin.close()
if encoder.wait() != 0:
    raise RuntimeError('FFmpeg encoding failed')
first, last, repeated = frame(0), frame(seconds - 1 / fps), frame(seconds)
report = {'resolution': [width, height], 'sourceResolution': list(Image.open(source).size),
          'fps': fps, 'seconds': seconds, 'frames': seconds * fps,
          'bytes': args.output.stat().st_size,
          'periodEndpointMaxDifference': int(np.abs(first.astype(int) - repeated.astype(int)).max()),
          'seamMeanStep': float(np.abs(first.astype(float) - last.astype(float)).mean()),
          'firstStepMean': float(np.abs(frame(1 / fps).astype(float) - first.astype(float)).mean()),
          'technique': 'soft irregular emission fluctuations in a fixed aurora silhouette; no linear phase bands, translation or image warp'}
(args.evidence / 'render-report.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report), flush=True)
