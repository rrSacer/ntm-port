#!/usr/bin/env python3
"""What the files of a test world hold of one metal, in quanta (the foundry's unit: an ingot is 72), read before it is
reopened (no game running). The counterpart of `disk_count.py` for the foundry's proofs (Onda 2, D4).

The atoms of gold: `minecraft:gold_ingot` 72, `minecraft:gold_nugget` 8, `minecraft:gold_block` 648, an `ntm:scraps__gold`
with the amount its `minecraft:custom_data` `ntm_amount` carries (72 without it), and the molten metal of the machines'
models: a pool (`mat`/`amount`: channels, tanks, molds, basins, the caster), a crucible's `recipe_stack` and
`waste_stack`, an arc furnace's `liquids`, a rotary furnace's `output`. Where: item entities, block entities' slots,
the models, what chunk records owe (ledgers: owed stacks, removals, batches of item effects) and the owner's inventory.
The same rules as `disk_count.py` (a machine whose model lists its slots counts them once).
"""
import gzip
import json
from pathlib import Path

import disk_count as dc

METAL = 'gold'
WEIGHT = {'minecraft:gold_ingot': 72, 'minecraft:gold_nugget': 8, 'minecraft:gold_block': 648}
SCRAP = 'ntm:scraps__' + METAL


def _amount_of(data):
    """The `ntm_amount` of a scrap's custom data, as the item's atom (72 without it)."""
    if isinstance(data, str):
        try:
            data = dc._snbt(data)
        except (AssertionError, IndexError):
            return 72
    if isinstance(data, dict) and 'ntm_amount' in data:
        return int(float(str(data['ntm_amount']).rstrip('bsilBSIL') or 0))
    return 72


def _weight(item, custom):
    if item == SCRAP:
        return _amount_of(custom)
    return WEIGHT.get(item, 0)


def stack_quanta(s):
    """A stack in the files' NBT (`id`, `count`, `components`): its quanta."""
    if not isinstance(s, dict):
        return 0
    item = s.get('id') or s.get('Id') or ''
    custom = (s.get('components') or {}).get('minecraft:custom_data')
    return _weight(item, custom) * int(s.get('count', s.get('Count', 1)) or 0)


def snbt_quanta(text):
    """A stack of a ledger entry (SNBT text)."""
    try:
        s = dc._snbt(text)
    except (AssertionError, IndexError):
        return 0
    if not isinstance(s, dict):
        return 0
    item = s.get('id', '')
    custom = (s.get('components') or {}).get('minecraft:custom_data') if isinstance(s.get('components'), dict) else None
    count = int(float(str(s.get('count', '1')).rstrip('bsilBSIL') or 0))
    return _weight(item, custom) * count


def _molten(v):
    """The molten metal a model's dict holds (one machine's fields): pools, stacks and buffers of `METAL`."""
    total = 0
    if v.get('mat') == METAL and isinstance(v.get('amount'), int):
        total += v['amount']
    for key in ('recipe_stack', 'waste_stack', 'liquids'):
        for pair in v.get(key, []) or []:
            if isinstance(pair, list) and len(pair) == 2 and pair[0] == METAL:
                total += int(pair[1])
    out = v.get('output')
    if isinstance(out, list) and len(out) == 2 and out[0] == METAL:
        total += int(out[1])
    for side in ('left', 'right'):
        pair = v.get(side)
        if isinstance(pair, list) and len(pair) == 2 and pair[0] == METAL:
            total += int(pair[1])
    return total


def _model(model):
    """(molten quanta, {stack id/weight: quanta} of the slots the model lists)."""
    molten, slots = 0, {}

    def walk(v):
        nonlocal molten
        if isinstance(v, dict):
            molten += _molten(v)
            q = stack_quanta(v)
            if q:
                key = (v.get('id') or v.get('Id'), _weight(v.get('id') or v.get('Id'),
                       (v.get('components') or {}).get('minecraft:custom_data')))
                slots[key] = slots.get(key, 0) + q
            for x in v.values():
                walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)

    walk(model)
    return molten, slots


def _be(be):
    """A block entity's quanta: its slots and its model's molten metal; a stack both list counts once."""
    slots = {}
    for s in be.get('Items', []) or []:
        q = stack_quanta(s)
        if q:
            key = (s.get('id') or s.get('Id'), _weight(s.get('id') or s.get('Id'),
                   (s.get('components') or {}).get('minecraft:custom_data')))
            slots[key] = slots.get(key, 0) + q
    molten, model_slots = 0, {}
    if isinstance(be.get('ntm'), str):
        try:
            molten, model_slots = _model(json.loads(be['ntm']))
        except ValueError:
            pass
    return molten + sum(max(slots.get(k, 0), model_slots.get(k, 0)) for k in set(slots) | set(model_slots)), molten


def _owed(ledger):
    total = 0
    for e in ledger.get('entries', []) or []:
        if e.get('state', 'Pending') == 'Pending':
            total += snbt_quanta(e.get('item', ''))
    for r in ledger.get('removals', []) or []:
        molten, slots = _model(r.get('machine', {}).get('data', {}))
        # A removed machine's slots were spilled by the engine; its molten metal is the debt (a scrap on reopening).
        total += molten
    for b in ledger.get('batches', []) or []:
        for effect in b.get('effects', []) or []:
            if 'Items' in effect:
                for item, n in effect['Items'].get('items', []):
                    total += WEIGHT.get(item, 0) * int(n) + (72 * int(n) if item == SCRAP else 0)
            elif 'EncodedItems' in effect:
                total += sum(snbt_quanta(t) for t in effect['EncodedItems'].get('items', []))
            elif 'Blocks' in effect:
                for d in effect['Blocks'].get('retained', []) or []:
                    total += sum(snbt_quanta(t) for t in d.get('items', []))
    return total


def count(folder, dims=('overworld',)):
    """`{'entities', 'blocks', 'molten', 'owed', 'player', 'total'}` in quanta of `METAL`."""
    world = Path(folder) / 'saves' / 'CrashTest'
    out = {'entities': 0, 'blocks': 0, 'molten': 0, 'owed': 0, 'player': 0}
    for name in dims:
        dim = world / 'dimensions' / 'minecraft' / name
        for path in sorted((dim / 'entities').glob('r.*.mca')):
            for chunk in dc.records(path):
                for e in chunk.get('Entities', []) or []:
                    if e.get('id') == 'minecraft:item':
                        out['entities'] += stack_quanta(e.get('Item'))
        for path in sorted((dim / 'region').glob('r.*.mca')):
            for chunk in dc.records(path):
                for be in chunk.get('block_entities', []) or []:
                    total, molten = _be(be)
                    out['blocks'] += total - molten
                    out['molten'] += molten
                out['owed'] += _owed(dc._ledger(chunk))
    for pl in _players(world):
        for part in ('Inventory', 'EnderItems'):
            out['player'] += sum(stack_quanta(s) for s in pl.get(part, []) or [])
    out['total'] = sum(out.values())
    return out


def _chunk_quanta(folder, chunk):
    """The quanta one chunk record holds: its item entities, or its block entities (and the ledger it owes)."""
    if folder == 'entities':
        return sum(stack_quanta(e.get('Item')) for e in chunk.get('Entities', []) or [] if e.get('id') == 'minecraft:item')
    return sum(_be(be)[0] for be in chunk.get('block_entities', []) or []) + _owed(dc._ledger(chunk))


def written(before, after, dims=('overworld',)):
    """The chunks holding the metal (before or after) whose record differs between two copies of the world, per folder,
    with what they held before and after: what the play wrote, as `disk_count.written` does for items."""
    out = {}
    for name, folder in ((n, f) for n in dims for f in ('region', 'entities')):
        chunks = {}
        for side, root in (('before', before), ('after', after)):
            dim = Path(root) / 'saves' / 'CrashTest' / 'dimensions' / 'minecraft' / name / folder
            for path in sorted(dim.glob('r.*.mca')):
                rx, rz = (int(v) for v in path.name.split('.')[1:3])
                for i, raw in dc.raw_records(path):
                    chunks.setdefault((rx * 32 + i % 32, rz * 32 + i // 32), {})[side] = raw
        rows = []
        for (x, z), sides in sorted(chunks.items()):
            if sides.get('before') == sides.get('after'):
                continue
            held = [_chunk_quanta(folder, dc.parse(sides[s])) if s in sides else 0 for s in ('before', 'after')]
            if any(held):
                rows.append([x, z, *held])
        out[folder if name == 'overworld' else f'{name}/{folder}'] = rows
    return out


def _players(world):
    level = world / 'level.dat'
    data = dc.parse(gzip.decompress(level.read_bytes())).get('Data', {}) if level.exists() else {}
    if isinstance(data.get('Player'), dict):
        return [data['Player']]
    files = sorted((world / 'players' / 'data').glob('*.dat')) + sorted((world / 'playerdata').glob('*.dat'))
    owner = data.get('singleplayer_uuid')
    if isinstance(owner, list) and len(owner) == 4:
        h = ''.join(f'{v & 0xffffffff:08x}' for v in owner)
        name = f'{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}.dat'
        files = [f for f in files if f.name == name] or files
    return [dc.parse(gzip.decompress(f.read_bytes())) for f in files]


if __name__ == '__main__':
    import sys
    print(json.dumps(count(sys.argv[1])))
