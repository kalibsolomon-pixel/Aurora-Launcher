"""Read-only P3-A.1 evidence checks; never authenticate, copy, restore or launch.

An offline match proves bytes/metadata consistency, not world provenance, absence
of writers, OS credential isolation, or authorization to use a directory.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import stat

MAX_FILES = 100_000
MAX_FILE_BYTES = 2 * 1024**3
MAX_TOTAL_BYTES = 16 * 1024**3
HASH = re.compile(r'[a-f0-9]{64}')
SID = re.compile(r'S-1-5-21-(?:[0-9]+-){3}[0-9]+')


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def isolation_guard(owner_sid, expected_test_sid, observed_sid, elevated):
    """Pure refusal guard. Caller must obtain observed facts from the live OS.

    Passing caller-supplied strings is not proof of the Windows token or vault.
    """
    require(all(isinstance(v, str) and SID.fullmatch(v)
                for v in (owner_sid, expected_test_sid, observed_sid)), 'invalid principal identity')
    require(expected_test_sid != owner_sid, 'test principal equals owner')
    require(observed_sid == expected_test_sid, 'active test principal mismatch')
    require(elevated is False, 'elevated or indeterminate context')


def plain(path):
    """Reject links/reparse points in the leaf and every existing ancestor."""
    for candidate in (path, *path.parents):
        info = candidate.lstat()
        require(not stat.S_ISLNK(info.st_mode)
                and not getattr(info, 'st_file_attributes', 0) & 0x400,
                'redirected evidence path')


def relative(name):
    require(isinstance(name, str) and len(name) <= 1024
            and re.fullmatch(r'[A-Za-z0-9_.\-/]+', name), 'unsafe inventory path')
    for part in name.split('/'):
        require(part not in ('', '.', '..') and not part.endswith('.')
                and not re.fullmatch(r'(?:con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\..*)?', part, re.I),
                'unsafe inventory component')


def digest(path):
    plain(path)
    require(path.is_file() and path.stat().st_size <= MAX_FILE_BYTES, 'invalid file or size bound')
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def inventory(root):
    plain(root)
    require(root.is_dir(), 'missing snapshot directory')
    files, directories, seen, total = [], [], set(), 0
    pending = [root]
    while pending:
        parent = pending.pop()
        for path in parent.iterdir():
            require(len(seen) < MAX_FILES, 'inventory count bound')
            name = path.relative_to(root).as_posix()
            relative(name)
            require(name.casefold() not in seen, 'case-colliding inventory')
            seen.add(name.casefold())
            plain(path)
            if path.is_dir():
                directories.append(name)
                pending.append(path)
            else:
                require(path.is_file(), 'special inventory entry')
                size = path.stat().st_size
                total += size
                require(total <= MAX_TOTAL_BYTES, 'inventory byte bound')
                files.append({'path': name, 'sizeBytes': size, 'sha256': digest(path)})
    return sorted(files, key=lambda x: x['path']), sorted(directories)


def read_json(path):
    plain(path)
    require(path.stat().st_size <= 16 * 1024**2, 'metadata size bound')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, 'duplicate metadata key')
            result[key] = value
        return result
    return json.loads(path.read_text(encoding='utf-8-sig'), object_pairs_hook=unique,
                      parse_constant=lambda _: require(False, 'nonfinite metadata'))


def world_check(manifest):
    manifest = manifest.absolute()
    m = read_json(manifest)
    require(set(m) == {'schemaVersion', 'sampleKind', 'workloadId', 'conditionsSha256',
                       'files', 'directories'}, 'snapshot manifest fields')
    require(type(m['schemaVersion']) is int and m['schemaVersion'] == 1, 'snapshot schema')
    require(m['sampleKind'] in ('synthetic', 'test-world') and m['workloadId'] in ('A', 'B', 'C'),
            'snapshot evidence identity')
    require(isinstance(m['conditionsSha256'], str) and HASH.fullmatch(m['conditionsSha256']),
            'missing conditions hash')
    conditions_path = manifest.parent / 'conditions.json'
    require(digest(conditions_path) == m['conditionsSha256'], 'conditions digest mismatch')
    c = read_json(conditions_path)
    fields = {'minecraftVersion', 'seed', 'generationSettings', 'playerPosition', 'orientation',
              'gameMode', 'difficulty', 'weather', 'timeOfDay', 'renderDistance',
              'simulationDistance', 'entityConditions', 'resourcePacks', 'shaders',
              'gamerules', 'generatedChunks'}
    require(isinstance(c, dict) and set(c) == fields, 'world conditions incomplete')
    require(c['minecraftVersion'] == '1.21.11', 'unexpected Minecraft world version')
    for key in ('seed', 'timeOfDay', 'renderDistance', 'simulationDistance'):
        require(type(c[key]) is int, 'invalid world integer')
    require(0 <= c['timeOfDay'] < 24000 and 2 <= c['renderDistance'] <= 32
            and 2 <= c['simulationDistance'] <= 32, 'invalid world settings')
    for key, length in (('playerPosition', 3), ('orientation', 2)):
        require(isinstance(c[key], list) and len(c[key]) == length
                and all(type(v) in (int, float) and math.isfinite(v) for v in c[key]),
                'invalid world camera')
    require(c['gameMode'] == 'creative' and c['difficulty'] == 'normal'
            and c['weather'] == 'clear', 'unexpected world conditions')
    for key in ('generationSettings', 'entityConditions', 'gamerules', 'generatedChunks'):
        require(isinstance(c[key], dict) and c[key], 'missing world conditions')
    require(c['resourcePacks'] == [] and c['shaders'] == [], 'unexpected packs or shaders')
    require(isinstance(m['files'], list) and m['files'] and isinstance(m['directories'], list),
            'missing complete inventory')
    for row in m['files']:
        require(isinstance(row, dict) and set(row) == {'path', 'sizeBytes', 'sha256'}, 'file fields')
        relative(row['path'])
        require(type(row['sizeBytes']) is int and 0 <= row['sizeBytes'] <= MAX_FILE_BYTES
                and isinstance(row['sha256'], str) and HASH.fullmatch(row['sha256']), 'file identity')
    for name in m['directories']:
        relative(name)
    require(any(row['path'] == 'level.dat' for row in m['files']), 'world completion data missing')
    for name in ('snapshot', 'restored'):
        files, directories = inventory(manifest.parent / name)
        require(files == m['files'] and directories == m['directories'], 'snapshot or restore inventory mismatch')
    return {'classification': 'VALID', 'sampleKind': m['sampleKind'], 'workloadId': m['workloadId'],
            'manifestSha256': digest(manifest), 'files': len(m['files']),
            'scope': 'offline inventory and conditions only; provenance and clean exit require review'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    args = parser.parse_args()
    try:
        result = world_check(args.manifest)
    except (ValueError, TypeError, KeyError, OSError, OverflowError, RecursionError):
        # Do not echo arbitrary source data or personal paths.
        result = {'classification': 'INVALID', 'reason': 'snapshot evidence missing, invalid or inconsistent'}
    print(json.dumps(result, indent=2))
    return 0 if result['classification'] == 'VALID' else 2


if __name__ == '__main__':
    raise SystemExit(main())
