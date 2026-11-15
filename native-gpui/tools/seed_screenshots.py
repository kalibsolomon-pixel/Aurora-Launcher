"""Generate a new disposable showcase root; never reads existing launcher data.

python native-gpui/tools/seed_screenshots.py target/screenshots-showcase --count 305
Requires Pillow. Refuses an existing destination. No game/network/Java work.
"""
import argparse
import json
import os
import time
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

parser = argparse.ArgumentParser()
parser.add_argument("destination", type=Path)
parser.add_argument("--count", type=int, default=36)
args = parser.parse_args()
root = args.destination.resolve()
root.mkdir(parents=True, exist_ok=False)
(root / "AURORA_DISPOSABLE_DIAGNOSTIC").write_text("Generated screenshots showcase only.\n")
launcher = root / "launcher"
launcher.mkdir()
instances = []
for identifier, name in [("gallery-practice", "Practice screenshots"), ("gallery-empty", "New instance")]:
    (root / "instances" / identifier / "screenshots").mkdir(parents=True)
    instances.append({"id": identifier, "displayName": name, "state": "ready",
        "installed": {"minecraftVersion": "1.21.4", "platform": {"kind": "vanilla"}, "aurora": None},
        "configuration": {"minecraftVersion": "1.21.4", "auroraEnabled": False,
            "loader": {"kind": "vanilla"}, "memoryMib": 4096,
            "additionalJvmArguments": "", "window": None}})
(launcher / "instances.json").write_text(json.dumps({"schemaVersion": 6, "instances": instances}))
(launcher / "config.json").write_text(json.dumps({"schemaVersion": 1, "selectedInstanceId": "gallery-practice"}))
font = ImageFont.truetype("C:/Windows/Fonts/segoeui.ttf", 24)
large = ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 42)
small = ImageFont.truetype("C:/Windows/Fonts/segoeui.ttf", 18)
now = int(time.time())
for index in range(args.count):
    image = Image.new("RGB", (960, 540), (23 + index % 20, 41, 57 + index % 35))
    draw = ImageDraw.Draw(image)
    # Intentionally synthetic practice arena: no game or owner image is read.
    for x in range(0, 960, 80):
        draw.line((x, 280, 480 + (x - 480) * 3, 540), fill=(45, 68, 77), width=2)
    for y in [300, 340, 400, 490]:
        draw.line((0, y, 960, y), fill=(45, 68, 77), width=2)
    draw.rectangle((45, 45, 540, 218), fill=(15, 24, 34))
    draw.text((68, 61), "Practice result", font=large, fill=(230, 238, 245))
    draw.text((68, 121), f"Round {args.count - index:03}   /   Victory", font=font, fill=(132, 212, 184))
    draw.text((68, 168), f"Accuracy {89 + index % 10}%    Best combo {12 + index % 16}", font=font, fill=(198, 211, 220))
    draw.rectangle((700, 76, 930, 278), fill=(15, 24, 34))
    draw.text((720, 94), "SCOREBOARD", font=font, fill=(203, 192, 250))
    for row, text in enumerate(["Practice arena", "Wins: 12", "Streak: 4", "Time: 02:18"]):
        draw.text((720, 138 + row * 30), text, font=small, fill=(210, 220, 228))
    draw.line((470, 270, 490, 270), fill="white", width=2)
    draw.line((480, 260, 480, 280), fill="white", width=2)
    draw.text((30, 502), "GENERATED GALLERY FIXTURE", font=small, fill=(166, 181, 193))
    path = root / "instances" / "gallery-practice" / "screenshots" / f"practice-{args.count - index:03}.png"
    image.save(path)
    os.utime(path, (now - index * 3600, now - index * 3600))
print(root)
