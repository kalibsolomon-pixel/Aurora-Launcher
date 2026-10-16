import copy
import csv
import json
from pathlib import Path
import tempfile
import unittest
import quality as q
import fixture


class QualityTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='aurora-p3a-parser-')
        self.addCleanup(self.temp.cleanup) # Only this test's exclusive disposable root.
        self.root=Path(self.temp.name);self.m=fixture.create(self.root);self.path=self.root/'run.json'

    def run_fixture(self):
        self.path.write_text(json.dumps(self.m));return q.analyze(self.path)

    def rehash(self,role):
        self.m['artifacts'][role]['sha256']=q.sha256(self.root/self.m['artifacts'][role]['path'])

    def frames(self,rows):
        with (self.root/'frames.csv').open('w',newline='') as f:
            w=csv.writer(f);w.writerow(['timestamp','pid']);w.writerows(rows)
        self.rehash('frames')

    def test_exact_known_metrics_and_unknowns(self):
        result=self.run_fixture();self.assertEqual(result['classification'],'VALID WITH WARNINGS')
        self.assertEqual(result['sampleKind'],'synthetic')
        frames=result['metrics']['frames_ms'];self.assertEqual(frames['count'],200)
        for key in ('median','mean','p95','p99'):self.assertAlmostEqual(frames[key],10)
        self.assertAlmostEqual(frames['presentation_rate_hz'],100);self.assertIsNone(frames['p999'])
        gc=result['metrics']['gc'];self.assertEqual(gc['total_pause_ms'],15);self.assertEqual(gc['full_gc_count'],1)
        process=result['metrics']['process'];self.assertEqual(process['cpu_percent_one_core_mean'],150)
        self.assertIsNone(process['heap_used_bytes']);self.assertIsNone(process['paging_bytes_per_second'])
        self.assertFalse(result['metrics']['clock']['causalityEstablished'])

    def test_valid_without_warnings_at_sufficient_sample_depth(self):
        self.frames((i/10000,123) for i in range(20001))
        r=self.run_fixture();self.assertEqual(r['classification'],'VALID');self.assertIsNotNone(r['metrics']['frames_ms']['p999'])

    def test_percentile_definition_and_tail_depth(self):
        self.assertEqual(q.p1.percentile([1,2,3,1000],.5),2.5)
        self.assertEqual(q.p1.distribution([1]*10000)['p999'],1)
        self.assertIsNone(q.p1.distribution([1]*9999)['p999'])

    def test_units_are_explicit(self):
        self.m['capture']['units']='ms';self.frames((i*10,123) for i in range(201))
        self.assertAlmostEqual(self.run_fixture()['metrics']['frames_ms']['mean'],10)
        self.m['capture']['units']='ns';self.frames((i*10_000_000,123) for i in range(201))
        self.assertAlmostEqual(self.run_fixture()['metrics']['frames_ms']['mean'],10)
        self.m['capture']['units']='us';self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_duplicate_nonmonotonic_malformed_and_impossible_timestamps(self):
        for times in ([0,1,1,2],[0,1,.5,2],[0,'NaN',2],[0,-1,2],[0,'inf',2],[0,'bad',2]):
            with self.subTest(times=times):
                self.frames((t,123) for t in times);self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_hash_tampering_is_invalid(self):
        with (self.root/'frames.csv').open('a') as f:f.write('3,123\n')
        r=self.run_fixture();self.assertEqual(r['classification'],'INVALID');self.assertIn('artifact hash mismatch',r['reasons'])

    def test_missing_capture_is_incomplete_not_discarded(self):
        (self.root/'gc.log').unlink()
        r=self.run_fixture();self.assertEqual(r['classification'],'INCOMPLETE');self.assertEqual(r['runId'],self.m['runId'])

    def test_empty_capture_is_invalid(self):
        (self.root/'frames.csv').write_text('timestamp,pid\n');self.rehash('frames')
        self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_partial_capture_and_insufficient_counts(self):
        self.frames((i/100,123) for i in range(101))
        self.assertEqual(self.run_fixture()['classification'],'INCOMPLETE')
        self.frames([(0,123),(1,123),(2,123)])
        self.assertIn('insufficient frame samples',self.run_fixture()['reasons'])

    def test_warmup_and_interrupted_runs(self):
        self.m['warmupSeconds']=299;self.assertEqual(self.run_fixture()['classification'],'INCOMPLETE')
        self.m['warmupSeconds']=300;self.m['status']='interrupted';self.m['failureReason']='fixture interruption'
        self.assertEqual(self.run_fixture()['classification'],'INCOMPLETE')
        del self.m['failureReason'];self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_missing_metadata_and_unknown_schema(self):
        original=copy.deepcopy(self.m)
        for key in ('runtime','runId','artifacts','hardware','jvmArguments'):
            self.m=copy.deepcopy(original);del self.m[key]
            self.assertEqual(self.run_fixture()['classification'],'INVALID')
        self.m=original;self.m['schemaVersion']=2;self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_impossible_hardware_render_and_nonfinite_metadata(self):
        original=copy.deepcopy(self.m)
        for section,key,value in [('hardware','ramBytes',0),('hardware','ramBytes',True),('graphics','width',-1),('graphics','refreshHz',float('inf')),('environment','acConnected','yes'),('environment','extra',float('nan'))]:
            self.m=copy.deepcopy(original);self.m[section][key]=value
            self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_secret_fields_and_private_paths_rejected_without_echo(self):
        self.m['environment']['accessToken']='fixture-secret-never-echo'
        r=self.run_fixture();self.assertEqual(r['classification'],'INVALID');self.assertNotIn('fixture-secret',json.dumps(r))
        del self.m['environment']['accessToken'];self.m['jvmArguments']=['-Dpath=C:/Users/private']
        r=self.run_fixture();self.assertEqual(r['classification'],'INVALID');self.assertNotIn('Users/private',json.dumps(r))

    def test_rejected_startup_and_warning_text_never_echo_raw_input(self):
        self.m['startupSeconds']['private']='fixture-secret-never-echo'
        r=self.run_fixture();self.assertEqual(r['classification'],'INVALID');self.assertNotIn('fixture-secret',json.dumps(r))
        del self.m['startupSeconds']['private'];self.m['dataQualityWarnings']=['email=fixture-secret-never-echo']
        r=self.run_fixture();self.assertEqual(r['classification'],'INVALID');self.assertNotIn('fixture-secret',json.dumps(r))

    def test_traversal_and_link_escape(self):
        for name in ('../outside','/absolute','C:/drive','back\\slash','a//b','./frames.csv'):
            self.m['artifacts']['frames']['path']=name
            self.assertEqual(self.run_fixture()['classification'],'INVALID')
        # Resolve checks also cover junction/symlink escape without creating privileged links.
        with self.assertRaises(q.Invalid):q.safe_path(self.root,'../escape')

    def test_gc_header_unsupported_collector_and_malformed_pause(self):
        for log in ('','[0.0s][info][gc] Using ZGC\n','[0.0s][info][gc] Using G1\n[301.0s][info][gc] GC(0) Pause broken\n'):
            (self.root/'gc.log').write_text(log);self.rehash('gc')
            self.assertEqual(self.run_fixture()['classification'],'INVALID')
        (self.root/'gc.log').write_text('[0.0s][info][gc] Using G1\n');self.rehash('gc')
        self.assertEqual(self.run_fixture()['metrics']['gc']['total_pause_ms'],0)

    def test_realistic_gc_shrink_and_rounded_cpu(self):
        (self.root/'gc.log').write_text('[0.0s][info][gc] Using G1\n[301.0s][info][gc] GC(0) Pause Full (System.gc()) 25M->2M(14M) 3.514ms\n[301.0s][info][gc,cpu] GC(0) User=0.00s Sys=0.05s Real=0.00s\n')
        self.rehash('gc');r=self.run_fixture();self.assertNotEqual(r['classification'],'INVALID')
        self.assertEqual(r['metrics']['gc']['max_post_gc_committed_bytes'],14*1024**2)
        self.assertEqual(r['metrics']['gc']['gc_logged_phase_cpu_seconds'],.05)
        (self.root/'gc.log').write_text('[0.0s][info][gc] Using G1\n[301.0s][info][gc] GC(0) Pause Full (System.gc()) 25M->20M(14M) 3.514ms\n')
        self.rehash('gc');self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_process_mismatch_and_counter_regression(self):
        self.frames([(i/100,999) for i in range(201)])
        self.assertEqual(self.run_fixture()['classification'],'INVALID')
        self.frames([(i/100,123) for i in range(201)])
        text=(self.root/'process.csv').read_text().replace('fixture-creation','wrong-creation')
        (self.root/'process.csv').write_text(text);self.rehash('process')
        self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_process_partial_and_sampling_gap(self):
        lines=(self.root/'process.csv').read_text().splitlines()
        (self.root/'process.csv').write_text('\n'.join(lines[:-1])+'\n');self.rehash('process')
        self.assertEqual(self.run_fixture()['classification'],'INCOMPLETE')
        self.m['qualityPolicy']['maxProcessIntervalSeconds']=.1
        self.assertIn('process sampling gap',self.run_fixture()['reasons'])

    def test_process_endpoint_overshoot_is_clipped_without_losing_last_interval(self):
        path=self.root/'process.csv';rows=list(csv.DictReader(path.open()))
        rows[-1]['monotonic_ns']=str(int(rows[-1]['monotonic_ns'])+1_000_000)
        with path.open('w',newline='') as f:
            writer=csv.DictWriter(f,fieldnames=list(rows[0]));writer.writeheader();writer.writerows(rows)
        self.rehash('process');r=self.run_fixture()
        self.assertTrue(r['metrics']['process']['boundary_coverage']);self.assertEqual(r['metrics']['process']['covered_seconds'],2)

    def test_clock_alignment_drift_and_uncertainty(self):
        mapping=self.m['clocks']['gcUptimeToRun'];f,drift,uncertainty=q.clock_mapping(mapping)
        self.assertEqual(f(301),1);self.assertEqual(drift,0);self.assertEqual(uncertainty,1)
        mapping['run1']=2.01;self.assertEqual(self.run_fixture()['classification'],'INVALID')
        mapping['run1']=2;mapping['uncertaintyMs']=10
        self.assertIn('clock uncertainty exceeds 5 ms; proximity is coarse',self.run_fixture()['warnings'])

    def test_correlation_does_not_imply_cause_or_match_distant_event(self):
        self.frames([(0,123),(.1,123),(.2,123),(.5,123),(1,123),(1.5,123),(2,123)])
        r=self.run_fixture();near=r['metrics']['gc_stutter_proximity']
        self.assertFalse(near['causalityEstablished']);self.assertLess(near['stutter_intervals_near_gc'],7)

    def presentmon_fixture(self):
        origin=10**18
        self.m['capture'].update(tool='PresentMon',version='2.3.1',format='presentmon-2.3.1-default-qpc',frameSemantic='etw-display-estimate',qpcFrequencyHz=10_000_000,originQpc=origin,swapChain='0x1')
        with (self.root/'frames.csv').open('w',newline='') as f:
            writer=csv.writer(f);writer.writerow(['ProcessID','SwapChainAddress','CPUStartQPC','DisplayLatency','DisplayedTime'])
            writer.writerows([123,'0x1',origin+i*100_000,0,10] for i in range(201))
        self.rehash('frames');return origin

    def test_presentmon_qpc_precision_and_semantics(self):
        self.presentmon_fixture();r=self.run_fixture();self.assertEqual(r['classification'],'VALID WITH WARNINGS')
        self.assertAlmostEqual(r['metrics']['frames_ms']['mean'],10)
        self.assertEqual(r['metrics']['frames_ms']['raw_rows'],201)
        self.assertEqual(r['metrics']['frames_ms']['semantic'],'etw-display-estimate')

    def test_presentmon_drops_retained_in_denominator(self):
        self.presentmon_fixture();path=self.root/'frames.csv'
        lines=path.read_text().splitlines();lines[51]=lines[51].rsplit(',',1)[0]+',NA'
        path.write_text('\n'.join(lines)+'\n');self.rehash('frames')
        r=self.run_fixture();self.assertEqual(r['metrics']['frames_ms']['undisplayed_rows'],1)
        self.assertEqual(r['metrics']['frames_ms']['raw_rows'],201)

    def test_presentmon_identity_chain_and_version_rejection(self):
        self.presentmon_fixture();self.m['capture']['swapChain']='0x2'
        self.assertEqual(self.run_fixture()['classification'],'INVALID')
        self.m['capture']['swapChain']='0x1';self.m['capture']['version']='latest'
        self.assertEqual(self.run_fixture()['classification'],'INVALID')

    def test_cohort_locks_and_duplicates(self):
        self.run_fixture();second=self.root/'second.json';other=copy.deepcopy(self.m);other['runId']='synthetic-parser-2';other['runtime']['build']='changed+2'
        second.write_text(json.dumps(other));self.assertTrue(all(r['classification']=='INVALID' for r in q.cohort_results([self.path,second])))
        other=copy.deepcopy(self.m);second.write_text(json.dumps(other))
        self.assertTrue(all(r['classification']=='INVALID' for r in q.cohort_results([self.path,second])))

    def test_changed_mods_render_and_workload_cannot_pool(self):
        self.run_fixture()
        for key,value in [('mods',[{'id':'fixture','version':'1','algorithm':'sha256','digest':'a'*64}]),('workloadId','other'),('graphics',{**self.m['graphics'],'renderDistance':24})]:
            other=copy.deepcopy(self.m);other['runId']='other';other[key]=value
            second=self.root/'second.json';second.write_text(json.dumps(other))
            self.assertTrue(all(r['classification']=='INVALID' for r in q.cohort_results([self.path,second])))

    def test_run_specific_origins_and_preflight_hashes_do_not_change_cohort(self):
        self.presentmon_fixture();self.run_fixture();other=copy.deepcopy(self.m)
        other['runId']='second';other['blockId']='block-2';other['capture']['originQpc']+=0
        other['environment']['quietPreflightSha256']='b'*64
        second=self.root/'second.json';second.write_text(json.dumps(other))
        self.assertTrue(all(r['classification'].startswith('VALID') for r in q.cohort_results([self.path,second])))

    def test_config_vector_changes_and_duplicate_blocks_are_invalid(self):
        self.run_fixture();other=copy.deepcopy(self.m);other['runId']='second';other['blockId']='block-2';other['jvmArguments']=['-Xmx1536m']
        second=self.root/'second.json';second.write_text(json.dumps(other))
        self.assertTrue(all(r['classification']=='INVALID' for r in q.cohort_results([self.path,second])))
        other['jvmArguments']=self.m['jvmArguments'];other['blockId']=self.m['blockId'];second.write_text(json.dumps(other))
        self.assertTrue(all(r['classification']=='INVALID' for r in q.cohort_results([self.path,second])))

    def test_measured_cannot_relabel_fixture_or_weaken_protocol(self):
        self.m['sampleKind']='measured';self.assertEqual(self.run_fixture()['classification'],'INVALID')
        self.m['measurementSeconds']=600;self.m['qualityPolicy']['minimumFrames']=1000
        self.assertEqual(self.run_fixture()['classification'],'INVALID')


if __name__=='__main__':unittest.main()
