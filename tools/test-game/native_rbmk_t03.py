#!/usr/bin/env python3
"""T03 proof on a disposable world (`rbmk-plano` T03): an RBMK fuel rod's stack, its tag and its hazards.

Mode `play` (J, NTM only): a rod crafted at a table comes out with its tag set up (`onCreated`); it burns in a channel
beside a Po-210/Be source (yield down, xenon and heat up); taken out (as a thrown drop, picked up) and moved into a chest
it keeps its tag bit for bit, its radiation follows its depletion and xenon while carried (none from the chest), and its
tooltip and bar are snapshotted; a hot rod sets its holder on fire, not in water nor with a reacher; a renamed rod
(Java's `{display}` tag) reads a yield of 0: its radiation is its target's and it burns nothing.
Mode `reopen`: after a reopen the chest's rod is the same; carried again its radiation is the same; put into a lone
channel it keeps its yield and xenon (no flux), and its heat goes on from where it was.

Scenario C (the runner, run-rbmk-t03-proof.py, owns the process and the cut), modes prepare/act/verify/again/drain:
  prepare  a channel P beside a Po-210/Be source S, both fuelled by hand; P's rod made by a command with a state, a
           foreign key and a lore; a second such rod in a lone channel Q
  act      P's rod out as a thrown drop (`ntmtest take`), the player away; then the save the runner cuts
  verify   every rod the world holds kept its foreign key and lore, its yield no higher than it was made (and P's, if
           back in its channel, going down), its xenon and heat not reset
With NTM_CRASH_VANILLA=1 the vanilla control: barrels where the columns stand, the act summoning the same stack as an
item where P's was.
"""
import math
import os
import re
import struct
import sys
import time
import json
from pathlib import Path
from native_game import Game

Y = 101
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
UEU = 'ntm:rbmk_fuel_ueu'
SOURCE_ROD = 'ntm:rbmk_fuel_po210be'
PREFIX = 'ntm:rbmk_fuel_'
MAX_YIELD = 100000000.0
HAZARDS = {h['id']: h for h in json.loads((Path(__file__).resolve().parents[2] / 'crates/ntm-core/data/hazards.json').read_text())['entries']}
KEYS = ('yield', 'xenon', 'core', 'hull')


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def expected_rate(item, tag):
    """`HazardModifierRBMKRadiation.modify` for one rod (ntm_core::rbmk::hazards::radiation_rate), mRAD/s."""
    h = HAZARDS[item]
    m = h['rbmk']
    base, target = f32(h['radiation_rad_s']), f32(m['target_rad_s'])
    e = tag.get('yield', 0.0) / MAX_YIELD
    depletion = 1 - e if m['linear'] else 1 - e ** 2
    level = f32(base + f32(target - base) * depletion)
    level = f32(level + f32(m['xe135_rad_s']) * (tag.get('xenon', 0.0) / 100))
    v = f32(level * 1000)
    return 0 if not v > 0 else int(math.floor(v + .5))


def tag_of(text):
    """The rod's four doubles out of a stack's printed components (absent keys left out)."""
    out = {}
    for k in KEYS:
        m = re.search(r'(?<![\w])' + k + r'"?: ?(-?[0-9.]+(?:E-?[0-9]+)?|NaN|-?Infinity)d', text)
        if m:
            out[k] = float(m[1])
    return out


def block_items(g, p):
    return ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))


def slot_rod(g, p):
    """The tag of the rod a channel (or barrel) holds, or None."""
    text = block_items(g, p)
    return tag_of(text) if PREFIX in text else None


def hand(g):
    inv = g.ctl('inventory')
    for part in ('hotbar', 'main'):
        for s in inv.get(part, []) or []:
            if isinstance(s, dict) and str(s.get('item') or '').startswith(PREFIX):
                return s
    return None


def select_rod(g, timeout=10):
    """Select the rod once the window has it (a server's `item replace` reaches the window a moment later)."""
    deadline = time.monotonic() + timeout
    while hand(g) is None and time.monotonic() < deadline:
        time.sleep(.2)
    g.ctl(f'select {UEU}')


def hand_text(g):
    s = hand(g)
    return None if s is None else str(s.get('components') or '')


def server_rod(g):
    """The first RBMK rod of the player's inventory as the server holds it (`data get entity`): the window's own copy
    can lag a component the server changed (the engine's click agreement compares item and count only)."""
    text = ' '.join(g.command('data get entity @s Inventory').get('chat', []))
    start = text.find('[')
    depth, begin = 0, None
    for k, c in enumerate(text[start + 1:], start + 1):
        if c == '{':
            depth += 1
            if depth == 1:
                begin = k
        elif c == '}':
            depth -= 1
            if depth == 0 and begin is not None and UEU in text[begin:k + 1]:
                return text[begin:k + 1]
    return None


def radbuf(g):
    return int(' '.join(g.command('ntmitem radbuf').get('chat', [])).strip() or 0)


def steady_radbuf(g, seconds=3.5):
    """The buffer once it holds a whole second of the carried stacks (it is read once a second)."""
    time.sleep(2.2)
    seen = []
    end = time.monotonic() + seconds
    # At least three readings, however slow the game answers.
    while time.monotonic() < end or len(seen) < 3:
        seen.append(radbuf(g))
        time.sleep(.4)
    return seen


def fire(g):
    text = ' '.join(g.command('data get entity @s Fire').get('chat', []))
    m = re.search(r'(-?\d+)s?\s*$', text.strip())
    return int(m[1]) if m else None


def floor(g, x0, x1, z0, z1):
    g.command(f'forceload add {x0} {z0} {x1} {z1}')
    time.sleep(3)
    g.command(f'fill {x0} {Y} {z0} {x1} {Y + 20} {z1} minecraft:air')
    g.command(f'fill {x0} {Y - 1} {z0} {x1} {Y - 1} {z1} minecraft:stone')


def place(g, p, block):
    """The column `block` placed by the player at `p`, standing south of it (as T02's proof: the dummies with it)."""
    g.command(f'give @s {block} 1')
    g.command(f'tp @s {p[0] + .5} {p[1]} {p[2] + 3.5} 180 45')
    time.sleep(.3)
    g.ctl(f'place {p[0]} {p[1]} {p[2]} --item {block}')
    time.sleep(.5)


def put_by_hand(g, p, item):
    """One rod from the hand into the channel at `p`, used on its top cell from the south."""
    g.command(f'tp @s {p[0] + .5} {Y} {p[2] + 2.5}')
    time.sleep(.3)
    g.ctl(f'select {item}')
    g.ctl(f'interact {p[0]} {p[1] + 3} {p[2]} south')
    time.sleep(.6)
    g.close()


def take_and_pick_up(g, p):
    """The channel's rod out as a thrown drop (`ntmtest take`), then picked up."""
    reply = ' '.join(g.command(f'ntmtest take {p[0]} {p[1]} {p[2]} drop').get('chat', []))
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline and hand(g) is None:
        items = [e for e in g.ctl('entities 20')['entities'] if e.get('kind') == 'item' and e['item']['item'].startswith(PREFIX)]
        if items:
            x, y, z = items[0]['pos']
            g.command(f'tp @s {x} {y} {z}')
        time.sleep(.5)
    return reply


def gui_scale(g):
    return g.ctl('status').get('window', {}).get('width', 1280) / g.ctl('ui')['gui_size'][0]


def tooltip_snapshot(g, name):
    """The rod's tooltip in the inventory screen, the pointer over it (and its bar under the icon)."""
    from xinput import XInput
    g.close()
    # The survival inventory screen: the creative one has other slots. Fire resistance keeps a hot rod harmless.
    g.command('effect give @s minecraft:fire_resistance infinite 0 true')
    g.command('effect give @s minecraft:resistance infinite 4 true')
    mode = 'creative' if g.ctl('state').get('player', {}).get('game_mode') == 'creative' else None
    g.command('gamemode survival @s')
    time.sleep(.5)
    g.ctl('open-inventory')
    time.sleep(1)
    slot = next((s['slot'] for s in g.ctl('screen').get('slots', []) if str(s.get('item') or '').startswith(PREFIX)), None)
    widget = next((w for w in g.ctl('ui')['widgets'] if w.get('id') == f'slot-{slot}'), None)
    if widget is None:
        g.note(name, f'no slot widget for {slot}')
        g.close()
        if name != 'rbmk-t03-hot-tooltip':
            g.command('gamemode creative @s')
        return
    b = widget['bounds']
    scale = gui_scale(g)
    keys = XInput(os.environ['DISPLAY'])
    keys.focus()
    keys.pointer((b[0] + b[2] / 2) * scale, (b[1] + b[3] / 2) * scale)
    time.sleep(.6)
    g.snapshot(name)
    keys.pointer(5, 5)
    g.close()
    if mode or name != 'rbmk-t03-hot-tooltip':
        g.command('gamemode creative @s')


def craft(g, table):
    """An empty rod and 8 uranium billets at a crafting table: the result taken with shift."""
    g.command('clear @s')
    g.command('give @s ntm:rbmk_fuel_empty 1')
    g.command('give @s ntm:billet_uranium 8')
    g.command(f'setblock {table[0]} {table[1]} {table[2]} minecraft:crafting_table')
    time.sleep(.5)
    g.close()
    g.command(f'tp @s {table[0] + .5} {Y} {table[2] + 2.5}')
    time.sleep(.4)
    g.ctl(f'interact {table[0]} {table[1]} {table[2]} south')
    time.sleep(1)
    def src(item):
        return next(s['slot'] for s in g.ctl('screen')['slots'] if s.get('item') == item and s['slot'] < 100)
    g.ctl(f'click-slot {src("ntm:rbmk_fuel_empty")}')
    time.sleep(.3)
    g.ctl('click-slot 100')
    time.sleep(.3)
    g.ctl(f'click-slot {src("ntm:billet_uranium")}')
    time.sleep(.3)
    for i in range(1, 9):
        g.ctl(f'click-slot {100 + i} right')
        time.sleep(.2)
    time.sleep(.5)
    result = next((s for s in g.ctl('screen')['slots'] if s['slot'] == 200), None)
    g.ctl('click-slot 200 shift')
    time.sleep(1)
    g.close()
    return result


# ----------------------------------------------------------------------------------------------------- J
P = (44, Y, 6)           # the channel beside the source
S = (45, Y, 6)           # a ReaSim channel with the Po-210/Be source
Q = (64, Y, 6)           # a lone channel, far from any flux
CHEST = (50, Y, 12)
TABLE = (40, Y, 12)


def play(g):
    g.ctl('wait-ready 120')
    g.close()
    floor(g, 34, 70, 0, 18)
    g.command('gamemode creative @s')
    g.command('ntmitem dose 0')
    # onCreated: the crafted rod has its tag (setNBTDefaults: yield at its maximum, core and hull at 20).
    result = craft(g, TABLE)
    made = server_rod(g)
    g.evidence['crafted'] = {'result_slot': result, 'stack': made, 'window': hand_text(g)}
    crafted = made is not None and tag_of(made) == {'yield': MAX_YIELD, 'core': 20.0, 'hull': 20.0}
    if os.environ.get('NTM_HOST_CRAFT_HOOK') == '1':
        g.check('crafted_rod_has_its_tag', crafted, made)
        # The window's copy too (the click's agreement compares components: engine 01e5147ef5).
        time.sleep(1)
        window = hand_text(g)
        g.check('the_window_sees_the_tag', window is not None and tag_of(window) == tag_of(made or ''), [window, made])
    else:
        # The engine calls `prepare_item_craft` for LAN guests and dedicated servers only, not for the integrated
        # server's host (pending engine decision): noted, not failed.
        g.note('crafted_rod_has_its_tag', {'ok': crafted, 'stack': made})
    # Renamed after crafting (an anvil's `set_name`): Java's crafted rod already has its tag, so the name keeps it alive;
    # the burn below then starts from its full yield.
    select_rod(g)
    renamed = g.command('item modify entity @s weapon.mainhand {type:"minecraft:set_name",name:"Renamed"}').get('chat')
    after = server_rod(g) or ''
    g.evidence['crafted_renamed'] = {'reply': renamed, 'stack': after, 'window': hand_text(g)}
    alive = 'Renamed' in after and tag_of(after).get('yield') == MAX_YIELD
    if os.environ.get('NTM_HOST_CRAFT_HOOK') == '1':
        g.check('a_crafted_rod_renamed_keeps_its_tag', alive, after)
    else:
        g.note('a_crafted_rod_renamed_keeps_its_tag', {'ok': alive, 'stack': after})
    # It burns beside a Po-210/Be source.
    for p, block in ((P, 'ntm:rbmk_rod'), (S, 'ntm:rbmk_rod_reasim'), (Q, 'ntm:rbmk_rod'), ((46, Y, 6), 'ntm:rbmk_rod')):
        place(g, p, block)
    g.command(f'setblock {CHEST[0]} {CHEST[1]} {CHEST[2]} minecraft:chest')
    time.sleep(1.5)
    put_by_hand(g, P, UEU)
    g.command(f'give @s {SOURCE_ROD} 1')
    put_by_hand(g, S, SOURCE_ROD)
    g.command('clear @s')
    first = slot_rod(g, P)
    time.sleep(25)
    burnt = slot_rod(g, P)
    g.evidence['burning'] = {'first': first, 'later': burnt}
    # A Po-210/Be source's flux is high: xenon burns faster than it builds (`xenonBurnFunc` > `xenonGenFunc` past a
    # flux of 25) and stays at 0, as in Java; its key is written all the same.
    g.check('it_burns_yield_down_and_heat_up', bool(first) and bool(burnt) and burnt['yield'] < first['yield'] <= MAX_YIELD
            and 'xenon' in burnt and burnt['core'] > 20, [first, burnt])
    # Out by hand (a thrown drop picked up): the tag as the channel last wrote it, then still while carried.
    before = slot_rod(g, P)
    reply = take_and_pick_up(g, P)
    carried = hand_text(g)
    g.evidence['taken'] = {'reply': reply, 'channel_before': before, 'carried': carried}
    tag = tag_of(carried or '')
    g.check('taken_out_with_its_tag', carried is not None and slot_rod(g, P) is None and 'xenon' in tag
            and tag['core'] > 20 and tag['yield'] <= before['yield'], [before, carried])
    time.sleep(3)
    g.check('a_carried_rod_does_not_burn', hand_text(g) == carried, [carried, hand_text(g)])
    g.command('ntmitem dose 0')
    want = expected_rate(UEU, tag)
    seen = steady_radbuf(g)
    g.evidence['carried_rate'] = {'want': want, 'seen': seen}
    g.check('its_radiation_follows_its_tag', seen and all(v == want for v in seen), {'want': want, 'seen': seen, 'tag': tag})
    tooltip_snapshot(g, 'rbmk-t03-burnt-tooltip')
    # Its xenon counts too: a rod made half spent and 40 % poisoned, carried alone for a moment.
    select_rod(g)
    g.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.1 from entity @s weapon.mainhand')
    g.command('clear @s')
    poisoned = {'yield': 5.0E7, 'xenon': 40.0, 'core': 20.0, 'hull': 20.0}
    g.command(f'give @s {UEU}[minecraft:custom_data={{yield:5.0E7d,xenon:40.0d,core:20.0d,hull:20.0d}}] 1')
    want = expected_rate(UEU, poisoned)
    seen = steady_radbuf(g)
    g.check('its_xenon_adds_to_its_radiation', seen and all(v == want for v in seen) and want > expected_rate(UEU, dict(poisoned, xenon=0.0)),
            {'want': want, 'seen': seen})
    # No snapshot: in survival (the inventory screen) its radiation kills in a second; `tooltip::lines` is unit-tested.
    g.command('clear @s')
    g.command(f'item replace entity @s weapon.mainhand from block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.1')
    g.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.1 with minecraft:air')
    time.sleep(.5)
    # Into a chest and back: the same stack.
    select_rod(g)
    g.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.0 from entity @s weapon.mainhand')
    g.command('clear @s')
    time.sleep(.5)
    in_chest = block_items(g, CHEST)
    g.check('the_chest_holds_the_same_tag', tag_of(in_chest) == tag, [in_chest, tag])
    seen = steady_radbuf(g)
    g.check('none_from_the_chest', seen and all(v == 0 for v in seen), seen)
    g.evidence['chest'] = in_chest
    # HOT: a rod at 400 °C in its hull (made by a command) sets its survival holder on fire.
    g.command('gamemode survival @s')
    g.command('effect give @s minecraft:fire_resistance infinite 0 true')
    g.command('effect give @s minecraft:resistance infinite 4 true')
    # Never on fire before each of the first two: with a reacher (dry), then in water (`isWet`), then out of both.
    g.command(f'tp @s 56.5 {Y} 14.5')
    g.command('give @s ntm:reacher 1')
    g.command(f'give @s {UEU}[minecraft:custom_data={{yield:1.0E8d,core:400.0d,hull:400.0d}}] 1')
    time.sleep(1.5)
    reacher = [fire(g) for _ in range(4)]
    g.check('not_with_a_reacher', all(v is not None and v <= 0 for v in reacher), reacher)
    g.command('clear @s ntm:reacher')
    g.command(f'setblock 58 {Y} 14 minecraft:water')
    g.command(f'tp @s 58.5 {Y} 14.5')
    time.sleep(1.5)
    wet = [fire(g) for _ in range(4)]
    g.check('not_in_water', all(v is not None and v <= 0 for v in wet), wet)
    # Out of the water, the water gone (it spreads over the floor): dry again.
    g.command(f'fill 50 {Y} 8 66 {Y} 20 minecraft:air replace minecraft:water')
    g.command(f'tp @s 54.5 {Y} 18.5')
    time.sleep(1.5)
    burning = fire(g)
    g.check('a_hot_rod_sets_its_holder_on_fire', burning is not None and burning > 500, burning)
    tooltip_snapshot(g, 'rbmk-t03-hot-tooltip')
    # Already burning, into water without the hot rod, then with it: the fire goes out in both (`isWet`). Before the
    # glue checked the water without a visit, the fire came back to 600 with the rod.
    select_rod(g)
    g.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.2 from entity @s weapon.mainhand')
    g.command(f'clear @s {UEU}')
    g.command(f'setblock 58 {Y} 14 minecraft:water')
    g.command(f'tp @s 58.5 {Y} 14.5')
    time.sleep(1.5)
    without = [fire(g) for _ in range(6)]
    g.command(f'item replace entity @s weapon.mainhand from block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.2')
    g.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.2 with minecraft:air')
    g.command(f'fill 50 {Y} 8 66 {Y} 20 minecraft:air replace minecraft:water')
    g.command(f'tp @s 54.5 {Y} 18.5')
    time.sleep(1.5)
    lit = fire(g)
    g.command(f'setblock 58 {Y} 14 minecraft:water')
    g.command(f'tp @s 58.5 {Y} 14.5')
    time.sleep(1.5)
    with_rod = [fire(g) for _ in range(6)]
    g.check('a_burning_holder_goes_out_in_water', all(v is not None and v <= 0 for v in without + with_rod) and lit and lit > 500,
            {'without_rod': without, 'relit': lit, 'with_rod': with_rod})
    g.command(f'fill 50 {Y} 8 66 {Y} 20 minecraft:air replace minecraft:water')
    g.command('clear @s')
    g.command('gamemode creative @s')
    g.command('ntmitem dose 0')
    # Renamed (`{display}`, no yield): a yield of 0, the target's radiation, nothing burnt in a channel.
    g.command(f'give @s {UEU}[minecraft:custom_name=\'"Renamed"\'] 1')
    seen = steady_radbuf(g)
    want = expected_rate(UEU, {})
    g.check('a_renamed_rod_reads_dead', seen and all(v == want for v in seen), {'want': want, 'seen': seen})
    # No snapshot: in survival (the inventory screen) its radiation kills in a second; `tooltip::lines` is unit-tested.
    put_by_hand(g, (46, Y, 6), UEU)
    time.sleep(4)
    dead = slot_rod(g, (46, Y, 6))
    g.evidence['renamed_in_channel'] = dead
    g.check('the_renamed_rod_burns_nothing', dead is not None and dead.get('yield') == 0.0 and dead.get('core', 0) <= 20.0, dead)
    g.command('clear @s')
    g.ctl('save')


def reopen(g):
    g.ctl('wait-ready 120')
    time.sleep(3)
    g.command('gamemode creative @s')
    g.command('clear @s')
    g.command('ntmitem dose 0')
    saved = (Path(g.folder) / 'native-rbmk-t03-play.json')
    before = json.loads(saved.read_text()).get('chest') if saved.exists() else None
    in_chest = block_items(g, CHEST)
    tag = tag_of(in_chest)
    g.check('the_chest_rod_is_the_same_after_reopening', before is not None and tag_of(before) == tag and tag, [before, in_chest])
    g.command(f'item replace entity @s weapon.mainhand from block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.0')
    g.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.0 with minecraft:air')
    time.sleep(.5)
    g.check('carried_again_the_same_tag', tag_of(hand_text(g) or '') == tag, hand_text(g))
    want = expected_rate(UEU, tag)
    seen = steady_radbuf(g)
    g.check('carried_again_the_same_radiation', seen and all(v == want for v in seen), {'want': want, 'seen': seen})
    tooltip_snapshot(g, 'rbmk-t03-reopened-tooltip')
    put_by_hand(g, Q, UEU)
    time.sleep(4)
    lone = slot_rod(g, Q)
    g.evidence['lone'] = {'carried': tag, 'channel': lone}
    g.check('a_lone_channel_keeps_yield_and_xenon', lone is not None and lone['yield'] == tag['yield'] and lone['xenon'] == tag['xenon'], [tag, lone])
    g.check('its_heat_goes_on_not_reset', lone is not None and lone['core'] != 20.0 and lone['hull'] != 20.0
            and (lone['core'], lone['hull']) != (tag['core'], tag['hull']), [tag, lone])


# ----------------------------------------------------------------------------------------------- scenario C
# All in chunk (2, 0).
CP, CS, CQ = (36, Y, 6), (37, Y, 6), (42, Y, 6)
MADE = {'yield': 6.0E7, 'xenon': 12.5, 'core': 300.0, 'hull': 250.0}
TOTAL = 3


def made_rod():
    tag = ','.join(f'{k}:{v!r}d' for k, v in MADE.items())
    return f'{UEU}[minecraft:custom_data={{{tag},foreign:7.0d}},minecraft:lore=[\'"kept"\']]'


def ledger(g):
    text = ' '.join(g.command('ntmledger').get('chat', []))
    return {k: int(v) for k, v in re.findall(r'(\w+)=(\d+)', text)}


def rods(text):
    return sum(int(n) for n in re.findall(r'count: ?(\d+)[^{}]*?id: ?"(?:ntm:rbmk_fuel_\w+)"', text))


def entity_texts(g):
    return g.command('execute as @e[type=minecraft:item] run data get entity @s Item').get('chat', [])


def inv_rods(g):
    inv = g.ctl('inventory')
    stacks = [s for part in ('hotbar', 'main') for s in inv.get(part, []) or []] + [inv.get('offhand'), inv.get('carried')]
    return sum(s.get('count', 0) for s in stacks if isinstance(s, dict) and str(s.get('item') or '').startswith(PREFIX))


def count(g):
    columns = sum(rods(block_items(g, p)) for p in (CP, CS, CQ))
    entities = sum(rods(t) for t in entity_texts(g))
    return {'columns': columns, 'entities': entities, 'inventory': inv_rods(g), 'ledger': ledger(g)}


def settled(g, timeout=90):
    deadline = time.monotonic() + timeout
    previous = None
    while time.monotonic() < deadline:
        now = count(g)
        key = (now['ledger'].get('pending'), now['entities'], now['columns'], now['inventory'])
        if now['ledger'].get('pending') == 0 and key == previous:
            return now
        previous = key
        time.sleep(2.5)
    return count(g)


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    floor(g, 32, 47, 0, 14)
    g.command('clear @s')
    g.command('gamemode creative @s')
    for p, id_ in ((CP, 'rbmk_rod'), (CS, 'rbmk_rod_reasim'), (CQ, 'rbmk_rod')):
        if VANILLA:
            g.command(f'setblock {p[0]} {p[1]} {p[2]} minecraft:barrel')
        else:
            place(g, p, 'ntm:' + id_)
    time.sleep(1.5)
    for p, item in ((CP, made_rod()), (CQ, made_rod()), (CS, SOURCE_ROD)):
        if VANILLA:
            g.command(f'item replace block {p[0]} {p[1]} {p[2]} container.0 with {item} 1')
        else:
            g.command(f'give @s {item} 1')
            put_by_hand(g, p, SOURCE_ROD if item == SOURCE_ROD else UEU)
            g.command('clear @s')
    g.command(f'tp @s 40.5 {Y} 12.5')
    time.sleep(3)
    base = count(g)
    g.check('base_rods', base['columns'] == TOTAL and base['inventory'] == 0 and base['entities'] == 0, base)
    if not VANILLA:
        g.evidence['base_tags'] = {'P': slot_rod(g, CP), 'Q': slot_rod(g, CQ)}
        g.check('made_rods_hold_their_state', all(t and t['yield'] <= MADE['yield'] for t in g.evidence['base_tags'].values()),
                g.evidence['base_tags'])
    g.evidence['base'] = base
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    g.command(f'tp @s {CP[0] + .5} {Y} {CP[2] + 2.5}')
    time.sleep(3)
    before = count(g)
    g.check('start_with_three', before['columns'] == TOTAL, before)
    if VANILLA:
        g.command(f'item replace block {CP[0]} {CP[1]} {CP[2]} container.0 with minecraft:air')
        tag = ','.join(f'{k}:{v!r}d' for k, v in MADE.items())
        g.command(f'summon minecraft:item {CP[0] + .5} {Y + 1.5} {CP[2] + 1.5} {{Item:{{id:"{UEU}",count:1,components:'
                  f'{{"minecraft:custom_data":{{{tag},foreign:7.0d}},"minecraft:lore":[\'"kept"\']}}}},PickupDelay:32767s}}')
    else:
        g.evidence['taken_from'] = slot_rod(g, CP)
        g.evidence['hook'] = g.command(f'ntmtest take {CP[0]} {CP[1]} {CP[2]} drop').get('chat')
    # A block of chunk (2, 0) changes in both worlds: the engine's `item replace block` does not mark the barrel's chunk
    # changed (T01's finding), and the cuts by chunk record must land alike.
    g.command(f'setblock 33 {Y} 13 minecraft:cobblestone')
    # Away at once, in both worlds: no pickup before the save.
    g.command(f'tp @s 40.5 {Y} 13.5')
    time.sleep(1.5)
    after = count(g)
    g.evidence['after_act'] = after
    g.check('the_rod_out_once', after['columns'] == TOTAL - 1 and after['entities'] == 1 and after['inventory'] == 0, after)
    g.save()
    if NO_SAVE:
        return
    if PAUSE_SAVE:
        g.evidence['pause_save'] = True
        g.save()
        try:
            g.ctl('pause')
        except Exception:
            return
        deadline = time.monotonic() + 270
        while time.monotonic() < deadline:
            try:
                g.ctl('status')
            except Exception:
                return
            time.sleep(.5)
        return
    try:
        g.ctl('save')
        g.evidence['survived_save'] = True
    except Exception as error:
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command(f'tp @s 40.5 {Y} 13.5')
    now = settled(g)
    now['total'] = now['columns'] + now['entities'] + now['inventory']
    g.evidence['counted' + ('_again' if again else '')] = now
    g.check('no_more_than_twice', now['total'] <= 2 * TOTAL, now)
    g.check('nothing_left_to_spawn', now['ledger'].get('pending') == 0, now['ledger'])
    if VANILLA:
        return
    # Every ueu rod, wherever the cut left it: its foreign key and lore, a yield no higher than made, its tag not reset
    # (a fresh one would read yield 1e8, no xenon key, 20 °C core and hull). Burning, its xenon may fall to 0 (a flux
    # over 25 burns more than it makes, as in Java).
    texts = [block_items(g, p) for p in (CP, CQ)] + [t for t in entity_texts(g) if UEU in t]
    found = []
    for t in texts:
        if UEU not in t:
            continue
        tag = tag_of(t)
        found.append({'tag': tag, 'foreign': bool(re.search(r'foreign"?: ?7\.0d', t)), 'lore': 'kept' in t})
    g.evidence['rods' + ('_again' if again else '')] = found
    g.check('each_rod_kept_its_state', found and all(r['foreign'] and r['lore'] and set(r['tag']) == set(KEYS)
                                                     and r['tag']['yield'] <= MADE['yield']
                                                     and (r['tag']['core'], r['tag']['hull']) != (20.0, 20.0) for r in found), found)
    # P's rod, if back in its channel, burns on: its yield never rises (the ReaSim source's streams go in random
    # directions, so a few seconds may bring it no flux at all).
    first = slot_rod(g, CP)
    if first:
        time.sleep(3)
        later = slot_rod(g, CP)
        g.check('the_yield_only_goes_down', later is not None and later['yield'] <= first['yield'], [first, later])


def drain(g):
    g.ctl('wait-ready 120')
    g.command('kill @e[type=minecraft:item]')
    time.sleep(1.5)
    led = ledger(g)
    g.check('nothing_owed_when_the_items_are_gone', led.get('pending') == 0, led)
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t03-' + mode)
    {'play': play, 'reopen': reopen, 'prepare': prepare, 'act': act, 'verify': verify,
     'again': lambda g: verify(g, True), 'drain': drain}[mode](g)
    g.evidence['ok'] = True
    g.save()
