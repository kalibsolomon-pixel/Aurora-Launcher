"""Fail-closed marker validation and interval accounting regression checks."""
import copy
import unittest
from analyze import validate_markers, union_ms, seconds, stats, owner

class AnalysisTests(unittest.TestCase):
    def setUp(self):
        self.valid = dict(menuReady=True, records=[
            dict(boundary='jna.isolation-confirmed', edge=0, ns=10),
            dict(boundary='menu.usable', edge=0, ns=20)])

    def test_valid(self):
        self.assertEqual(len(validate_markers(self.valid)),2)

    def test_missing_duplicate_or_failed_menu_rejected(self):
        cases=[]
        a=copy.deepcopy(self.valid); a['records'].pop(); cases.append(a)
        a=copy.deepcopy(self.valid); a['records'].append(a['records'][-1]); cases.append(a)
        a=copy.deepcopy(self.valid); a['menuReady']=False; cases.append(a)
        a=copy.deepcopy(self.valid); a['records'].append(dict(boundary='error.jna-probe')); cases.append(a)
        a=copy.deepcopy(self.valid); a['records'].pop(0); cases.append(a)
        for a in cases:
            with self.assertRaises(AssertionError): validate_markers(a)

    def test_overlap_not_added_twice(self):
        self.assertEqual(union_ms([(1,3),(2,4),(2.5,3),(5,6)]),4000)

    def test_duration_and_sample_deviation(self):
        self.assertEqual(seconds('PT1M2.5S'),62.5)
        self.assertEqual(stats([1,2,3])['sampleSd'],1)

    def test_verified_injected_method_owner(self):
        self.assertEqual(owner('net.minecraft.client.renderer.texture.SpriteContents.scanSpriteContents'),'Sodium (injected)')
        self.assertEqual(owner('net.minecraft.client.Minecraft.handler$example$aurora$preciseLimit'),'Aurora')

if __name__=='__main__': unittest.main()
