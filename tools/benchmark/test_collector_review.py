"""Synthetic format/gate tests; never actual Minecraft or overhead evidence."""
import hashlib
import tempfile
import unittest
from pathlib import Path

import collector_review as c


class CollectorReviewTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="aurora-collector-synthetic-")
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "frames.csv"
        self.lines = ["# PerformanceOverlay Benchmark", "# Date: 2026-10-17 01:00:00",
                      "# ModVersion: 1.0.2", "# Minecraft: 1.21.11", ",".join(c.COLUMNS),
                      "1,1.000,1000.0,0,0,0,0,0,0,0,,20,2048",
                      "2,1.000,1000.0,0,0,0,0,0,0,0,0.0,20,2048",
                      "# EndReason: MANUAL", "# SUMMARY", "# FramesLogged: 2", "# FramesSummary: 2"]
        self.meta = {"format": c.FORMAT, "sourceJarSha256": c.JAR_SHA256,
                     "workloadId": c.WORKLOAD, "worldArchiveSha256": c.ARCHIVE_SHA256,
                     "timestampUnit": "ms", "frameUnit": "ms", "expectedWindowMs": [0, 2],
                     "recordingComplete": True, "bufferOverflow": False}

    def run_review(self):
        self.path.write_text("\n".join(self.lines) + "\n", encoding="utf-8")
        self.meta["rawSha256"] = hashlib.sha256(self.path.read_bytes()).hexdigest()
        return c.review(self.path, self.meta)

    def test_consistency_never_promotes(self):
        r = self.run_review()
        self.assertEqual(r["status"], "CONSISTENT DIAGNOSTIC")
        self.assertEqual(r["metrics"]["p99Ms"], 1)
        self.assertIsNone(r["missingOrDroppedSamples"])
        self.assertFalse(r["independentCompleteness"])

    def test_missing_timestamp(self):
        self.lines[6] = self.lines[6].replace("2,", ",", 1)
        self.assertEqual(self.run_review()["status"], "INVALID")

    def test_invalid_units(self):
        self.meta["timestampUnit"] = "ns"
        self.assertEqual(self.run_review()["status"], "INVALID")

    def test_timestamp_ties_and_indistinguishable_duplicates(self):
        self.lines[5] = "1,0.500,2000.0,0,0,0,0,0,0,0,,20,2048"
        self.lines[6] = self.lines[5]
        r = self.run_review()
        self.assertEqual(r["timestampTies"], 1)
        self.assertEqual(r["indistinguishableAdjacentRows"], 1)
        self.assertFalse(r["independentCompleteness"])

    def test_buffer_overflow(self):
        self.meta["bufferOverflow"] = True
        self.assertEqual(self.run_review()["status"], "INCOMPLETE")

    def test_missing_source_metadata(self):
        del self.meta["sourceJarSha256"]
        self.assertEqual(self.run_review()["status"], "INVALID")

    def test_wrong_workload(self):
        self.meta["worldArchiveSha256"] = "0" * 64
        self.assertEqual(self.run_review()["status"], "INVALID")

    def test_locked_normal_terrain_remains_diagnostic(self):
        self.meta["workloadId"] = "workload-b-normal-stationary-v1"
        self.meta["worldArchiveSha256"] = c.WORKLOAD_ARCHIVES[self.meta["workloadId"]]
        result = self.run_review()
        self.assertEqual(result["status"], "CONSISTENT DIAGNOSTIC")
        self.assertEqual(result["metricRole"], "DIAGNOSTIC ONLY")
        self.assertFalse(result["observerOverheadBounded"])

    def test_crossed_snapshot_identity_refused(self):
        self.meta["workloadId"] = "workload-b-normal-stationary-v1"
        self.assertEqual(self.run_review()["status"], "INVALID")

    def test_unreviewed_and_malformed_workload_refused(self):
        for workload in ["unreviewed-world", None, [], {}]:
            with self.subTest(workload=workload):
                self.meta["workloadId"] = workload
                self.assertEqual(self.run_review()["status"], "INVALID")

    def test_unsupported_format(self):
        self.meta["format"] = "presentmon"
        self.assertEqual(self.run_review()["status"], "INVALID")

    def test_incomplete_window(self):
        self.meta["expectedWindowMs"] = [0, 1000]
        self.assertEqual(self.run_review()["status"], "INCOMPLETE")

    def test_invalid_jfr_alignment(self):
        self.meta["jfrAlignment"] = {"method": "subtract-nanotime-from-epoch"}
        self.assertEqual(self.run_review()["status"], "INVALID")

    def test_unjustified_metric_promotion(self):
        self.meta["requestedMetricRole"] = "primary"
        self.assertEqual(self.run_review()["status"], "INVALID")

    def test_missing_footer_and_truncated_summary(self):
        self.lines.pop()
        self.assertEqual(self.run_review()["status"], "INCOMPLETE")
        self.lines.append("# FramesSummary: 1")
        self.assertEqual(self.run_review()["status"], "INCOMPLETE")

    def test_nonfinite_interval_and_reversed_time(self):
        self.lines[6] = self.lines[6].replace("1.000", "NaN")
        self.assertEqual(self.run_review()["status"], "INVALID")
        self.lines[6] = "2,1e308,0,0,0,0,0,0,0,0,,20,2048"
        self.assertEqual(self.run_review()["status"], "INVALID")
        self.lines[6] = "0,1.000,1000.0,0,0,0,0,0,0,0,,20,2048"
        self.assertEqual(self.run_review()["status"], "INVALID")


if __name__ == "__main__":
    unittest.main()
