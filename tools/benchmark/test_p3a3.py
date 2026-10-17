"""Synthetic refusal/regression tests; actual captures are reviewed separately."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zlib

import calibration_review as review
import etw_query
import native_wgl
import readiness_checks as ready
import world_scene as world


def scene():
    return {'generateFeatures': 0, 'conditions': {
        'minecraftVersion': '1.21.11', 'seed': 123456789,
        'generationSettings': {'type': 'minecraft:flat', 'settings': {
            'biome': 'minecraft:plains', 'features': 0, 'lakes': 0,
            'layers': [{'block': 'minecraft:bedrock', 'height': 1},
                       {'block': 'minecraft:dirt', 'height': 128},
                       {'block': 'minecraft:grass_block', 'height': 1}]}},
        'playerPosition': [.5, 66., .5], 'orientation': [0., 10.],
        'gameMode': 'creative', 'difficulty': 'normal', 'weather': 'clear', 'timeOfDay': 6000,
        'renderDistance': 12, 'simulationDistance': 8,
        'entityConditions': {'nonPlayerEntityCount': 0, 'entityTypes': [], 'blockEntityCount': 0},
        'gamerules': {'advance_time': 0, 'advance_weather': 0, 'spawn_mobs': 0, 'random_tick_speed': 3},
        'generatedChunks': {'requiredFullChunks': 1024, 'missingRequiredFullChunks': 0,
                            'requiredChunkRectangle': [-16, -16, 15, 15]},
        'resourcePacks': [], 'shaders': []}, 'options': {
            'enableVsync': 'false', 'fullscreen': 'false', 'maxFps': '260',
            'inactivityFpsLimit': '"minimized"', 'renderClouds': '"false"',
            'overrideWidth': '1920', 'overrideHeight': '1080', 'guiScale': '2',
            'fov': '0.0', 'particles': '0', 'graphicsPreset': '"custom"',
            'ao': 'true', 'cutoutLeaves': 'true', 'improvedTransparency': 'false',
            'mipmapLevels': '4', 'textureFiltering': '0'}}


class WorldTests(unittest.TestCase):
    def test_optional_mods_cannot_claim_shader_absence(self):
        with tempfile.TemporaryDirectory(prefix='aurora-p3a3-synthetic-') as directory:
            root = Path(directory); (root / 'mods').mkdir()
            (root / 'mods/shader-loader.jar').write_bytes(b'explicit synthetic untrusted mod')
            with self.assertRaisesRegex(ValueError, 'empty mods'):
                world.inspect(root, root / 'options', root / 'mods')

    def test_scene_and_required_settings(self):
        self.assertEqual(world.conform_a(scene())['classification'], 'CONFORMING')
        for key in ('seed', 'orientation', 'gamerules', 'generationSettings'):
            s = scene(); del s['conditions'][key]
            with self.assertRaises((KeyError, ValueError)): world.conform_a(s)
        for key in scene()['options']:
            s = scene(); del s['options'][key]
            with self.assertRaises(ValueError): world.conform_a(s)

    def test_entities_camera_chunks_and_terrain_refused(self):
        variants = []
        for key in ('nonPlayerEntityCount', 'blockEntityCount'):
            s = scene(); s['conditions']['entityConditions'][key] = 1; variants.append(s)
        s = scene(); s['conditions']['playerPosition'][0] = .6; variants.append(s)
        s = scene(); s['conditions']['generatedChunks']['missingRequiredFullChunks'] = 1; variants.append(s)
        s = scene(); s['conditions']['generationSettings']['settings']['layers'][1]['height'] = 127; variants.append(s)
        s = scene(); s['options']['inactivityFpsLimit'] = '"afk"'; variants.append(s)
        for s in variants:
            with self.assertRaises(ValueError): world.conform_a(s)

    def test_nbt_truncation_duplicate_trailing_and_decompression_bound(self):
        empty = b'\x0a\x00\x00\x00'
        self.assertEqual(world.Nbt(empty).root(), {})
        for data in (empty[:-1], empty + b'x', b'\x0a\0\0\x01\0\x01a\x01\x01\0\x01a\x02\0'):
            with self.assertRaises(ValueError): world.Nbt(data).root()
        with patch.object(world, 'MAX_NBT', 128):
            with self.assertRaises(ValueError): world.inflate(zlib.compress(b'x' * 129), 15)
        with self.assertRaises(ValueError): world.inflate(zlib.compress(empty)[:-1], 15)

    def test_empty_region_placeholder_and_invalid_sector(self):
        with tempfile.TemporaryDirectory(prefix='aurora-p3a3-synthetic-') as directory:
            path = Path(directory) / 'r.0.0.mca'; path.write_bytes(b'')
            self.assertEqual(list(world.region(path)), [])
            path.write_bytes(b'x')
            with self.assertRaises(ValueError): list(world.region(path))
            data = bytearray(8192); data[:4] = bytes([0, 0, 1, 1]); path.write_bytes(data)
            with self.assertRaises(ValueError): list(world.region(path))

    def test_archive_identity_restore_equivalence_and_scene(self):
        with tempfile.TemporaryDirectory(prefix='aurora-p3a3-synthetic-') as directory:
            root = Path(directory)
            for name in ('snapshot', 'restored'):
                (root / name).mkdir(); (root / name / 'level.dat').write_bytes(b'synthetic placeholder')
            c = scene()['conditions']; c['generatedChunks']['totalRecordedChunks'] = 1024
            (root / 'conditions.json').write_text(json.dumps(c), encoding='utf-8')
            files, dirs = ready.inventory(root / 'snapshot')
            manifest = root / 'snapshot.json'
            manifest.write_text(json.dumps(dict(schemaVersion=1, sampleKind='synthetic', workloadId='A',
                                conditionsSha256=ready.digest(root / 'conditions.json'), files=files, directories=dirs)), encoding='utf-8')
            archive = root / 'archive.tar'; archive.write_bytes(b'explicit synthetic archive identity')
            with patch.object(world, 'inspect', return_value=scene()):
                self.assertEqual(world.restored_check(manifest, archive, ready.digest(archive), root / 'options', root / 'mods')['classification'], 'VALID')
                with self.assertRaises(ValueError): world.restored_check(manifest, archive, '0' * 64, root / 'options', root / 'mods')
                (root / 'restored/level.dat').write_bytes(b'altered')
                with self.assertRaises(ValueError): world.restored_check(manifest, archive, ready.digest(archive), root / 'options', root / 'mods')
            (root / 'restored/level.dat').write_bytes(b'synthetic placeholder')
            bad = scene(); bad['conditions']['entityConditions']['nonPlayerEntityCount'] = 19
            with patch.object(world, 'inspect', side_effect=[scene(), bad]):
                with self.assertRaises(ValueError): world.restored_check(manifest, archive, ready.digest(archive), root / 'options', root / 'mods')


class CalibrationTests(unittest.TestCase):
    def fixture(self):
        manifest = dict(pid=123, creationId='456', qpcFrequencyHz=1000, originQpc=0,
                        windowStartSeconds=1, windowSeconds=2)
        stimulus = dict(pid=123, creationId='456', qpcFrequencyHz=1000, originQpc=0,
                        status='complete', durationSeconds=4)
        markers = [dict(beforeQpc=str(i), afterQpc=str(i + 1)) for i in range(0, 4001, 10)]
        rows = [dict(ProcessID='123', TimeInQPC=str(i + 1)) for i in range(0, 4001, 10)]
        return rows, markers, manifest, stimulus

    def test_marker_identity_clock_and_incomplete_refused(self):
        self.assertEqual(review.reconcile(*self.fixture())['markerConfidence'], 'RECONCILED')
        for field, value in [('pid', 124), ('creationId', '999'), ('qpcFrequencyHz', 1_000_000),
                             ('originQpc', 1), ('status', 'cancelled')]:
            args = self.fixture(); args[3][field] = value
            with self.assertRaises(ValueError): review.reconcile(*args)

    def test_wrong_units_even_when_both_metadata_agree(self):
        args = self.fixture(); args[2]['qpcFrequencyHz'] = args[3]['qpcFrequencyHz'] = 1_000_000
        with self.assertRaises(ValueError): review.reconcile(*args)

    def test_interior_missing_marker_is_not_boundary_allowance(self):
        args = self.fixture(); args[0].pop(200)
        self.assertEqual(review.reconcile(*args)['markerConfidence'], 'REVIEW REQUIRED')

    def test_duplicate_nearest_marker_is_retained(self):
        args = self.fixture(); args[0].insert(201, dict(ProcessID='123', TimeInQPC='2002'))
        self.assertEqual(review.reconcile(*args)['duplicateNearestMarkers'], 1)

    def test_loss_unknown_is_never_zero(self):
        self.assertEqual(review.loss_state([], 100), 'UNKNOWN')
        self.assertEqual(review.loss_state([{'statusCode': 5}], 100), 'UNKNOWN')
        s = dict(statusCode=0, qpcNs=101, eventsLost=0, logBuffersLost=0, realTimeBuffersLost=0)
        self.assertEqual(review.loss_state([s], 100), 'ZERO OBSERVED THROUGH WINDOW')
        self.assertEqual(review.loss_state([s], 102), 'UNKNOWN')
        s['eventsLost'] = 1
        self.assertEqual(review.loss_state([s], 100), 'CONFIRMED LOSS')
        s['eventsLost'] = None
        with self.assertRaises(ValueError): review.loss_state([s], 100)

    def test_plan_metadata_bounds_and_exact_phases(self):
        plan = [dict(label='low', seconds=26, hz=30), dict(label='mid', seconds=26, hz=60), dict(label='high', seconds=26, hz=120)]
        self.assertEqual(native_wgl.validate_plan(plan), 78)
        for bad in ([dict(label='x', seconds=31, hz=30)], [dict(label='x', seconds=20, hz=241)],
                    [plan[0], plan[0]], [dict(label='x', seconds=20, hz=30, staleStall=26)],
                    [dict(label=str(i), seconds=30, hz=30) for i in range(4)]):
            with self.assertRaises(ValueError): native_wgl.validate_plan(bad)

    def test_etw_only_accepts_own_session_namespace(self):
        for name in ('NT Kernel Logger', 'PresentMon', '', 'Aurora-P3A3-../other'):
            with self.assertRaises(ValueError): etw_query.query(name)


if __name__ == '__main__':
    unittest.main()
