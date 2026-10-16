import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import subprocess
import sys
import process_capture as pc
import quality


@unittest.skipUnless(os.name=='nt','actual Windows API plumbing; not Linux validation')
class WindowsProcessTests(unittest.TestCase):
    def test_own_process_identity_and_read_only_memory_counters(self):
        reader=pc.Reader(os.getpid())
        try:
            row=reader.sample();self.assertEqual(row['pid'],os.getpid())
            self.assertEqual(row['creation_id'],reader.creation_id)
            self.assertGreater(row['rss_bytes'],0)
            self.assertLessEqual(row['rss_bytes'],row['peak_rss_bytes'])
            self.assertLessEqual(row['system_available_bytes'],row['system_total_bytes'])
        finally: reader.close()

    def test_bounded_self_capture_is_ingestible(self):
        with tempfile.TemporaryDirectory(prefix='aurora-p3a-self-test-') as tmp:
            reader=pc.Reader(os.getpid());identity=reader.creation_id;reader.close()
            directory=Path(tmp)/'new-run';meta=pc.capture(os.getpid(),identity,.5,.1,directory)
            result=quality.process_metrics(directory/'process.csv',meta,0,.5,.15)
            self.assertTrue(result['boundary_coverage']);self.assertGreater(result['rss_bytes_peak_sampled'],0)

    def test_identity_mismatch_and_existing_output_are_refused(self):
        with tempfile.TemporaryDirectory(prefix='aurora-p3a-refusal-') as tmp:
            with self.assertRaises(ValueError):pc.capture(os.getpid(),'wrong',.2,.1,Path(tmp)/'new-run')
            with self.assertRaises(FileExistsError):pc.capture(os.getpid(),'wrong',.2,.1,Path(tmp))

    def test_nonfinite_and_unbounded_parameters_are_refused(self):
        for seconds,interval in [(float('nan'),1),(1,float('inf')),(661,.5),(1,.01)]:
            with self.assertRaises(ValueError):pc.capture(os.getpid(),'unused',seconds,interval,Path('never-created'))

    def test_unvalidated_processor_group_accounting_is_refused(self):
        with patch('process_capture.os.cpu_count',return_value=65):
            with self.assertRaises(ValueError):pc.Reader(os.getpid())

    def test_exact_handle_exit_detection_even_for_exit_code_259(self):
        child=subprocess.Popen([sys.executable,'-c','import time,sys;time.sleep(.3);sys.exit(259)'])
        reader=pc.Reader(child.pid)
        try:
            self.assertEqual(child.wait(timeout=3),259)
            with self.assertRaises(OSError):reader.sample()
        finally:
            reader.close();child.wait(timeout=3)


if __name__=='__main__':unittest.main()
