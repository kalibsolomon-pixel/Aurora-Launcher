import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import readiness_checks as r


class ReadinessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='aurora-p3a1-evidence-test-')
        self.addCleanup(self.temp.cleanup)  # Only this test's exclusively created root.
        self.root = Path(self.temp.name)
        self.snapshot = self.root / 'snapshot'
        self.snapshot.mkdir()
        (self.snapshot / 'level.dat').write_bytes(b'synthetic-not-a-Minecraft-world')
        (self.snapshot / 'region').mkdir()
        (self.snapshot / 'region/r.0.0.mca').write_bytes(b'synthetic-chunks')
        (self.snapshot / 'empty').mkdir()
        shutil.copytree(self.snapshot, self.root / 'restored')
        self.conditions = dict(minecraftVersion='1.21.11', seed=123456789,
                               generationSettings={'type': 'superflat'},
                               playerPosition=[.5, 66, .5], orientation=[0, 10],
                               gameMode='creative', difficulty='normal', weather='clear',
                               timeOfDay=6000, renderDistance=12, simulationDistance=8,
                               entityConditions={'spawned': 0}, resourcePacks=[], shaders=[],
                               gamerules={'doMobSpawning': False},
                               generatedChunks={'status': 'synthetic-inventory'})
        self.write_conditions()
        files, directories = r.inventory(self.snapshot)
        self.m = dict(schemaVersion=1, sampleKind='synthetic', workloadId='A',
                      conditionsSha256=r.digest(self.root / 'conditions.json'),
                      files=files, directories=directories)
        self.manifest = self.root / 'snapshot.json'

    def write_conditions(self):
        (self.root / 'conditions.json').write_text(json.dumps(self.conditions))

    def check(self):
        self.manifest.write_text(json.dumps(self.m))
        return r.world_check(self.manifest)

    def test_complete_synthetic_inventory_and_restoration(self):
        before = {p: hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in self.root.rglob('*') if p.is_file()}
        result = self.check()
        self.assertEqual(result['classification'], 'VALID')
        self.assertEqual(result['sampleKind'], 'synthetic')
        self.assertEqual(result['files'], 2)
        for path, digest in before.items():
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), digest)

    def test_missing_added_changed_and_empty_directory_restore(self):
        target = self.root / 'restored/level.dat'
        for content in (b'', b'tampered'):
            target.write_bytes(content)
            with self.assertRaises(ValueError): self.check()
        target.unlink()
        with self.assertRaises(ValueError): self.check()
        shutil.copyfile(self.snapshot / 'level.dat', target)
        (self.root / 'restored/extra').write_bytes(b'unknown')
        with self.assertRaises(ValueError): self.check()
        (self.root / 'restored/extra').unlink()
        (self.root / 'restored/empty').rmdir()
        with self.assertRaises(ValueError): self.check()

    def test_unknown_schema_duplicate_inventory_and_unsafe_paths(self):
        original = copy.deepcopy(self.m)
        for version in (2, True):
            self.m = copy.deepcopy(original); self.m['schemaVersion'] = version
            with self.assertRaises(ValueError): self.check()
        for name in ('../owner', '/absolute', 'C:/owner', 'a\\b', 'a//b', 'CON.dat', 'name.'):
            self.m = copy.deepcopy(original); self.m['files'][0]['path'] = name
            with self.assertRaises(ValueError): self.check()
        self.m = copy.deepcopy(original); self.m['files'].append(self.m['files'][0])
        with self.assertRaises(ValueError): self.check()

    def test_missing_conditions_hash_drift_and_nonfinite_camera(self):
        del self.conditions['generatedChunks']; self.write_conditions()
        self.m['conditionsSha256'] = r.digest(self.root / 'conditions.json')
        with self.assertRaises(ValueError): self.check()
        self.conditions['generatedChunks'] = {'status': 'synthetic'}
        self.conditions['orientation'] = [float('nan'), 10]; self.write_conditions()
        self.m['conditionsSha256'] = r.digest(self.root / 'conditions.json')
        with self.assertRaises(ValueError): self.check()
        self.conditions['orientation'] = [0, 10]; self.write_conditions()
        with self.assertRaises(ValueError): self.check()  # Digest was not refreshed.

    def test_duplicate_json_keys_and_case_collision(self):
        self.manifest.write_text('{"schemaVersion":1,"schemaVersion":1}')
        with self.assertRaises(ValueError): r.world_check(self.manifest)
        with patch.object(Path, 'iterdir', return_value=iter([self.snapshot/'level.dat', self.snapshot/'LEVEL.dat'])):
            with self.assertRaises(ValueError): r.inventory(self.snapshot)

    def test_reparse_and_byte_count_bounds(self):
        info = self.snapshot.stat()
        class Redirected:
            st_mode = info.st_mode
            st_file_attributes = 0x400
        with patch.object(Path, 'lstat', return_value=Redirected()):
            with self.assertRaises(ValueError): r.inventory(self.snapshot)
        with patch.object(r, 'MAX_FILE_BYTES', 1):
            with self.assertRaises(ValueError): self.check()
        with patch.object(r, 'MAX_FILES', 1):
            with self.assertRaises(ValueError): r.inventory(self.snapshot)
        with patch.object(r, 'MAX_TOTAL_BYTES', 1):
            with self.assertRaises(ValueError): r.inventory(self.snapshot)

    def test_cli_refusal_does_not_echo_private_input(self):
        self.manifest.write_text('{"password":"never-echo-fixture-secret"}')
        result = subprocess.run([sys.executable, '-B', str(Path(r.__file__)), str(self.manifest)],
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertNotIn('never-echo', result.stdout + result.stderr)
        self.assertNotIn(str(self.root), result.stdout + result.stderr)

    def test_isolation_refuses_owner_wrong_principal_and_elevation(self):
        owner, test = 'S-1-5-21-1-2-3-1003', 'S-1-5-21-1-2-3-1007'
        r.isolation_guard(owner, test, test, False)
        for args in ((owner, owner, owner, False), (owner, test, owner, False),
                     (owner, test, test, True), (owner, test, test, None),
                     (owner, test, 'unknown', False)):
            with self.assertRaises(ValueError): r.isolation_guard(*args)


if __name__ == '__main__':
    unittest.main()
