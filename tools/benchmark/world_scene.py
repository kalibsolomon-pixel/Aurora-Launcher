"""Bounded, read-only Minecraft 1.21.11 NBT scene inspection.

Never creates, edits or restores world files. Inventory equivalence and archive
identity are separate gates; passing NBT inspection does not establish provenance.
"""
import argparse
import json
from pathlib import Path
import struct
import zlib

from readiness_checks import plain, require, digest, world_check

MAX_NBT = 64 * 1024**2


def inflate(data, mode):
    decoder = zlib.decompressobj(mode)
    result = decoder.decompress(data, MAX_NBT + 1)
    require(len(result) <= MAX_NBT and decoder.eof and not decoder.unconsumed_tail
            and not decoder.unused_data, 'invalid or oversized compressed NBT')
    return result


class Nbt:
    def __init__(self, data):
        require(len(data) <= MAX_NBT, 'NBT size bound')
        self.data, self.pos, self.count = data, 0, 0

    def take(self, size):
        require(0 <= size <= len(self.data) - self.pos, 'truncated NBT')
        result = self.data[self.pos:self.pos + size]
        self.pos += size
        return result

    def number(self, fmt):
        return struct.unpack('>' + fmt, self.take(struct.calcsize(fmt)))[0]

    def string(self):
        return self.take(self.number('H')).decode('utf-8')

    def value(self, tag, depth=0):
        self.count += 1
        require(depth < 64 and self.count < 2_000_000, 'NBT work bound')
        if tag in (1, 2, 3, 4, 5, 6):
            return self.number({1: 'b', 2: 'h', 3: 'i', 4: 'q', 5: 'f', 6: 'd'}[tag])
        if tag == 8:
            return self.string()
        if tag in (7, 11, 12):
            size = self.number('i')
            require(0 <= size <= 16_000_000, 'NBT array bound')
            if tag == 7:
                return self.take(size)
            return [self.number('i' if tag == 11 else 'q') for _ in range(size)]
        if tag == 9:
            subtype, size = self.number('B'), self.number('i')
            require(0 <= size < 1_000_000 and (1 <= subtype <= 12 or size == 0), 'NBT list bound')
            return [self.value(subtype, depth + 1) for _ in range(size)]
        if tag == 10:
            result = {}
            while True:
                subtype = self.number('B')
                if subtype == 0:
                    return result
                name = self.string()
                require(name not in result, 'duplicate NBT field')
                result[name] = self.value(subtype, depth + 1)
        raise ValueError('unsupported NBT tag')

    def root(self):
        require(self.number('B') == 10, 'NBT root must be compound')
        self.string()
        result = self.value(10)
        require(self.pos == len(self.data), 'trailing NBT bytes')
        return result


def read_bytes(path, maximum):
    path = Path(path).absolute()
    plain(path)
    require(path.is_file() and path.stat().st_size <= maximum, 'NBT input bound')
    return path.read_bytes()


def level(path):
    return Nbt(inflate(read_bytes(path, MAX_NBT), 31)).root()['Data']


def region(path):
    data = read_bytes(path, 128 * 1024**2)
    # The installed game leaves zero-byte entity-region placeholders after save.
    if not data:
        return
    require(len(data) >= 8192 and len(data) % 4096 == 0, 'region size')
    occupied = {0, 1}
    for index in range(1024):
        entry = data[index * 4:index * 4 + 4]
        sector, count = int.from_bytes(entry[:3], 'big'), entry[3]
        if sector == 0:
            require(count == 0, 'invalid empty sector')
            continue
        require(sector >= 2 and count > 0 and (sector + count) * 4096 <= len(data), 'region sector bounds')
        sectors = set(range(sector, sector + count))
        require(not occupied.intersection(sectors), 'overlapping region sectors')
        occupied.update(sectors)
        offset = sector * 4096
        length = int.from_bytes(data[offset:offset + 4], 'big')
        require(1 < length <= count * 4096 - 4, 'region payload bounds')
        compression, payload = data[offset + 4], data[offset + 5:offset + 4 + length]
        if compression in (1, 2):
            payload = inflate(payload, 31 if compression == 1 else 15)
        else:
            require(compression == 3, 'unsupported region compression')
        yield Nbt(payload).root()


def inspect(world, options, mods_root):
    world = Path(world).absolute()
    mods_root = Path(mods_root).absolute()
    plain(mods_root)
    require(mods_root.is_dir() and not any(mods_root.iterdir()), 'Workload A requires an empty mods directory')
    data = level(world / 'level.dat')
    player, generation, rules = data['Player'], data['WorldGenSettings'], data['game_rules']
    chunks, entities, block_entities = {}, [], []
    # Include all dimensions in the entity/block-entity count, not just loaded chunks.
    for path in sorted(world.rglob('*.mca')):
        if path.parent.name not in ('region', 'entities'):
            continue
        for chunk in region(path):
            if path.parent.name == 'entities':
                entities.extend(chunk.get('Entities', []))
            else:
                block_entities.extend(chunk.get('block_entities', []))
                if path.parent == world / 'region':
                    key = (chunk['xPos'], chunk['zPos'])
                    require(key not in chunks, 'duplicate world chunk')
                    chunks[key] = chunk['Status']
    values = {}
    for line in read_bytes(options, 1024**2).decode('utf-8-sig').splitlines():
        if ':' in line:
            key, value = line.split(':', 1)
            require(key not in values, 'duplicate option')
            values[key] = value
    missing = sum(chunks.get((x, z)) not in ('full', 'minecraft:full')
                  for x in range(-16, 16) for z in range(-16, 16))
    conditions = dict(minecraftVersion=data['Version']['Name'], seed=generation['seed'],
                      generationSettings=generation['dimensions']['minecraft:overworld']['generator'],
                      playerPosition=player['Pos'], orientation=player['Rotation'],
                      gameMode='creative' if data['GameType'] == player['playerGameType'] == 1 else 'unexpected',
                      difficulty='normal' if data['Difficulty'] == 2 else 'unexpected',
                      weather='clear' if not data['raining'] and not data['thundering'] else 'unexpected',
                      timeOfDay=data['DayTime'] % 24000, renderDistance=int(values['renderDistance']),
                      simulationDistance=int(values['simulationDistance']),
                      entityConditions={'nonPlayerEntityCount': len(entities),
                                        'entityTypes': sorted({e.get('id', 'unknown') for e in entities}),
                                        'blockEntityCount': len(block_entities)},
                      resourcePacks=json.loads(values['resourcePacks']), shaders=[],
                      gamerules={k: rules['minecraft:' + k] for k in
                                 ('advance_time', 'advance_weather', 'spawn_mobs', 'random_tick_speed')},
                      generatedChunks={'totalRecordedChunks': len(chunks), 'requiredFullChunks': 1024,
                                       'missingRequiredFullChunks': missing,
                                       'requiredChunkRectangle': [-16, -16, 15, 15]})
    return {'conditions': conditions, 'options': values, 'generateFeatures': generation['generate_features'],
            'gameRules': rules}


def conform_a(scene):
    c = scene['conditions']
    require(c['minecraftVersion'] == '1.21.11' and c['seed'] == 123456789, 'world identity')
    require(c['playerPosition'] == [.5, 66., .5] and c['orientation'] == [0., 10.], 'camera mismatch')
    require(c['gameMode'] == 'creative' and c['difficulty'] == 'normal'
            and c['weather'] == 'clear' and c['timeOfDay'] == 6000, 'scene settings')
    generator = c['generationSettings']
    require(generator['type'] == 'minecraft:flat' and scene['generateFeatures'] == 0, 'generator mismatch')
    settings = generator['settings']
    require(settings['biome'] == 'minecraft:plains' and settings['features'] == 0 and settings['lakes'] == 0
            and settings['layers'] == [{'block': 'minecraft:bedrock', 'height': 1},
                                       {'block': 'minecraft:dirt', 'height': 128},
                                       {'block': 'minecraft:grass_block', 'height': 1}], 'flat terrain mismatch')
    require(c['renderDistance'] == 12 and c['simulationDistance'] == 8, 'chunk distances')
    require(c['entityConditions'] == {'nonPlayerEntityCount': 0, 'entityTypes': [], 'blockEntityCount': 0}, 'unexpected entities')
    require(c['gamerules'] == {'advance_time': 0, 'advance_weather': 0, 'spawn_mobs': 0, 'random_tick_speed': 3}, 'gamerules mismatch')
    require(c['generatedChunks']['missingRequiredFullChunks'] == 0
            and c['generatedChunks']['requiredFullChunks'] == 1024
            and c['generatedChunks']['requiredChunkRectangle'] == [-16, -16, 15, 15], 'chunk coverage')
    require(c['resourcePacks'] == [] and c['shaders'] == [], 'packs or shaders')
    for key, expected in {'enableVsync': 'false', 'fullscreen': 'false', 'maxFps': '260',
                          'inactivityFpsLimit': '"minimized"', 'renderClouds': '"false"',
                          'overrideWidth': '1920', 'overrideHeight': '1080',
                          'guiScale': '2', 'fov': '0.0', 'particles': '0',
                          'graphicsPreset': '"custom"', 'ao': 'true', 'cutoutLeaves': 'true',
                          'improvedTransparency': 'false', 'mipmapLevels': '4',
                          'textureFiltering': '0'}.items():
        require(scene['options'].get(key) == expected, 'missing or altered graphics option: ' + key)
    return {'classification': 'CONFORMING', 'scope': 'persisted A scene; live focus/framebuffer/provenance require review'}


def restored_check(manifest, archive, expected_archive, options, mods_root):
    require(digest(Path(archive).absolute()) == expected_archive, 'archive digest mismatch')
    result = world_check(Path(manifest))
    root = Path(manifest).absolute().parent
    scenes = [inspect(root / name, options, mods_root) for name in ('snapshot', 'restored')]
    require(scenes[0] == scenes[1], 'restored scene mismatch')
    for scene in scenes:
        conform_a(scene)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--expected-archive-sha256', required=True)
    parser.add_argument('--options', type=Path, required=True)
    parser.add_argument('--mods', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = restored_check(args.manifest, args.archive, args.expected_archive_sha256, args.options, args.mods)
        result['sceneClassification'] = 'CONFORMING'
    except (ValueError, KeyError, TypeError, OSError, OverflowError, RecursionError, zlib.error):
        result = {'classification': 'INVALID', 'reason': 'archive, restored inventory or A scene failed validation'}
    print(json.dumps(result, indent=2))
    return 0 if result['classification'] == 'VALID' else 2


if __name__ == '__main__':
    raise SystemExit(main())
