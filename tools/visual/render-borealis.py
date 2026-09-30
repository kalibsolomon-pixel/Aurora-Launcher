"""Offline internal-curtain animation inside the original, anchored composition.

Only the aurora's fine emission detail is deformed. Its broad envelope, stars,
sky and landscape are fixed. Integer temporal harmonics close the 20-second
loop in both value and velocity. Pillow/NumPy/FFmpeg are authoring tools only.
"""
import argparse
import hashlib
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
p.add_argument('--preview', action='store_true', help='Render temporal stills and metrics without encoding')
args = p.parse_args()
source = Path(__file__).resolve().parents[2] / 'static/backgrounds/borealis.webp'
width, height, fps, seconds = 1920, 1080, 30, 20
im = Image.open(source).convert('RGB').resize((width, height), Image.Resampling.LANCZOS)
base = np.asarray(im, dtype=np.float32)
# Keep sharp stars in the fixed layer; do not remap the source image itself.
soft_image = im.filter(ImageFilter.MedianFilter(5)).filter(ImageFilter.GaussianBlur(1.2))
soft = np.asarray(soft_image, dtype=np.float32)
y, x = np.mgrid[0:height, 0:width].astype(np.float32)
x /= width
y /= height
sky = np.stack([7 + 10 * y, 18 + 14 * y, 28 + 16 * y], axis=2)
mask = np.clip((.80 - y) / .20, 0, 1)
emission = np.maximum(soft - sky, 0) * mask[:, :, None]

def blur_rgb(values, radius):
    return np.asarray(Image.fromarray(np.uint8(np.clip(np.rint(values), 0, 255)))
                      .filter(ImageFilter.GaussianBlur(radius)), dtype=np.float32)

# The large-scale emission envelope never moves. Only the signed fine detail
# relative to it is sampled at locally deformed coordinates. This prevents a
# moving source silhouette, sky warp, or camera drift.
envelope = blur_rgb(emission, 24)
detail = emission - envelope
strength = np.max(emission, axis=2)
support = np.clip((strength - 9) / 38, 0, 1)
support = support * support * (3 - 2 * support) * mask
pixel_y, pixel_x = np.mgrid[0:height, 0:width].astype(np.float32)

def sample_detail(dx, dy):
    sx = np.clip(pixel_x + dx, 0, width - 1.001)
    sy = np.clip(pixel_y + dy, 0, height - 1.001)
    ix, iy = sx.astype(np.int32), sy.astype(np.int32)
    ax, ay = (sx - ix)[:, :, None], (sy - iy)[:, :, None]
    return ((detail[iy, ix] * (1 - ax) + detail[iy, ix + 1] * ax) * (1 - ay)
            + (detail[iy + 1, ix] * (1 - ax) + detail[iy + 1, ix + 1] * ax) * ay)

# Curved phases and unequal wavelengths prevent a single rigid travelling
# stripe. Displacement changes sign locally: folds bend, stretch and contract
# around their original positions. Frequencies are integer cycles / 20 s.
tau = 2 * math.pi
fold_phase = tau * (7.1 * x + 2.3 * y + .22 * np.sin(tau * (2.1 * x - y)))
stretch_phase = tau * (3.8 * x - 4.6 * y + .18 * np.sin(tau * (x + 2 * y)))
light_phase = tau * (9.4 * x + 1.8 * y + .28 * np.sin(tau * (1.7 * x - y)))
rise_phase = tau * (2.7 * x - 6.2 * y + .20 * np.sin(tau * (2.3 * x + y)))

def frame(t):
    phase = 2 * math.pi * (t % seconds) / seconds
    dx = support * (12 * np.sin(fold_phase - 2 * phase)
                    + 5 * np.sin(stretch_phase + 3 * phase))
    dy = support * (8 * np.sin(stretch_phase + 2 * phase)
                    + 4 * np.sin(fold_phase - 3 * phase))
    moving_detail = sample_detail(dx, dy)
    redistributed = envelope + moving_detail
    light = (.27 * np.sin(light_phase - 3 * phase)
             + .16 * np.sin(rise_phase + 2 * phase))
    delta = moving_detail - detail + redistributed * light[:, :, None]
    # Remove broad luminosity changes from the local delta. Together with the
    # fixed support this keeps the macro envelope anchored while light travels
    # through individual folds, instead of the whole curtain breathing.
    broad_delta = (blur_rgb(delta * .5 + 128, 32) - 128) * 2
    delta = (delta - broad_delta) * support[:, :, None]
    return np.clip(np.rint(base + delta), 0, 255).astype(np.uint8)

args.output.parent.mkdir(parents=True, exist_ok=True)
args.evidence.mkdir(parents=True, exist_ok=True)
for t in (0, 2, 4, 6, 10, 15, 19 + 29 / 30, 20):
    Image.fromarray(frame(t)).save(args.evidence / f'frame-{t:06.2f}s.png')
first, last, repeated = frame(0), frame(seconds - 1 / fps), frame(seconds)
first_step = frame(1 / fps).astype(float) - first.astype(float)
seam_step = first.astype(float) - last.astype(float)
report = {'resolution': [width, height], 'sourceResolution': list(Image.open(source).size),
          'fps': fps, 'seconds': seconds, 'frames': seconds * fps,
          'periodEndpointMaxDifference': int(np.abs(first.astype(int) - repeated.astype(int)).max()),
          'seamMeanStep': float(np.abs(seam_step).mean()),
          'firstStepMean': float(np.abs(first_step).mean()),
          'seamVelocityDifferenceMean': float(np.abs(seam_step - first_step).mean()),
          'maximumDetailDisplacementPixels': [17, 12],
          'outsideSupportMaxDifference': int(np.abs(first.astype(float) - base)[support == 0].max()),
          'technique': 'local fold deformation and travelling light; fixed broad emission envelope and source composition'}
if report['periodEndpointMaxDifference'] or report['outsideSupportMaxDifference']:
    raise RuntimeError('The loop must close exactly and leave the fixed support exterior unchanged')
if args.preview:
    (args.evidence / 'render-report.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(report), flush=True)
    raise SystemExit(0)
# Retain a lossless local authoring master so seam verification tests the final
# delivery codec too. Encode three periods, then keep the middle one: its first
# IDR has the same surrounding periodic context as every later loop boundary,
# rather than the encoder's special cold-start allocation/smoothing state.
master = args.evidence / 'borealis-master.mkv'
periodic = args.evidence / 'borealis-periodic-encode.mp4'
command = [args.ffmpeg, '-hide_banner', '-loglevel', 'warning', '-y',
           '-f', 'rawvideo', '-pixel_format', 'rgb24', '-video_size', f'{width}x{height}',
           '-framerate', str(fps), '-i', '-', '-an', '-c:v', 'libx264rgb',
           '-preset', 'veryfast', '-crf', '0', '-pix_fmt', 'rgb24',
           '-g', str(fps * 2), str(master)]
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
subprocess.run([args.ffmpeg, '-hide_banner', '-loglevel', 'warning', '-y',
                '-stream_loop', '2', '-i', str(master), '-an', '-c:v', 'libx264',
                '-preset', 'slow', '-tune', 'grain', '-crf', '18', '-pix_fmt', 'yuv420p',
                '-g', str(fps * 2), '-keyint_min', str(fps * 2), '-sc_threshold', '0',
                '-x264-params', 'open-gop=0', str(periodic)], check=True)
subprocess.run([args.ffmpeg, '-hide_banner', '-loglevel', 'warning', '-y',
                '-ss', str(seconds), '-i', str(periodic), '-frames:v', str(seconds * fps),
                '-map', '0:v:0', '-c', 'copy', '-movflags', '+faststart', str(args.output)], check=True)
report['bytes'] = args.output.stat().st_size
report['sha256'] = hashlib.sha256(args.output.read_bytes()).hexdigest()
validation = subprocess.run([args.ffmpeg, '-v', 'error', '-nostats', '-i', str(args.output),
                             '-progress', 'pipe:1', '-f', 'null', '-'],
                            capture_output=True, text=True, check=True)
report['decodedFrames'] = int([line.split('=')[1] for line in validation.stdout.splitlines()
                              if line.startswith('frame=')][-1])
if validation.stderr or report['decodedFrames'] != seconds * fps:
    raise RuntimeError('Delivery video must decode cleanly to exactly 600 frames')
(args.evidence / 'render-report.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report), flush=True)
