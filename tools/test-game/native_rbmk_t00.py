#!/usr/bin/env python3
"""T00 catalog/data acceptance, deliberately not a claim of RBMK runtime.

Executed through run-native-reactors.py rbmk-t00, with a second session proving
inventory conservation. Screenshots capture current tooltips; Java key/argument
traces are attached separately, never passed off as the game's dynamic tooltip.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import time

from native_game import Game
from xinput import XInput

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tools/rbmk-oracle/fixtures/java.json'
SENTINELS = {'minecraft:stone': 17, 'minecraft:emerald': 13}


def inventory(g):
    value = g.ctl('inventory')
    return sorted([s for s in value['hotbar'] + value['main'] if s.get('item')], key=lambda s: s['slot'])


def check_sentinels(g, name):
    counts = g.ctl('inventory')['counts']
    g.check(name, all(counts.get(i) == n for i, n in SENTINELS.items()), counts)


def setup(g):
    fixture = json.loads(FIXTURE.read_text())
    rows = json.loads((ROOT / 'content/ids.json').read_text())['entries']
    variables = {'rbmk_lid', 'rbmk_lid_glass', 'rbmk_tool'}
    variables.update(r['variable'] for r in fixture['rods'] + fixture['pellets'])
    blocks = [r for r in rows if r['type'] == 'block' and r['source_variable'].startswith(('rbmk_', 'pribris'))]
    selected = [r for r in rows if r['source_variable'] in variables or r in blocks]
    g.check('source_catalog_105', len(selected) == 105 and len(blocks) == 37, len(selected))
    g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.ground(10, 101, 8, width=48, height=18, depth=48)
    for item, count in SENTINELS.items():
        g.command(f'give @s {item} {count}')
    # Place every registered block at its own coordinates, including the 33
    # currently Inert machines. Probe identity only; they have no simulation.
    for index, row in enumerate(blocks):
        x, z = 2 + index % 8 * 5, 2 + index // 8 * 5
        g.command(f'setblock {x} 101 {z} {row["id"]}')
        actual = g.ctl(f'block {x} 101 {z}')
        g.check('block_' + row['id'], actual['block'] == row['id'], actual)
    # Deliver every catalog item in batches while retaining previous inventory.
    for offset in range(0, len(selected), 30):
        batch = selected[offset:offset + 30]
        for row in batch:
            g.command('give @s ' + row['id'] + ' 1')
        actual = g.ctl('inventory')['counts']
        for row in batch:
            g.check('catalog_item_' + row['id'], actual.get(row['id']) == 1, actual.get(row['id']))
        check_sentinels(g, f'sentinels_catalog_batch_{offset}')
        for row in batch:
            g.command('clear @s ' + row['id'])
    # The given data is a fixture, not newly implemented state initialization.
    # Every rod has a distinct retained marker and exact Java-observed fresh data.
    for index, rod in enumerate(fixture['rods']):
        nbt = rod['fresh_stack']['nbt']
        tag = ','.join(f'{key}:{value}d' for key, value in nbt.items())
        g.command(f'give @s {rod["item"]}[minecraft:custom_data={{{tag},oracle_marker:{index}}}] 1')
    actual = inventory(g)
    for index, rod in enumerate(fixture['rods']):
        stacks = [s for s in actual if s['item'] == rod['item']]
        g.check('rod_delivery_' + rod['variable'], len(stacks) == 1 and stacks[0]['count'] == 1, stacks)
        component = stacks[0].get('components', '')
        observed = {key: float(value) for key, value in re.findall(
            r'\b(yield|core|hull):\s*([-+\d.eE]+)[dD]?', component)}
        g.check('rod_data_' + rod['variable'], observed == rod['fresh_stack']['nbt'], component)
        g.check('rod_marker_' + rod['variable'], re.search(r'\boracle_marker:\s*' + str(index) + r'\b', component) is not None, component)
    check_sentinels(g, 'sentinels_after_33_rods')
    # A vanilla chest exposes all 36 player slots in creative as well. The
    # creative item tab only draws the hotbar, hiding rods in slots 9..35.
    g.command('setblock 48 101 8 minecraft:chest')
    g.open(48, 101, 8)
    keys = XInput(os.environ['DISPLAY'])
    ui = g.ctl('ui')
    # run-native-reactors.py fixes --width 1280; older status has no window field.
    scale = g.ctl('status').get('window', {}).get('width', 1280) / ui['gui_size'][0]
    traces = []
    for rod in fixture['rods']:
        slot = next(s for s in actual if s['item'] == rod['item'])['slot']
        node = next(w for w in ui['widgets'] if w['id'] == 'slot-' + str(slot))
        x, y, width, height = node['bounds']
        keys.pointer((x + width / 2) * scale, (y + height / 2) * scale)
        name = 'rbmk-t00-tooltip-' + rod['variable']
        g.snapshot(name)
        traces.append({'item': rod['item'], 'delivered': next(s for s in actual if s['item'] == rod['item']),
                       'source_full_name': rod['full_name'], 'java_tooltip_keys': rod['fresh_tooltip_keys'],
                       'current_tooltip_screenshot': 'native-' + name + '.png',
                       'runtime_gap': 'Dynamic RBMK tooltip is T03; screenshot shows current catalog/hazard presentation.'})
    g.close();after = inventory(g)
    g.check('hover_did_not_mutate_any_stack', actual == after, {'before': actual, 'after': after})
    expected = {'inventory': after, 'fixture_sha256': hashlib.sha256(FIXTURE.read_bytes()).hexdigest(),
                'catalog_items': len(selected), 'blocks': len(blocks), 'rods': len(fixture['rods']),
                'tooltip_traces': traces}
    (g.folder / 'rbmk-t00-expected.json').write_text(json.dumps(expected, ensure_ascii=False, indent=2) + '\n')
    g.note('scope', 'T00 catalog/fixtures only. Given custom_data is controlled test input; native RBMK state initialization, physics and dynamic tooltips remain T03/T04. D8 leverStop remains blocked.')
    g.snapshot('rbmk-t00-before-reopen')


def reopened(g):
    expected = json.loads((g.folder / 'rbmk-t00-expected.json').read_text())
    g.check('same_fixture', expected['fixture_sha256'] == hashlib.sha256(FIXTURE.read_bytes()).hexdigest(), expected['fixture_sha256'])
    actual = inventory(g)
    g.check('reopen_preserves_all_rod_data_and_previous_inventory', actual == expected['inventory'],
            {'before': expected['inventory'], 'after': actual})
    check_sentinels(g, 'sentinels_after_reopen')
    g.snapshot('rbmk-t00-reopened')


if __name__ == '__main__':
    mode = sys.argv[2] if len(sys.argv) > 2 else 'setup'
    game = Game(sys.argv[1], 'rbmk-t00-' + mode)
    game.evidence['phase'] = mode
    game.ctl('wait-ready 120')
    {'setup': setup, 'check-state': reopened}[mode](game)
    game.evidence['ok'] = True
    game.save()
