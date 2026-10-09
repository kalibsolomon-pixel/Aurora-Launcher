"""Reuse P0's read-only snapshot while protecting every pre-existing P0 tool."""
import pathlib
import sys

base = pathlib.Path(__file__).resolve().parent
assert len(sys.argv) == 2, 'provide the new P0.1 private inventory directory'
destination = pathlib.Path(sys.argv[1]).resolve()
assert destination == base / 'private', 'P0.1 inventories belong in this ignored private directory'
assert not any((destination / name).exists() for name in ('launcher-baseline.json', 'client-baseline.json')), \
    'preserve the existing inventories; a new investigation needs its own tools/destination'
source_path = base.parent / 'performance-p0/snapshot.py'
source = source_path.read_text(encoding='utf-8')
old = "name.startswith('docs/performance-p0/')"
new = "name.startswith('docs/performance-p0-1/')"
assert source.count(old) == 1, 'review the reused snapshot if its exclusion changed'
exec(compile(source.replace(old, new), str(source_path), 'exec'))
