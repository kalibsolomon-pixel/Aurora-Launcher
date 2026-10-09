"""Read-only public evidence, privacy, provenance and preservation validation."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
EVIDENCE = HERE / 'evidence'

def sha(p):
    with p.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))

spec = importlib.util.spec_from_file_location('p2', HERE / 'analyze.py')
p2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p2)
traces = sorted(EVIDENCE.glob('trace-*.json'))
assert traces
for path in traces: p2.attributed(path)
summary_before = (EVIDENCE / 'summary.json').read_bytes()
subprocess.run([sys.executable, str(HERE / 'analyze.py')], stdout=subprocess.DEVNULL, check=True)
assert summary_before == (EVIDENCE / 'summary.json').read_bytes()
# Reject private payloads, overflow and malformed ancestry with real trace
# mutations in an exclusively created scratch root. Never edit real evidence.
original = read(next(p for p in traces if 'attribution' in p.name))
negative = 0
with tempfile.TemporaryDirectory(prefix='aurora-p2-validator-') as temporary:
    scratch = Path(temporary) / 'trace.json'
    for mutation in ['string', 'overflow', 'unknown-event', 'foreign-parent']:
        doc = json.loads(json.dumps(original))
        if mutation == 'string': doc['records'][0]['correlation'] = 'private'
        elif mutation == 'overflow': doc['dropped'] = 1
        elif mutation == 'unknown-event': doc['records'][0]['event'] = 99
        else:
            for r in doc['records']:
                if r['event'] == 20 and r['correlation'] == 0: r['parent'] = 999999
        scratch.write_text(json.dumps(doc))
        try: p2.attributed(scratch)
        except (AssertionError, KeyError): negative += 1
        else: raise AssertionError('invalid trace admitted: ' + mutation)

for path in EVIDENCE.glob('*.json'):
    text = path.read_text(encoding='utf-8')
    assert not re.search(r'(?i)(?:[A-Z]:\\|[A-Z]:/Users/|Bearer\s|\b(?:accessToken|refreshToken|minecraft_name|processId|accountId)\b)', text), path.name

critical = ['AGENTS.md', 'ARCHITECTURE.md', 'README.md', 'PERFORMANCE_RESEARCH_P0_2.md',
            'PERFORMANCE_OPTIMIZATION_P1.md', 'package.json', 'package-lock.json',
            'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json',
            'src-tauri/build.rs', 'src-tauri/capabilities/default.json']
assert all((ROOT / p).is_file() for p in critical)
assert subprocess.check_output(['git', 'ls-files', '--deleted'], cwd=ROOT) == b''
baseline = read(HERE / 'private/baseline.json')
tracked = {r['relative'] for r in baseline['files'] if r['kind'] == 'tracked'}
current = set(subprocess.check_output(['git', 'ls-files', '-z'], cwd=ROOT).decode().strip('\0').split('\0'))
assert tracked <= current
for document in [ROOT / 'PERFORMANCE_RESEARCH_P2.md', HERE / 'README.md']:
    body = document.read_text(encoding='utf-8')
    for link in re.findall(r'\]\(([^)]+)\)', body):
        if '://' not in link:
            assert (document.parent / link.split('#')[0]).exists(), link

artifacts_checked = 0
if sys.argv[1:]:
    root = Path(sys.argv[1])
    for cohort in ['baseline', 'attribution', 'control']:
        manifest = read(EVIDENCE / (cohort + '-artifact.json'))
        artifact = root / 'artifacts' / cohort
        assert sha(artifact / 'aurora-launcher.exe') == manifest['executableSha256']
        for row in manifest['frontendFiles']:
            p = artifact / 'frontend' / row['path']
            assert p.stat().st_size == row['bytes'] and sha(p) == row['sha256']
        assert hashlib.sha256(json.dumps(manifest['frontendFiles'], sort_keys=True, separators=(',', ':')).encode()).hexdigest() == manifest['frontendTreeSha256']
        artifacts_checked += 1
    # SvelteKit's default build-version timestamp changes its global identifier
    # and dependent chunk filenames. Verify exactly these changes, not a loose
    # semantic minifier comparison; the other 17 files must match byte-for-byte.
    baseline_bundle = read(EVIDENCE / 'baseline-artifact.json')
    attribution_bundle = read(EVIDENCE / 'attribution-artifact.json')
    old_hashes = {r['sha256']: r['path'] for r in baseline_bundle['frontendFiles']}
    new_hashes = {r['sha256']: r['path'] for r in attribution_bundle['frontendFiles']}
    old_paths = [v for k, v in old_hashes.items() if k not in new_hashes]
    new_paths = [v for k, v in new_hashes.items() if k not in old_hashes]
    assert len(old_hashes.keys() & new_hashes.keys()) == 17
    assert len(old_paths) == len(new_paths) == 6
    old_root = root / 'artifacts/baseline/frontend'
    new_root = root / 'artifacts/attribution/frontend'
    replacements = dict(zip(old_paths, new_paths))
    replacements[re.search('__sveltekit_[a-z0-9]+', (old_root / 'index.html').read_text())[0]] = re.search('__sveltekit_[a-z0-9]+', (new_root / 'index.html').read_text())[0]
    replacements[read(old_root / '_app/version.json')['version']] = read(new_root / '_app/version.json')['version']
    for old_path, new_path in zip(old_paths, new_paths):
        old_text = (old_root / old_path).read_text()
        for before, after in replacements.items():
            old_text = old_text.replace(before, after).replace(before.rsplit('/', 1)[-1], after.rsplit('/', 1)[-1])
        assert old_text == (new_root / new_path).read_text()

assert subprocess.check_output(['git', 'diff', '--name-only',
    '0d68340f0e280978f675c6f83330a10fb97bf678', 'b8591ae78f9ba319dfd07f2d56dbe12085c272b4',
    '--', 'src', 'static', 'package.json', 'package-lock.json', 'vite.config.js', 'svelte.config.js', 'tsconfig.json'], cwd=ROOT) == b''

result = dict(numericTracesValidated=len(traces), strictNegativeSchemaTests=negative,
              summaryByteReproducible=True, sanitizedMetadataPatternChecks=True,
              originalTrackedFilesPresent=len(tracked), criticalFilesPresent=len(critical),
              trackedDeletions=0, artifactBundlesRehashed=artifacts_checked,
              canonicalFrontendSourceDifferences=0,
              arbitraryTextSafetyClaim=False)
print(json.dumps(result))
