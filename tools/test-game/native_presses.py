#!/usr/bin/env python3
"""Native equivalents of presses.py: eight iron plates and a worn wire stamp.

Usage: python3 tools/test-game/native_presses.py GAME [prensa|eprensa ...]
Run the game from build-native.sh with NTM content and GAME/control.sock.
The electric press uses its creative battery slot until the HE network is
accepted separately. The gas centrifuge is fed by a UF6 barrel and typed ducts in the native
network scenario; the former WASM pour button is not a native input.
"""
import sys
from native_game import Game


def count(slots, item):
    return sum(s.get('count', 0) for s in slots if s.get('item') == item)


def run(game, phase):
    game.evidence['phase'] = phase
    x, y, z = (40 if phase == 'prensa' else 48), 100, 8
    game.close()
    game.ground(x, y, z, width=5)
    game.command('clear @s')
    block = 'machine_press' if phase == 'prensa' else 'machine_epress'
    # Player placement creates the multiblock. Open its visible upper dummy.
    game.command(f'give @s ntm:{block} 1')
    game.ctl(f'place {x} {y} {z} --item ntm:{block}')
    game.open(x, y, z, dummy_height=1)
    if phase == 'prensa':
        game.put('minecraft:coal', 4, 0)
        game.put('ntm:stamp_iron_plate', 1, 1)
        game.put('minecraft:iron_ingot', 8, 2)
        slots = game.poll(lambda s: count(s, 'ntm:plate_iron') == 8, timeout=240)
        game.check('eight_plates', count(slots, 'ntm:plate_iron') == 8, slots)
        stamp = next((s for s in slots if s.get('item') == 'ntm:stamp_iron_plate'), {})
        game.check('stamp_damage_8', stamp.get('damage') == 8, stamp)
        game.snapshot('presses-prensa')
        game.ctl('click-slot 303 shift')
        game.close()
        inventory = game.ctl('inventory')
        game.check('eight_plates_taken', inventory.get('counts', {}).get('ntm:plate_iron') == 8, inventory)
    elif phase == 'eprensa':
        game.put('ntm:stamp_iron_wire', 1, 1, damage=62)
        game.put('ntm:ingot_copper', 5, 2)
        game.put('ntm:battery_creative', 1, 0)
        slots = game.poll(lambda s: count(s, 'ntm:wire_fine__copper') == 16)
        game.snapshot('presses-eprensa')
        game.command(f'data get block {x} {y} {z}')
        game.check('sixteen_wires', count(slots, 'ntm:wire_fine__copper') == 16, slots)
        game.check('three_copper_left', count(slots, 'ntm:ingot_copper') == 3, slots)
        game.check('stamp_broken', count(slots, 'ntm:stamp_iron_wire') == 0, slots)
        game.close()
    game.evidence['phase'] = phase
    game.save()


if __name__ == '__main__':
    game = Game(sys.argv[1], 'presses')
    phases = sys.argv[2:] or ['prensa', 'eprensa']
    unknown = set(phases) - {'prensa', 'eprensa'}
    if unknown:
        raise SystemExit(f'unknown phases: {sorted(unknown)}')
    game.ctl('wait-ready 120')
    for phase in phases:
        run(game, phase)
    game.evidence['ok'] = True
    game.save()
