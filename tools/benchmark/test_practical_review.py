"""Synthetic arithmetic/integrity regressions; never empirical performance proof."""
import hashlib
import tempfile
import unittest
from pathlib import Path

import collector_review as c
import practical_review as p


class PracticalReviewTests(unittest.TestCase):
    def samples(self):
        return [dict(monotonic_ns=i * 500000000, system_total_ns=i * 14000000000,
                     system_busy_ns=i * 1400000000, game_cpu_ns=i * 1000000000,
                     launcher_cpu_ns=i * 100000000, sampler_cpu_ns=i * 20000000,
                     available_bytes=12 * 1024**3, game_rss_bytes=1024**3) for i in range(3)]

    def test_game_not_mistaken_for_background(self):
        r = p.environment(self.samples())
        self.assertAlmostEqual(r['machineCpuMeanPercent']['total'], 10)
        self.assertAlmostEqual(r['machineCpuMeanPercent']['game'], 100 / 14)
        self.assertAlmostEqual(r['machineCpuMeanPercent']['residual'], 2)
        self.assertFalse(r['unrelatedCpuCertified'])
        self.assertIsNone(r['environmentComparable'])

    def test_weighted_cpu_not_unweighted_average(self):
        rows = self.samples()
        rows[-1]['system_total_ns'] *= 2
        self.assertAlmostEqual(p.environment(rows)['machineCpuMeanPercent']['total'], 5)

    def test_negative_residual_is_retained(self):
        rows = self.samples()
        for row in rows:
            row['system_busy_ns'] = row['game_cpu_ns'] - row['sampler_cpu_ns']
        r = p.environment(rows)
        self.assertLess(r['machineCpuMeanPercent']['residual'], 0)
        self.assertEqual(r['negativeResidualIntervals'], 2)

    def test_missing_and_reversed_counters(self):
        rows = self.samples()
        del rows[1]['game_cpu_ns']
        with self.assertRaises(ValueError): p.environment(rows)
        rows = self.samples()
        rows[-1]['system_busy_ns'] = 0
        with self.assertRaises(ValueError): p.environment(rows)

    def test_memory_pressure(self):
        rows = self.samples()
        rows[1]['available_bytes'] = 5 * 1024**3
        self.assertFalse(p.environment(rows)['memoryReservePassed'])

    def test_impossible_capacity_and_short_capture(self):
        rows = self.samples()
        rows[-1]['system_busy_ns'] = rows[-1]['system_total_ns'] * 2
        with self.assertRaises(ValueError): p.environment(rows)
        with self.assertRaises(ValueError): p.environment(self.samples()[:2])

    def export(self, change=None, metadata_change=None):
        lines = ['# PerformanceOverlay Benchmark', '# Date: 2026-10-17 01:00:00',
                 '# ModVersion: 1.0.2', '# Minecraft: 1.21.11', '# LowMethod: PERCENTILE',
                 '# StutterThresholdMs: 40', '# DurationSec: 30', ','.join(c.COLUMNS),
                 '1,1.000,1000,0,0,0,0,0,0,0,,20,2048',
                 '3,2.000,500,0,0,0,0,0,0,0,,20,2048',
                 '6,3.000,333.3,0,0,0,0,0,0,0,,20,2048',
                 '# EndReason: MANUAL', '# SUMMARY', '# FramesLogged: 3', '# FramesSummary: 3',
                 '# AvgFPS: 500.0', '# Low1FPS: 333.3', '# Low01FPS: 333.3',
                 '# MaxSpikeMs: 3.0', '# Stutters: 0', '# StutterPercent: 0']
        if change: lines = change(lines)
        with tempfile.TemporaryDirectory(prefix='aurora-practical-synthetic-') as directory:
            path = Path(directory) / 'frames.csv'
            path.write_text('\n'.join(lines) + '\n')
            meta = dict(format=c.FORMAT, sourceJarSha256=c.JAR_SHA256, workloadId=c.WORKLOAD,
                        worldArchiveSha256=c.ARCHIVE_SHA256, timestampUnit='ms', frameUnit='ms',
                        rawSha256=hashlib.sha256(path.read_bytes()).hexdigest(), recordingComplete=True,
                        bufferOverflow=False, expectedWindowMs=[0,6], warmupEvidenceVerified=True)
            if metadata_change: meta.update(metadata_change)
            return p.overlay(path, meta)

    def test_real_format_reconciliation_keeps_strict_limits(self):
        r = self.export()
        self.assertEqual(r['status'], 'COHERENT INTERNAL DIAGNOSTIC')
        self.assertEqual(r['strictReview']['metricRole'], 'DIAGNOSTIC ONLY')
        self.assertFalse(r['allRenderedFramesProven'])
        self.assertFalse(r['practicalTimingOverheadKnown'])

    def test_wrong_producer_summary(self):
        r = self.export(lambda lines: [x.replace('AvgFPS: 500.0', 'AvgFPS: 600.0') for x in lines])
        self.assertEqual(r['status'], 'INVALID')

    def test_nonfinite_footer(self):
        r = self.export(lambda lines: [x.replace('MaxSpikeMs: 3.0', 'MaxSpikeMs: NaN') for x in lines])
        self.assertEqual(r['status'], 'INVALID')

    def test_nonfinite_secondary_column(self):
        r = self.export(lambda lines: [x.replace('1000,0,0,0,0', '1000,NaN,0,0,0') for x in lines])
        self.assertEqual(r['status'], 'INVALID')

    def test_incomplete_export_and_wrong_identity(self):
        self.assertEqual(self.export(lambda lines: [x for x in lines if 'FramesSummary' not in x])['status'], 'INVALID')
        self.assertEqual(self.export(metadata_change={'sourceJarSha256':'0'*64})['status'], 'INVALID')

    def test_invalid_window_and_overflow(self):
        self.assertEqual(self.export(metadata_change={'expectedWindowMs':[0,1000]})['status'], 'INVALID')
        self.assertEqual(self.export(metadata_change={'bufferOverflow':True})['status'], 'INVALID')

    def test_auto_duration_must_match_external_window(self):
        r = self.export(lambda lines: [x.replace('EndReason: MANUAL', 'EndReason: AUTO_DURATION') for x in lines])
        self.assertEqual(r['status'], 'INVALID')

    def test_summary_stutter_regression_not_hidden(self):
        r = self.export(lambda lines: [x.replace('Stutters: 0', 'Stutters: 1') for x in lines])
        self.assertEqual(r['status'], 'INVALID')

    def test_variation_is_observed_range(self):
        r = p.variation([100,110,105])
        self.assertAlmostEqual(r['rangePercentOfMedian'], 1000/105)
        self.assertEqual(r['median'],105)

    def test_missing_or_nonfinite_runs(self):
        for values in ([1,2],[1,2,float('nan')],[1,2,0]):
            with self.assertRaises(ValueError): p.variation(values)
