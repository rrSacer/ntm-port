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


def raw_records(path):
    """Every chunk record of a region file, decompressed, by its index in the region."""
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
        yield i, raw


def records(path):
    """Every chunk record of a region file, parsed."""
    for _, raw in raw_records(path):
        yield parse(raw)


def _held(folder, chunk, prefixes):
    if folder == 'entities':
        return sum(_stack(e.get('Item'), prefixes) for e in chunk.get('Entities', []) or [] if e.get('id') == 'minecraft:item')
    total = 0
    for be in chunk.get('block_entities', []) or []:
        items = sum(_stack(s, prefixes) for s in be.get('Items', []) or [])
        model = _model(be['ntm'], prefixes) if isinstance(be.get('ntm'), str) else 0
        total += max(model, items)
    return total


def written(before, after, prefixes):
    """The chunks holding the counted items (before or after) whose record differs between two copies of the world, per
    folder (`region`, `entities`), with what they held before and after: what the play wrote of them, so two worlds cut
    at the same point can be told to have written the same of their items or not."""
    prefixes = tuple(prefixes)
    out = {}
    for folder in ('region', 'entities'):
        chunks = {}
        for side, root in (('before', before), ('after', after)):
            dim = Path(root) / 'saves' / 'CrashTest' / 'dimensions' / 'minecraft' / 'overworld' / folder
            for path in sorted(dim.glob('r.*.mca')):
                rx, rz = (int(v) for v in path.name.split('.')[1:3])
                for i, raw in raw_records(path):
                    chunks.setdefault((rx * 32 + i % 32, rz * 32 + i // 32), {})[side] = raw
        rows = []
        for (x, z), sides in sorted(chunks.items()):
            if sides.get('before') == sides.get('after'):
                continue
            held = [_held(folder, parse(sides[s]), prefixes) if s in sides else 0 for s in ('before', 'after')]
            if any(held):
                rows.append([x, z, *held])
        out[folder] = rows
    return out


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
    out = {'entities': 0, 'blocks': 0, 'machines': 0, 'owed': 0, 'player': 0}
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
            # What the chunk's record owes the world (its ledger): items a reopen puts out.
            owed = {}
            _owed_items(_ledger(chunk), prefixes, owed)
            out['owed'] += sum(owed.values())
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


# ---------------------------------------------------------------------------------------- what the world holds, by item

# `ntm_core::pile::ROD_NAMES`: a pile rod's kind in a core's model.
ROD_NAMES = ['ra226be', 'po210be', 'zr', 'nu', 'pu239', 'rgp', 'waste', 'thorium', 'thorium_fuel']


def _snbt(text):
    """A small SNBT reader: compounds, lists, quoted and bare strings, numbers (as text)."""
    i = 0

    def ws():
        nonlocal i
        while i < len(text) and text[i].isspace():
            i += 1

    def value():
        nonlocal i
        ws()
        c = text[i]
        if c == '{':
            i += 1
            out = {}
            ws()
            if text[i] == '}':
                i += 1
                return out
            while True:
                ws()
                k = word(key=True)
                ws()
                assert text[i] == ':'
                i += 1
                out[k] = value()
                ws()
                if text[i] == '}':
                    i += 1
                    return out
                assert text[i] == ','
                i += 1
        if c == '[':
            i += 1
            if text[i:i + 2] in ('B;', 'I;', 'L;'):
                i += 2
            out = []
            ws()
            if text[i] == ']':
                i += 1
                return out
            while True:
                out.append(value())
                ws()
                if text[i] == ']':
                    i += 1
                    return out
                assert text[i] == ','
                i += 1
        return word()

    def word(key=False):
        nonlocal i
        ws()
        if text[i] in '"\'':
            q = text[i]
            i += 1
            out = []
            while text[i] != q:
                if text[i] == '\\':
                    i += 1
                out.append(text[i])
                i += 1
            i += 1
            return ''.join(out)
        start = i
        while i < len(text) and (text[i].isalnum() or text[i] in '_-.+' or text[i] == ':' and not key):
            i += 1
        return text[start:i]

    return value()


def _add(out, item, n, prefixes):
    if isinstance(item, str) and item.startswith(prefixes) and n > 0:
        out[item] = out.get(item, 0) + n


def _stack_items(s, prefixes, out):
    if isinstance(s, dict):
        item = s.get('id') or s.get('Id') or ''
        _add(out, item, int(s.get('count', s.get('Count', 1)) or 0), prefixes)


def _snbt_items(text, prefixes, out):
    try:
        s = _snbt(text)
    except (AssertionError, IndexError):
        return
    if isinstance(s, dict):
        item = s.get('id', '')
        _add(out, item, int(float(str(s.get('count', '1')).rstrip('bsilBSIL') or 0)), prefixes)


def _machine_items(m, prefixes, out):
    """The items an NTM machine's model (`ntm_core::machine::Machine`, as JSON) holds: a pile core's rods by kind, and
    the stacks of its slots."""
    def walk(v):
        if isinstance(v, dict):
            if 'Core' in v and isinstance(v['Core'], dict):
                for c in v['Core'].get('channels', []) or []:
                    if c.get('kind') == 'Fuel':
                        for r in c.get('rods', []) or []:
                            if r:
                                _add(out, 'ntm:pile_rod__' + ROD_NAMES[r['kind']], 1, prefixes)
                walk({k: x for k, x in v['Core'].items() if k != 'channels'})
                for k, x in v.items():
                    if k != 'Core':
                        walk(x)
                return
            _stack_items(v, prefixes, out)
            for x in v.values():
                walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)
    walk(m)


def _ledger(chunk):
    """The NTM ledger a chunk record's attachments hold (`horizon:attachments` → `ntm` → `ledger`), as JSON."""
    try:
        text = chunk['horizon:attachments']['ntm']['ledger']['value']['v']
        return json.loads(text) if text else {}
    except (KeyError, TypeError, ValueError):
        return {}


def _owed_items(ledger, prefixes, out):
    """What a chunk's record owes the world: owed stacks, removed machines' contents, batches' item effects."""
    for e in ledger.get('entries', []) or []:
        if e.get('state', 'Pending') == 'Pending':
            _snbt_items(e.get('item', ''), prefixes, out)
    for r in ledger.get('removals', []) or []:
        _machine_items(r.get('machine', {}).get('data', {}), prefixes, out)
    for b in ledger.get('batches', []) or []:
        for effect in b.get('effects', []) or []:
            if 'Items' in effect:
                for item, n in effect['Items'].get('items', []):
                    _add(out, item, int(n), prefixes)
            elif 'EncodedItems' in effect:
                for text in effect['EncodedItems'].get('items', []):
                    _snbt_items(text, prefixes, out)
            elif 'Blocks' in effect:
                for d in effect['Blocks'].get('retained', []) or []:
                    for text in d.get('items', []):
                        _snbt_items(text, prefixes, out)


def items(folder, prefixes):
    """What the files of the world hold, item by item (`{id: count}`): item entities, block entities' slots, NTM models
    (a core's rods by kind), the owner's inventory, and what chunk records owe (ledgers). The same world reopened holds
    the same; the vanilla control at the same cut holds the same, or vanilla's loss or copy."""
    prefixes = tuple(prefixes)
    world = Path(folder) / 'saves' / 'CrashTest'
    dim = world / 'dimensions' / 'minecraft' / 'overworld'
    out = {}
    for path in sorted((dim / 'entities').glob('r.*.mca')):
        for chunk in records(path):
            for e in chunk.get('Entities', []) or []:
                if e.get('id') == 'minecraft:item':
                    _stack_items(e.get('Item'), prefixes, out)
    for path in sorted((dim / 'region').glob('r.*.mca')):
        for chunk in records(path):
            for be in chunk.get('block_entities', []) or []:
                slots, model = {}, {}
                for s in be.get('Items', []) or []:
                    _stack_items(s, prefixes, slots)
                if isinstance(be.get('ntm'), str):
                    try:
                        _machine_items(json.loads(be['ntm']), prefixes, model)
                    except ValueError:
                        pass
                # A machine whose model also lists its slots (a pile loader) holds them once.
                for k in set(slots) | set(model):
                    out[k] = out.get(k, 0) + max(slots.get(k, 0), model.get(k, 0))
            _owed_items(_ledger(chunk), prefixes, out)
    level = world / 'level.dat'
    data = parse(gzip.decompress(level.read_bytes())).get('Data', {}) if level.exists() else {}
    players = []
    if isinstance(data.get('Player'), dict):
        players = [data['Player']]
    else:
        files = sorted((world / 'players' / 'data').glob('*.dat')) + sorted((world / 'playerdata').glob('*.dat'))
        owner = data.get('singleplayer_uuid')
        if isinstance(owner, list) and len(owner) == 4:
            h = ''.join(f'{v & 0xffffffff:08x}' for v in owner)
            name = f'{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}.dat'
            files = [f for f in files if f.name == name] or files
        players = [parse(gzip.decompress(f.read_bytes())) for f in files]
    for pl in players:
        for part in ('Inventory', 'EnderItems'):
            for s in pl.get(part, []) or []:
                _stack_items(s, prefixes, out)
    return out


def kv_items(folder):
    """The entries the NTM storage of the world keeps that can hold items or decide a chunk's contents (removals,
    batches, prepared retentions): zero in a world of this version."""
    import ntm_store
    total = 0
    for store in ntm_store.folders(folder):
        raw = ntm_store.keys(store).get('world')
        if raw:
            kept = json.loads(raw)
            total += len(kept.get('removals', [])) + len(kept.get('machine_effects', [])) + len(kept.get('prepared_retentions', []))
    return total


if __name__ == '__main__':
    import sys
    print(json.dumps(count(sys.argv[1], sys.argv[2:])))
