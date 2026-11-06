#!/usr/bin/env python3
"""What the files of a test world hold of some items, read before it is reopened (no game running).

`count(folder, prefixes)` sums the items whose id starts with one of `prefixes` in the overworld's entity files (item
entities), its chunk records (block entities' `Items`, and the rods and slots of an NTM machine's model in its `ntm`
tag) and the owner's player file: what a vanilla load of those files can hold.
"""
import gzip
import json
import struct
import zlib
from pathlib import Path


def _nbt(data, at, kind):
    """The value of a tag of `kind` at `at`, and where it ends."""
    if kind == 1:
        return struct.unpack_from('>b', data, at)[0], at + 1
    if kind == 2:
        return struct.unpack_from('>h', data, at)[0], at + 2
    if kind == 3:
        return struct.unpack_from('>i', data, at)[0], at + 4
    if kind == 4:
        return struct.unpack_from('>q', data, at)[0], at + 8
    if kind == 5:
        return struct.unpack_from('>f', data, at)[0], at + 4
    if kind == 6:
        return struct.unpack_from('>d', data, at)[0], at + 8
    if kind in (7, 11, 12):
        n = struct.unpack_from('>i', data, at)[0]
        fmt, size = {7: ('b', 1), 11: ('i', 4), 12: ('q', 8)}[kind]
        return list(struct.unpack_from(f'>{n}{fmt}', data, at + 4)), at + 4 + n * size
    if kind == 8:
        n = struct.unpack_from('>H', data, at)[0]
        return data[at + 2:at + 2 + n].decode('utf-8', 'replace'), at + 2 + n
    if kind == 9:
        inner, n = data[at], struct.unpack_from('>i', data, at + 1)[0]
        at += 5
        out = []
        for _ in range(max(n, 0)):
            v, at = _nbt(data, at, inner)
            out.append(v)
        return out, at
    if kind == 10:
        out = {}
        while True:
            inner = data[at]
            at += 1
            if inner == 0:
                return out, at
            n = struct.unpack_from('>H', data, at)[0]
            name = data[at + 2:at + 2 + n].decode('utf-8', 'replace')
            out[name], at = _nbt(data, at + 2 + n, inner)
    raise ValueError(f'tag {kind}')


def parse(data):
    """A named root compound."""
    if data[0] != 10:
        raise ValueError('not a compound')
    n = struct.unpack_from('>H', data, 1)[0]
    return _nbt(data, 3 + n, 10)[0]


def records(path):
    """Every chunk record of a region file, parsed."""
    data = path.read_bytes()
    for i in range(1024):
        offset = int.from_bytes(data[i * 4:i * 4 + 3], 'big') * 4096
        if offset == 0 or offset + 5 > len(data):
            continue
        length = int.from_bytes(data[offset:offset + 4], 'big')
        method, raw = data[offset + 4], data[offset + 5:offset + 4 + length]
        if method == 2:
            raw = zlib.decompress(raw)
        elif method == 1:
            raw = gzip.decompress(raw)
        elif method != 3:
            continue
        yield parse(raw)


def _stack(s, prefixes):
    if not isinstance(s, dict):
        return 0
    item = s.get('id') or s.get('Id') or ''
    if not isinstance(item, str) or not item.startswith(prefixes):
        return 0
    return int(s.get('count', s.get('Count', 1)) or 0)


def _model(text, prefixes):
    """The items an NTM machine's model holds: slot stacks, and a pile's fuel rods (one item each)."""
    try:
        model = json.loads(text)
    except ValueError:
        return 0
    total = 0

    def walk(v):
        nonlocal total
        if isinstance(v, dict):
            if 'Core' in v and isinstance(v['Core'], dict) and any(p.startswith('ntm:pile_rod') for p in prefixes):
                for c in v['Core'].get('channels', []):
                    if c.get('kind') == 'Fuel':
                        total += sum(1 for r in c.get('rods', []) if r)
                rest = {k: x for k, x in v['Core'].items() if k != 'channels'}
                walk(rest)
                for k, x in v.items():
                    if k != 'Core':
                        walk(x)
                return
            total += _stack(v, prefixes)
            for x in v.values():
                walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)

    walk(model)
    return total


def _inventory(player, prefixes):
    return sum(_stack(s, prefixes) for part in ('Inventory', 'EnderItems') for s in player.get(part, []) or [])


def count(folder, prefixes):
    prefixes = tuple(prefixes)
    world = Path(folder) / 'saves' / 'CrashTest'
    dim = world / 'dimensions' / 'minecraft' / 'overworld'
    out = {'entities': 0, 'blocks': 0, 'machines': 0, 'player': 0}
    for path in sorted((dim / 'entities').glob('r.*.mca')):
        for chunk in records(path):
            for e in chunk.get('Entities', []) or []:
                if e.get('id') == 'minecraft:item':
                    out['entities'] += _stack(e.get('Item'), prefixes)
    for path in sorted((dim / 'region').glob('r.*.mca')):
        for chunk in records(path):
            for be in chunk.get('block_entities', []) or []:
                items = sum(_stack(s, prefixes) for s in be.get('Items', []) or [])
                # A machine whose model also lists its slots (a pile loader) holds them once.
                model = _model(be['ntm'], prefixes) if isinstance(be.get('ntm'), str) else 0
                if model:
                    out['machines'] += max(model, items)
                else:
                    out['blocks'] += items
    # The owner's file: `players/data/<singleplayer_uuid>.dat` (older worlds: level.dat's Player, or playerdata/).
    level = world / 'level.dat'
    data = parse(gzip.decompress(level.read_bytes())).get('Data', {}) if level.exists() else {}
    if isinstance(data.get('Player'), dict):
        out['player'] += _inventory(data['Player'], prefixes)
    else:
        files = sorted((world / 'players' / 'data').glob('*.dat')) + sorted((world / 'playerdata').glob('*.dat'))
        owner = data.get('singleplayer_uuid')
        if isinstance(owner, list) and len(owner) == 4:
            h = ''.join(f'{v & 0xffffffff:08x}' for v in owner)
            name = f'{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}.dat'
            files = [f for f in files if f.name == name] or files
        for path in files:
            out['player'] += _inventory(parse(gzip.decompress(path.read_bytes())), prefixes)
    out['total'] = sum(out.values())
    return out


if __name__ == '__main__':
    import sys
    print(json.dumps(count(sys.argv[1], sys.argv[2:])))
