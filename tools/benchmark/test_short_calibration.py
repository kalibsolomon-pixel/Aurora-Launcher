"""Real default-CSV regression specimen plus isolated malformed-input tests."""
import copy
import csv
import io
from pathlib import Path
import tempfile
import unittest

import short_calibration as s
import quality as q


class ShortCalibrationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='aurora-short-test-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.raw = Path(__file__).parent/'fixtures/presentmon-2.3.1-default-qpc.csv'
        (self.root/'frames.csv').write_bytes(self.raw.read_bytes())
        (self.root/'presentmon-stderr.txt').write_text('')
        self.rows = list(csv.DictReader(io.StringIO(self.raw.read_text(encoding='utf-8-sig'))))
        first = self.rows[0]
        self.m = dict(schemaVersion=1, artifactKind='short-calibration', targetKind='controlled-opengl',
                      status='complete', startUtc='2026-10-16T21:27:55Z', endUtc='2026-10-16T21:28:15Z',
                      pid=int(first['ProcessID']), creationId='134366596753000000', executableSha256='1'*64,
                      toolSha256=s.TOOL_SHA256, format=s.FORMAT, qpcFrequencyHz=10000000,
                      originQpc=int(first['TimeInQPC']), swapChain=first['SwapChainAddress'],
                      presentRuntime='Other', windowStartSeconds=.25, windowSeconds=1,
                      framesSha256=q.sha256(self.root/'frames.csv'), diagnosticsSha256=q.sha256(self.root/'presentmon-stderr.txt'))

    def run_check(self): return s.inspect(self.root, self.m)

    def mutate(self, index, key, value):
        self.rows[index][key] = value
        with (self.root/'frames.csv').open('w', newline='', encoding='utf-8') as f:
            writer = csv.DictWriter(f, fieldnames=self.rows[0]); writer.writeheader(); writer.writerows(self.rows)
        self.m['framesSha256'] = q.sha256(self.root/'frames.csv')

    def test_real_default_csv(self):
        result = self.run_check()
        self.assertEqual(result['quality'], 'VALID WITH WARNINGS')
        self.assertEqual(result['rawRows'], 180)
        self.assertEqual(result['fullMeasuredRunQuality'], 'INCOMPLETE')

    def test_wrong_identity_stream_runtime_and_qpc(self):
        for key, value in [('ProcessID', '999'), ('SwapChainAddress', '0x1'),
                           ('PresentRuntime', 'DXGI'), ('TimeInQPC', '1.2')]:
            with self.subTest(key=key):
                saved = copy.deepcopy(self.rows); self.mutate(5, key, value)
                with self.assertRaises(ValueError): self.run_check()
                self.rows = saved; self.mutate(5, key, saved[5][key])

    def test_invalid_and_unavailable_display_rows(self):
        self.mutate(5, 'MsUntilDisplayed', 'NA')
        self.assertGreater(self.run_check()['unavailableDisplayRows'], 0)
        for invalid in ('NaN', '-1', '0', '', 'Infinity'):
            self.mutate(5, 'MsUntilDisplayed', invalid)
            with self.assertRaises(ValueError): self.run_check()

    def test_duplicate_and_missing_columns(self):
        self.mutate(5, 'TimeInQPC', self.rows[4]['TimeInQPC'])
        with self.assertRaises(ValueError): self.run_check()
        (self.root/'frames.csv').write_text('ProcessID,ProcessID\n1,1\n')
        self.m['framesSha256'] = q.sha256(self.root/'frames.csv')
        with self.assertRaises(ValueError): self.run_check()

    def test_hash_and_manifest_refusals(self):
        for key, value in [('schemaVersion', 2), ('artifactKind', 'measured'),
                           ('windowSeconds', 600), ('qpcFrequencyHz', 0),
                           ('toolSha256', '2'*64), ('framesSha256', '2'*64),
                           ('startUtc', '2026-10-16T21:27:55'), ('pid', True)]:
            with self.subTest(key=key):
                saved = self.m[key]; self.m[key] = value
                with self.assertRaises(ValueError): self.run_check()
                self.m[key] = saved
        self.m['secret'] = 'forbidden'
        with self.assertRaises(ValueError): self.run_check()

    def test_incomplete_and_producer_loss(self):
        self.m['windowStartSeconds'] = 50
        with self.assertRaises(ValueError): self.run_check()
        self.m['windowStartSeconds'] = .25; self.m['status'] = 'interrupted'
        self.assertEqual(self.run_check()['quality'], 'INCOMPLETE')
        (self.root/'presentmon-stderr.txt').write_text('Lost 2 events')
        self.m['diagnosticsSha256'] = q.sha256(self.root/'presentmon-stderr.txt')
        with self.assertRaises(ValueError): self.run_check()

    def test_duplicate_metadata(self):
        path = self.root/'short.json'; path.write_text('{"schemaVersion":1,"schemaVersion":1}')
        with self.assertRaises(ValueError): s.load(path)

    def test_windows_redirected_diagnostic_encoding(self):
        path = self.root/'presentmon-stderr.txt'
        path.write_bytes('warning: elevated privilege process-query limitation'.encode('utf-16'))
        self.m['diagnosticsSha256'] = q.sha256(path)
        self.assertIn('unelevated-process-query-limitation', self.run_check()['warnings'])
        path.write_bytes(b'\xff\xfe\x01')
        self.m['diagnosticsSha256'] = q.sha256(path)
        with self.assertRaises(ValueError): self.run_check()


if __name__ == '__main__': unittest.main()
