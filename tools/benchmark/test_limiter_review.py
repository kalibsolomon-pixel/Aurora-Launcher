"""Synthetic limiter/quality regressions, never empirical Minecraft evidence."""
import copy
import unittest

import limiter_review as r


class LimiterTests(unittest.TestCase):
    def config(self):
        return dict(frameCapFps=500,smoothFramePacer=True,framePacingStrategy='PARK',
                    framePacerSpinThresholdMicros=500,framePacerParkThresholdMicros=2000,
                    framePacerLowCpuMode=False,lowLatencyRender=True,adaptiveRenderSleeping=False,
                    disableVSync=False,guiFpsLimit=60)

    def options(self): return dict(maxFps='260',enableVsync='false',inactivityFpsLimit='"minimized"')

    def test_supported_exact_caps(self):
        for cap in (0,1,240,500,750,1000,2000):
            c=self.config();c['frameCapFps']=cap
            self.assertEqual(r.activation(c,self.options(),cap)['globalCapActive'],cap>0)

    def test_unsupported_values_and_missing_evidence(self):
        for cap in (-1,2001,True,500.5):
            with self.assertRaises(ValueError): r.activation(self.config(),self.options(),cap)
        for key in r.CONFIG_KEYS:
            c=self.config();del c[key]
            with self.assertRaises(ValueError): r.activation(c,self.options(),500)

    def test_native_and_adaptive_confound(self):
        for key,value in [('maxFps','240'),('enableVsync','true'),('inactivityFpsLimit','"afk"')]:
            o=self.options();o[key]=value
            with self.assertRaises(ValueError): r.activation(self.config(),o,500)
        c=self.config();c['adaptiveRenderSleeping']=True
        with self.assertRaises(ValueError): r.activation(c,self.options(),500)

    def test_fallback_and_unknown_strategy(self):
        c=self.config();c['smoothFramePacer']=False
        self.assertEqual(r.activation(c,self.options(),500)['waitStrategy'],'FIXED HYBRID FALLBACK')
        c['framePacingStrategy']='YIELD'
        with self.assertRaises(ValueError):r.activation(c,self.options(),500)

    def test_target_units_and_signed_errors(self):
        a=r.target_intervals([1,2,3],500)
        self.assertEqual(a['idealMs'],2);self.assertEqual(a['observedRowsPerSecond'],500)
        self.assertEqual(a['minimumSignedErrorMs'],-1);self.assertEqual(a['maximumSignedErrorMs'],1)
        for values in ([],[0],[float('nan')]):
            with self.assertRaises(ValueError):r.target_intervals(values,500)

    def test_observer_budget_failure_is_not_outlier_removal(self):
        a=dict(ratePercent=.1,p99Ms=.01,machineCpuPoints=.2)
        b=dict(ratePercent=.1,p99Ms=.01,machineCpuPoints=1.306)
        self.assertFalse(r.observer_screen([a,b])['operationalScreensPass'])
        self.assertFalse(r.observer_screen([a])['equivalenceEstablished'])
        with self.assertRaises(ValueError):r.observer_screen([{}])

    def test_wgl_cannot_certify_minecraft(self):
        with self.assertRaises(ValueError):r.suitability(target_kind='minecraft',independent_kind='controlled-opengl',unique_complete=True)
        self.assertEqual(r.suitability(target_kind='minecraft')['submission'],'DIAGNOSTIC ONLY')

    def test_incomplete_or_censored_cannot_promote(self):
        full=dict(target_kind='minecraft',independent_kind='minecraft',unique_complete=True,
                  observer_bound=True,capture_complete=True,sensitivity_validated=True)
        self.assertFalse(r.suitability(**full)['operatingPointEstablished'])
        for key in ('unique_complete','observer_bound','capture_complete','sensitivity_validated'):
            trial=copy.deepcopy(full);trial[key]=False
            self.assertFalse(r.suitability(**trial)['operatingPointEstablished'])

    def test_canonical_drift(self):
        a=dict(world='1'*64,options='2'*64)
        self.assertTrue(r.canonical_identity(a,a.copy()))
        with self.assertRaises(ValueError):r.canonical_identity(a,dict(world='1'*64,options='3'*64))


if __name__=='__main__':unittest.main()
