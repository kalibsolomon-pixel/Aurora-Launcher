"""Synthetic fixtures only; these tests never launch Minecraft or Java."""
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

HERE = Path(__file__).parent
spec = importlib.util.spec_from_file_location("p1_analyze", HERE / "analyze.py")
a = importlib.util.module_from_spec(spec)
spec.loader.exec_module(a)


class AnalysisTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="aurora-p1-parser-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for path in (HERE / "fixtures").iterdir():
            shutil.copyfile(path, self.root / path.name)
        self.path = self.root / "run.json"
        self.manifest = json.loads(self.path.read_text())

    def run_fixture(self):
        self.path.write_text(json.dumps(self.manifest), encoding="utf-8")
        return a.analyze(self.path)

    def test_exact_summary_and_no_double_counting(self):
        result = self.run_fixture()
        self.assertEqual(result["sampleKind"], "synthetic")
        self.assertEqual(result["metrics"]["frames_ms"]["mean"], 25)
        self.assertEqual(result["metrics"]["frames_ms"]["median"], 25)
        self.assertAlmostEqual(result["metrics"]["frames_ms"]["p95"], 38.5)
        self.assertEqual(result["metrics"]["frames_ms"]["variance"], 125)
        self.assertEqual(result["metrics"]["frames_ms"]["sustained_fps"], 40)
        self.assertIsNone(result["metrics"]["frames_ms"]["p999"])
        gc = result["metrics"]["gc"]
        self.assertEqual(gc["pause_ms"]["count"], 3)
        self.assertEqual(gc["total_pause_ms"], 6.5)
        self.assertEqual(gc["full_gc_count"], 0)
        self.assertEqual(gc["max_post_gc_used_bytes"], 30 * 1024**2)
        self.assertEqual(gc["outside_window_pause_count"], 1)
        self.assertIsNone(gc["oom_count"])
        self.assertEqual(result["metrics"]["process"]["cpu_percent_one_core"]["time_weighted_mean"], 150)
        self.assertIsNone(result["metrics"]["process"]["paging_bytes_per_second"])
        self.assertEqual(len(result["source_sha256"]["gc"]), 64)

    def test_percentiles_outliers_units_and_empty(self):
        self.assertEqual(a.percentile([1], .99), 1)
        self.assertEqual(a.percentile([1, 2, 3, 1000], .5), 2.5)
        self.assertGreater(a.percentile([1, 2, 3, 1000], .99), 900)
        self.assertEqual(a.distribution([])["count"], 0)
        self.assertIsNone(a.distribution([1] * 9999)["p999"])
        self.assertEqual(a.distribution([1] * 10000)["p999"], 1)
        for unit, scale in [("B", 1), ("K", 1024), ("M", 1024**2), ("G", 1024**3)]:
            self.assertEqual(a.bytes_value("1.5", unit), 1.5 * scale)
        for args in [([], .5), ([1], -1), ([1], 2)]:
            with self.assertRaises(a.InvalidData):
                a.percentile(*args)

    def test_invalid_numbers(self):
        for value in ["", "nan", "inf", "-1", None, True, [], {}]:
            with self.subTest(value=value), self.assertRaises(a.InvalidData):
                a.number(value)
        with self.assertRaises(a.InvalidData):
            a.number(0, positive=True)

    def test_stutter_bursts_and_duration_units(self):
        path = self.root / "stutters.csv"
        path.write_text("timestamp_s,frame_ms\n0.06,60\n0.12,60\n0.13,10\n0.20,70\n")
        result = a.frames(path, .2)
        self.assertEqual(result["stutter_frames_over_50ms"], 3)
        self.assertEqual(result["stutter_events_over_50ms"], 2)
        self.assertEqual(result["stutter_events_per_minute"], 600)
        path = self.root / "gc.log"
        path.write_text(path.read_text().replace("[301.000s]", "[300.090s]"))
        result = self.run_fixture()["metrics"]["gc"]
        self.assertEqual(result["full_gc_count"], 1)
        self.assertEqual(result["collection_count"], 2)
        self.assertEqual(result["total_pause_ms"], 16.5)

    def test_optional_missing_and_invalid_fields(self):
        self.manifest["startupSeconds"] = None
        self.assertIsNone(self.run_fixture()["metrics"]["startup_seconds"])
        self.manifest["startupSeconds"] = {"jvm": 1}
        with self.assertRaises(a.InvalidData):
            self.run_fixture()
        self.manifest["startupSeconds"] = None
        self.manifest["timestamp"] = 123
        with self.assertRaises(a.InvalidData):
            self.run_fixture()

    def test_missing_metadata_and_schema(self):
        original = copy.deepcopy(self.manifest)
        for key in original:
            self.manifest = copy.deepcopy(original)
            del self.manifest[key]
            if key == "startupSeconds":  # Explicitly optional.
                self.run_fixture()
            else:
                with self.subTest(key=key), self.assertRaises((a.InvalidData, ValueError)):
                    self.run_fixture()
        for version in [2, True, "1"]:
            self.manifest = copy.deepcopy(original)
            self.manifest["schemaVersion"] = version
            with self.assertRaises(a.InvalidData):
                self.run_fixture()

    def test_timestamp_formats_and_vendors(self):
        for timestamp in ["2026-10-16T00:00:00Z", "2026-10-15T20:00:00-04:00", "2026-10-16T00:00:00.123+00:00"]:
            for vendor in ["Microsoft", "Eclipse Adoptium", "Oracle"]:
                self.manifest["timestamp"] = timestamp
                self.manifest["runtime"]["vendor"] = vendor
                self.run_fixture()
        for timestamp in ["2026-10-16", "garbage", "2026-10-16T00:00:00"]:
            self.manifest["timestamp"] = timestamp
            with self.assertRaises(ValueError):
                self.run_fixture()

    def test_failed_interrupted_runs_not_summarized(self):
        for status in ["failed", "interrupted"]:
            self.manifest["status"] = status
            with self.assertRaises(a.InvalidData):
                self.run_fixture()
            self.manifest["failureReason"] = "synthetic failure"
            self.assertIsNone(self.run_fixture()["metrics"])
            del self.manifest["failureReason"]

    def test_bad_frame_csvs(self):
        for content in ["", "timestamp_s,wrong\n0.1,10\n", "timestamp_s,frame_ms\n",
                        "timestamp_s,frame_ms\n0.1,nan\n", "timestamp_s,frame_ms\n0.1,0\n",
                        "timestamp_s,frame_ms\n0.1,10\n", "timestamp_s,frame_ms\n0.01,10,extra\n",
                        "timestamp_s,frame_ms\n0.01,10\n0.01,10\n", "timestamp_s,frame_ms\n0.01,10\n"]:
            (self.root / "frames.csv").write_text(content)
            with self.subTest(content=content), self.assertRaises(a.InvalidData):
                self.run_fixture()

    def test_bad_gc_logs(self):
        good = (self.root / "gc.log").read_text()
        for content in ["", good.replace("Using G1", "Using ZGC"), good.replace("5.000ms", "NaNms"),
                        good.replace("[300.025s]", "[not-uptime]"), good.replace("100M->30M(256M)", "100M->30M(20M)"),
                        good + "\n[300.025s][info][gc] GC(0) Pause Young (Normal) (G1 Evacuation Pause) 101M->30M(256M) 5.000ms\n",
                        good.replace("[300.025s]", "[300.001s]"), good + "\nOutOfMemoryError\n"]:
            (self.root / "gc.log").write_text(content)
            with self.subTest(content=content[:40]), self.assertRaises(a.InvalidData):
                self.run_fixture()
        (self.root / "gc.log").write_text("[0.001s][info][gc] Using G1\n")
        self.assertEqual(self.run_fixture()["metrics"]["gc"]["pause_ms"]["count"], 0)

    def test_bad_process_csvs(self):
        for content in ["timestamp_s,cpu_percent_one_core,rss_bytes\n0,1,100\n",
                        "timestamp_s,cpu_percent_one_core,rss_bytes\n0,1,100\n0.05,1,100\n",
                        "timestamp_s,cpu_percent_one_core,rss_bytes\n0,1,100\n0.1,,100\n",
                        "timestamp_s,cpu_percent_one_core,rss_bytes,heap_used_bytes,heap_committed_bytes\n0,1,100,200,100\n0.1,1,100,200,100\n"]:
            (self.root / "process.csv").write_text(content)
            with self.subTest(content=content), self.assertRaises(a.InvalidData):
                self.run_fixture()

    def test_safe_source_paths(self):
        for name in ["../frames.csv", "C:/frames.csv", "/frames.csv", "a\\frames.csv", "./frames.csv"]:
            self.manifest["sources"]["frames"] = name
            with self.assertRaises(a.InvalidData):
                self.run_fixture()

    def test_comparisons_and_noise(self):
        base = self.run_fixture()
        runs = []
        for i in range(5):
            for config, value in [("current", 40), ("candidate", 36 + i)]:
                run = copy.deepcopy(base)
                run.update(configId=config, blockId=f"b{i}", benchmarkId=f"{config}-{i}")
                run["metrics"]["frames_ms"]["p99"] = value
                runs.append(run)
        summary = a.compare(runs, "current")["configurations"]["candidate"]
        self.assertEqual(summary["paired_blocks"], 5)
        self.assertEqual(summary["other_metrics"]["cpu_mean_percent_one_core"]["median_across_runs"], 150)
        self.assertEqual(summary["other_metrics"]["rss_sampled_peak_bytes"]["median_across_runs"], 125829120)
        self.assertEqual(summary["other_metrics"]["main_menu_seconds"]["median_across_runs"], 2)
        self.assertAlmostEqual(summary["paired_p99_percent_change_median"], -5)
        self.assertEqual(a.paired_change_ci([-1, 0, 1, -1, 1]), a.paired_change_ci([-1, 0, 1, -1, 1]))
        self.assertIsNone(a.paired_change_ci([1] * 4))
        for invalid in [runs + [runs[0]], [dict(base, cohort="other"), runs[0]], []]:
            with self.assertRaises(a.InvalidData):
                a.compare(invalid, "current")
        mismatched = copy.deepcopy(runs)
        mismatched[2]["configuration_signature"] = "different"
        with self.assertRaises(a.InvalidData):
            a.compare(mismatched, "current")
        with self.assertRaises(a.InvalidData):
            a.compare(runs, "absent")
        failed = dict(base, status="failed", benchmarkId="failed")
        self.assertEqual(a.compare(runs + [failed], "current")["excluded_runs"], ["failed"])


if __name__ == "__main__":
    unittest.main()
