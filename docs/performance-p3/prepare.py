"""Copy protected inputs, then patch only the disposable research checkout."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]

def sha(p):
    with p.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

def replace(path, old, new):
    text = path.read_text(encoding='utf-8')
    assert text.count(old) == 1, 'source boundary changed: ' + path.name
    path.write_text(text.replace(old, new), encoding='utf-8', newline='\n')

def prepare(root):
    root = Path(root).resolve(strict=True)
    assert root.name.startswith('aurora-p0-2-p3-')
    baseline = json.loads((root / 'baseline.json').read_text())
    assert json.loads((root / 'jna-isolation.json').read_text())['protectedJnaChanged'] == 0
    managed = Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
    # Copies, never links. Every existing managed non-operational byte is checked.
    target = root / 'managed'
    target.mkdir(exist_ok=False)
    for folder in ['launcher', 'instances', 'runtimes', 'cache', 'metadata']:
        src = managed / folder
        if src.exists():
            assert not any(p.is_symlink() or p.is_junction() for p in src.rglob('*'))
            shutil.copytree(src, target / folder)
    rows = [r for r in baseline['files'] if r['kind'] == 'managed']
    assert all(sha(target / r['relative']) == r['sha256'] for r in rows)
    print('managed copy verified', len(rows), flush=True)
    stage = root / 'source'
    tracked = subprocess.check_output(['git', 'ls-files', '-z'], cwd=REPO).decode().split('\0')
    inputs = []
    for name in filter(None, tracked):
        src, dest = REPO / name, stage / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dest)
        inputs.append(dict(path=name, sha256=sha(src)))
    shutil.copytree(REPO / 'node_modules', stage / 'node_modules')
    # Source contract tests inspect HEAD. Only this new repository/index is written.
    subprocess.run(['git', 'init', str(stage)], check=True, capture_output=True)
    subprocess.run(['git', '-C', str(stage), 'fetch', '--no-tags', str(REPO), 'HEAD'], check=True, capture_output=True)
    head = baseline['repositories']['launcher']['head'].strip()
    subprocess.run(['git', '-C', str(stage), 'update-ref', 'HEAD', head], check=True)
    subprocess.run(['git', '-C', str(stage), 'read-tree', 'HEAD'], check=True)
    # Research feature exists only in this disposable copy; production files stay unchanged.
    cargo = stage / 'src-tauri/Cargo.toml'
    replace(cargo, 'performance-p0-2 = []', 'performance-p0-2 = []\nperformance-p3 = ["performance-p0-2"]')
    paths = stage / 'src-tauri/src/paths.rs'
    replace(paths, '        if !data_root.is_absolute() {', '''        #[cfg(feature = "performance-p3")]
        let data_root = if let Some(root) = std::env::var_os("AURORA_P3_ROOT") {
            let root = PathBuf::from(root);
            let canonical = root.canonicalize().expect("research root exists");
            assert!(canonical.starts_with(std::env::temp_dir().canonicalize().unwrap()));
            assert!(canonical.file_name().unwrap().to_string_lossy().starts_with("aurora-p0-2-p3-"));
            assert!(canonical.join("baseline.json").is_file());
            root.join("managed")
        } else { data_root };
        if !data_root.is_absolute() {''')
    process = stage / 'src-tauri/src/launch/process.rs'
    replace(process, '    let arguments = spec.command_arguments();', '''    let arguments = spec.command_arguments();
    #[cfg(feature = "performance-p3")]
    let arguments = {
        let mut arguments = arguments;
        let extra = p3_research::arguments().map_err(|_| LaunchProcessError::Spawn("research isolation failed".into()))?;
        arguments.splice(spec.jvm_arguments().len()..spec.jvm_arguments().len(),
            extra.into_iter().map(super::resolve::ResolvedArgument::Plain));
        arguments
    };''')
    replace(process, '    for name in super::activity_bridge::ENVIRONMENT {', '''    #[cfg(feature = "performance-p3")]
    p3_research::environment(&mut command);
    for name in super::activity_bridge::ENVIRONMENT {''')
    replace(process, '    let process_id = child.id();', '''    let process_id = child.id();
    #[cfg(feature = "performance-p3")]
    p3_research::spawned(process_id);''')
    with process.open('a', encoding='utf-8') as f:
        f.write('\n#[cfg(feature = "performance-p3")]\nmod p3_research;\n')
    dest = process.parent / 'process/p3_research.rs'
    dest.parent.mkdir(exist_ok=True)
    shutil.copy2(HERE / 'research_spawn.rs', dest)
    replace(stage / 'src-tauri/src/performance.rs', '                Some(State {\n                    start: Instant::now(),', '''                let start = Instant::now();
                #[cfg(feature = "performance-p3")]
                {
                    let before = start.elapsed().as_micros() as u64;
                    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64;
                    let after = start.elapsed().as_micros() as u64;
                    let mut clock = OpenOptions::new().write(true).create_new(true).open(root.join("native-clock.json")).unwrap();
                    use std::io::Write;
                    write!(clock, "{{\\"nativeUsBefore\\":{before},\\"unixUs\\":{unix},\\"nativeUsAfter\\":{after}}}").unwrap();
                }
                Some(State {
                    start,''')
    config = json.loads((HERE / 'build-config-template.json').read_text())
    config['app'] = json.loads((REPO / 'src-tauri/tauri.conf.json').read_text())['app']
    config['app']['windows'][0]['dataDirectory'] = str(root / 'webview')
    (root / 'build-config.json').write_text(json.dumps(config))
    (root / 'source-inputs.json').write_text(json.dumps(inputs, indent=2))
    previous_index = REPO / 'docs/performance-p2/private/research-root.txt'
    if previous_index.exists():
        previous = Path(previous_index.read_text().strip()) / 'targets/attribution'
        if previous.is_dir(): shutil.copytree(previous, root / 'target')
    (root / 'copy-proof.json').write_text(json.dumps(dict(protectedManagedCopies=len(rows),
        allCopiedBytesMatch=True, trackedSourceInputs=len(inputs),
        sourceHead=baseline['repositories']['launcher']['head'].strip()), indent=2))
    print('source and compiler cache copied', flush=True)

if __name__ == '__main__': prepare(sys.argv[1])
