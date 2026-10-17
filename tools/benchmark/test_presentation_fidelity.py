"""Explicit synthetic mutations of a retained real specimen; not empirical proof."""
import unittest

import native_wgl
import presentation_fidelity as fidelity
from test_short_calibration import ShortCalibrationTests


class FidelityTests(unittest.TestCase):
    setUp = ShortCalibrationTests.setUp
    mutate = ShortCalibrationTests.mutate

    def test_all_display_unavailable_keeps_submission(self):
        for r in self.rows: r['MsUntilDisplayed'] = r['MsBetweenDisplayChange'] = 'NA'
        self.mutate(0, 'MsUntilDisplayed', 'NA')
        result = fidelity.inspect(self.root, self.m)
        self.assertEqual(result['quality'], 'VALID WITH WARNINGS')
        self.assertIsNone(result['displayEstimateIntervals'])
        self.assertEqual(result['lossStatus'], 'UNKNOWN')
        self.assertIsNone(result['fields']['MsBetweenDisplayChange']['distribution'])

    def test_submission_and_reported_intervals_agree(self):
        r = fidelity.inspect(self.root, self.m)
        self.assertLess(r['exportedIntervalMaxDisagreementMs'], .0002)
        self.assertEqual(r['metricClassification']['physicalDisplay'], 'UNAVAILABLE')
        self.assertIsNone(r['uncertainty']['physicalScanoutMs'])
        self.assertFalse(any('stutter' in k for k in r['submissionIntervals']))

    def test_wrong_interval_and_nonfinite_diagnostics_refused(self):
        self.mutate(5, 'MsBetweenPresents', '999')
        with self.assertRaises(ValueError): fidelity.inspect(self.root, self.m)

    def test_signed_diagnostics_zero_and_null_are_distinct(self):
        self.mutate(5, 'MsAnimationError', '-2.5')
        result = fidelity.inspect(self.root, self.m)
        self.assertGreater(result['fields']['MsAnimationError']['distribution']['variance'], 0)
        self.assertEqual(result['fields']['MsInPresentAPI']['zero'], len(self.rows))
        self.assertIsNone(result['fields']['MsPCLatency']['distribution'])
        self.mutate(5, 'MsGPUTime', 'Infinity')
        with self.assertRaises(ValueError): fidelity.inspect(self.root, self.m)

    def test_wrong_identity_frequency_order_and_claim(self):
        identity = dict(pid=self.m['pid'], creationId=self.m['creationId'])
        self.assertTrue(fidelity.inspect(self.root,self.m,identity=identity)['independentCreationIdentityMatched'])
        identity['creationId'] = '999'
        with self.assertRaises(ValueError): fidelity.inspect(self.root,self.m,identity=identity)
        for key,value in [('pid',999),('qpcFrequencyHz',0),('qpcFrequencyHz',1),('qpcFrequencyHz',True)]:
            original=self.m[key];self.m[key]=value
            with self.assertRaises(ValueError): fidelity.inspect(self.root,self.m)
            self.m[key]=original
        with self.assertRaises(ValueError): fidelity.inspect(self.root,self.m,claim='physical-display')
        self.mutate(5,'TimeInQPC',self.rows[4]['TimeInQPC'])
        with self.assertRaises(ValueError): fidelity.inspect(self.root,self.m)

    def test_incomplete_status_boundaries_and_lifetime(self):
        self.m['status']='interrupted'
        self.assertEqual(fidelity.inspect(self.root,self.m)['quality'],'INCOMPLETE')
        self.m['windowStartSeconds']=19.5
        with self.assertRaises(ValueError): fidelity.inspect(self.root,self.m)

    def test_loss_zero_unknown_and_nonzero(self):
        end=int((self.m['originQpc']/self.m['qpcFrequencyHz']+1.25)*1e9)
        s=dict(statusCode=0,qpcNs=end+10000,eventsLost=0,logBuffersLost=0,realTimeBuffersLost=0)
        self.assertEqual(fidelity.inspect(self.root,self.m,[s])['lossStatus'],'ZERO OBSERVED THROUGH WINDOW')
        s['eventsLost']=1
        self.assertEqual(fidelity.inspect(self.root,self.m,[s])['lossStatus'],'CONFIRMED LOSS')
        s['statusCode']=5
        self.assertEqual(fidelity.inspect(self.root,self.m,[s])['lossStatus'],'UNKNOWN')

    def test_high_rate_requires_explicit_opt_in(self):
        p=[dict(label='high',seconds=10,hz=1800)]
        with self.assertRaises(ValueError): native_wgl.validate_plan(p)
        self.assertEqual(native_wgl.validate_plan(p,high_rate=True),10)
        p[0]['hz']=4001
        with self.assertRaises(ValueError): native_wgl.validate_plan(p,high_rate=True)

    def test_malformed_external_evidence_is_rejected(self):
        for identity in ([], {'pid': True, 'creationId': self.m['creationId']}):
            with self.assertRaises(ValueError): fidelity.inspect(self.root,self.m,identity=identity)
        for snapshots in ({}, [None], [dict(statusCode=0,qpcNs=None,eventsLost=0,logBuffersLost=0,realTimeBuffersLost=0)]):
            with self.assertRaises(ValueError): fidelity.inspect(self.root,self.m,snapshots)
        mixed=[dict(statusCode=5),dict(statusCode=0,qpcNs=1,eventsLost=1,logBuffersLost=0,realTimeBuffersLost=0)]
        self.assertEqual(fidelity.inspect(self.root,self.m,mixed)['lossStatus'],'CONFIRMED LOSS')
        self.assertEqual(fidelity.inspect(self.root,self.m,mixed)['quality'],'INCOMPLETE')


del ShortCalibrationTests

if __name__ == '__main__': unittest.main()
