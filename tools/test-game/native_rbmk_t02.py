#!/usr/bin/env python3
"""T02 proof on a disposable world (`rbmk-plano` T02): the RBMK columns as blocks, placed and broken by a survival
player.

Mode `play` (J, NTM only): the dial `dialColumnHeight` at 2, 4 (default), 16 and its clamp/parse (1, 99, abc) gives the
height of the columns placed after it; an obstructed one is refused with the item kept; the four facings give the same
column; a column broken from its base, a middle cell or its top goes whole and gives one item back; the lid goes on from
the top cell (one taken from the hand), comes off with a sneaking screwdriver as one item (the tool worn), never on a
control rod; a core taken away by a command takes its dummies; `/ntmrbmk migrate` rebuilds a column to the dial; a
hopper over a storage's top cell fills it with rods only, one over a fuel channel's top keeps its rod.
Mode `reopen`: after a reopen the dial, the heights and the lid are as they were.

Scenario C (the runner, run-rbmk-t02-proof.py, owns the process and the cut), modes prepare/act/verify/again/drain:
  prepare  three blank columns A, B, C (4 high); B and C with a lid; the player holding one lid, a screwdriver and an
           iron pickaxe
  act      the lid in hand onto A (one transfer), B's lid off with the screwdriver (an item at its top), C broken with
           its lid (the column's item and the lid at its top); then the save the runner cuts
With NTM_CRASH_VANILLA=1 the vanilla control: barrels where the columns stand, B's and C's holding a lid; the act puts
the hand's lid into A's barrel (`item replace`, then `clear`), takes B's out to an item at the same place (`summon`), and
breaks C's barrel (`setblock ... destroy`).
"""
import os
import re
import sys
import time
from native_game import Game
from native_pile import block

Y = 101
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
DUMMY = 'ntm:dummy_rbmk'
LID = 'ntm:rbmk_lid'
LIDS = ('ntm:rbmk_lid', 'ntm:rbmk_lid_glass')


def name(b):
    return (b or '').split('[')[0]


def cells(g, p, n):
    return [name(block(g, (p[0], p[1] + dy, p[2]))) for dy in range(n)]


def stands(g, core, column, h):
    """The column at `core` is `h` cells high: its core, `h - 1` dummies, and no dummy above."""
    got = cells(g, core, h + 1)
    return got == ['ntm:' + column] + [DUMMY] * (h - 1) + [got[h]] and got[h] != DUMMY, got


def lid(g, p):
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))
    m = re.search(r'\\?"lid\\?":\s*\\?"(\w+)', text)
    return m[1] if m else ('None' if 'has_rod' in text else None)


def inv(g, prefixes):
    i = g.ctl('inventory')
    stacks = [s for part in ('hotbar', 'main') for s in i.get(part, []) or []] + [i.get('offhand'), i.get('carried')]
    return sum(s.get('count', 0) for s in stacks if isinstance(s, dict) and str(s.get('item') or '').startswith(tuple(prefixes)))


def held_damage(g):
    s = g.ctl('inventory').get('held') or {}
    return s.get('damage', 0)


def entities(g, prefixes, radius=60):
    found = g.ctl(f'entities {radius}')['entities']
    return sum(e['item']['count'] for e in found if e.get('kind') == 'item' and e['item']['item'].startswith(tuple(prefixes)))


def dial(g, value):
    reply = ' '.join(g.command(f'ntmrbmk dial dialColumnHeight {value}').get('chat', []))
    g.note(f'dial_{value}', reply)
    return reply


def place(g, p, item, side=(0, 3)):
    """`item` placed by the player at `p` (on the stone under it), standing `side` away from it (south by default):
    the side the player stands on gives the facing."""
    # Looking at the cell: from the south north (yaw 180), from the west east (-90), and so on.
    yaw = {(0, 1): 180, (-1, 0): -90, (0, -1): 0, (1, 0): 90}[(max(-1, min(1, side[0])), max(-1, min(1, side[1])))]
    g.command(f'tp @s {p[0] + side[0] + .5} {p[1]} {p[2] + side[1] + .5} {yaw} 45')
    time.sleep(.3)
    r = g.ctl(f'place {p[0]} {p[1]} {p[2]} --item {item}')
    time.sleep(.5)
    return r


def interact(g, p, item, face='south', sneak=False, away=2.5):
    """A use of `item` on the cell `p` of a column, standing on the floor `away` south of it."""
    g.command(f'tp @s {p[0] + .5} {Y} {p[2] + away}')
    time.sleep(.3)
    g.ctl(f'select {item}')
    if sneak:
        g.ctl('key shift down')
        time.sleep(.3)
    g.ctl(f'interact {p[0]} {p[1]} {p[2]} {face}')
    time.sleep(.5)
    if sneak:
        g.ctl('key shift up')
    g.close()
    time.sleep(.3)


def floor(g, x0, x1, z0, z1):
    g.command(f'forceload add {x0} {z0} {x1} {z1}')
    time.sleep(3)
    g.command(f'fill {x0} {Y} {z0} {x1} {Y + 20} {z1} minecraft:air')
    g.command(f'fill {x0} {Y - 1} {z0} {x1} {Y - 1} {z1} minecraft:stone')


def play(g):
    g.ctl('wait-ready 120')
    g.close()
    floor(g, 40, 76, 0, 24)
    g.command('clear @s')
    g.command('gamemode survival @s')
    g.command('effect give @s minecraft:resistance infinite 4 true')
    # Heights: the dial as Java parses it (`Integer.parseInt` or 4, clamped to 2..16), each column as placed.
    heights = {}
    for i, (value, h) in enumerate([('2', 2), ('4', 4), ('16', 16), ('1', 2), ('99', 16), ('abc', 4)]):
        dial(g, value)
        p = (42 + 2 * i, Y, 4)
        g.command('give @s ntm:rbmk_blank 1')
        place(g, p, 'ntm:rbmk_blank')
        ok, got = stands(g, p, 'rbmk_blank', h)
        heights[value] = got
        g.check(f'height_{value}_is_{h}', ok, got)
    g.check('placing_takes_the_item', inv(g, ('ntm:rbmk_blank',)) == 0, inv(g, ('ntm:rbmk_blank',)))
    # The standing columns kept their heights when the dial moved on.
    g.check('standing_columns_keep_their_height', stands(g, (42, Y, 4), 'rbmk_blank', 2)[0] and stands(g, (46, Y, 4), 'rbmk_blank', 16)[0],
            [cells(g, (42, Y, 4), 3), cells(g, (46, Y, 4), 17)])
    # Obstruction: a stone 3 above the cell; the 4 high column does not fit and the item stays in the hand.
    dial(g, '4')
    g.command(f'setblock 56 {Y + 3} 4 minecraft:stone')
    g.command('give @s ntm:rbmk_blank 1')
    try:
        place(g, (56, Y, 4), 'ntm:rbmk_blank')
        g.note('obstructed_place', 'placed')
    except RuntimeError as refused:
        g.note('obstructed_place', str(refused))
    g.check('obstructed_is_refused_with_the_item_kept', cells(g, (56, Y, 4), 3) == ['minecraft:air'] * 3 and inv(g, ('ntm:rbmk_blank',)) == 1,
            [cells(g, (56, Y, 4), 4), inv(g, ('ntm:rbmk_blank',))])
    g.command('clear @s')
    # Facings: four placements looking each way give the same column.
    facings = {}
    for i, side in enumerate(((0, 3), (-3, 0), (0, -3), (3, 0))):
        p = (42 + 4 * i, Y, 10)
        g.command('give @s ntm:rbmk_rod 1')
        place(g, p, 'ntm:rbmk_rod', side)
        ok, got = stands(g, p, 'rbmk_rod', 4)
        facings[str(side)] = g.ctl(f'block {p[0]} {p[1]} {p[2]}').get('state')
        g.check(f'facing_{side[0]}_{side[1]}_same_column', ok, got)
    g.evidence['facings'] = facings
    g.check('four_facings', len(set(facings.values())) == 4, facings)
    # Broken from its base, a middle cell, its top: the whole column goes and one item comes back.
    g.command('give @s minecraft:iron_pickaxe 1')
    for p, dy in [((42, Y, 10), 0), ((46, Y, 10), 2), ((50, Y, 10), 3)]:
        before = inv(g, ('ntm:rbmk_rod',)) + entities(g, ('ntm:rbmk_rod',), 20)
        g.command(f'tp @s {p[0] + .5} {Y} {p[2] + 2.5}')
        g.ctl('select minecraft:iron_pickaxe')
        g.ctl(f'break {p[0]} {p[1] + dy} {p[2]}')
        time.sleep(1.5)
        after = inv(g, ('ntm:rbmk_rod',)) + entities(g, ('ntm:rbmk_rod',), 20)
        gone = cells(g, p, 5)
        g.check(f'broken_from_dy{dy}_whole_and_one_item', all(c == 'minecraft:air' for c in gone) and after - before == 1, [gone, before, after])
    g.command('clear @s')
    # Lids, from the top cell of a 4 high blank: one from the hand, then off with a sneaking screwdriver.
    a, c = (60, Y, 4), (62, Y, 4)
    g.command('give @s ntm:rbmk_blank 1')
    place(g, a, 'ntm:rbmk_blank')
    g.command('give @s ntm:rbmk_control 1')
    place(g, c, 'ntm:rbmk_control')
    g.command(f'give @s {LID} 2')
    g.command('give @s ntm:rbmk_lid_glass 1')
    g.command('give @s ntm:screwdriver 1')
    interact(g, (a[0], Y + 3, a[2]), LID)
    g.check('lid_on_from_the_top_one_taken', lid(g, a) == 'Standard' and inv(g, (LID,)) == 1 + 1, [lid(g, a), inv(g, LIDS)])
    interact(g, (a[0], Y + 2, a[2]), 'ntm:rbmk_lid_glass')
    g.check('no_second_lid', lid(g, a) == 'Standard' and inv(g, ('ntm:rbmk_lid_glass',)) == 1, [lid(g, a), inv(g, LIDS)])
    interact(g, (c[0], Y + 3, c[2]), LID)
    g.check('a_control_rod_takes_no_lid', inv(g, (LID,)) == 2 and lid(g, c) == 'None', [lid(g, c), inv(g, LIDS)])
    g.command(f'tp @s 50.5 {Y} 20.5')
    time.sleep(.5)
    floor_items = entities(g, LIDS)
    interact(g, (a[0], Y + 1, a[2]), 'ntm:screwdriver', sneak=True)
    g.command(f'tp @s 50.5 {Y} 20.5')
    time.sleep(1)
    near = g.ctl('entities 40')['entities']
    dropped = [e for e in near if e.get('kind') == 'item' and e['item']['item'] == LID]
    g.note('lid_drop', dropped)
    g.ctl('select ntm:screwdriver')
    g.check('lid_off_one_item_at_the_top_tool_worn', lid(g, a) == 'None' and entities(g, LIDS) == floor_items + 1 and held_damage(g) == 1,
            [lid(g, a), entities(g, LIDS), held_damage(g), dropped])
    g.command('kill @e[type=minecraft:item]')
    interact(g, (a[0], Y + 1, a[2]), 'ntm:screwdriver', sneak=True)
    g.ctl('select ntm:screwdriver')
    g.check('nothing_to_screw_no_wear', held_damage(g) == 1 and entities(g, LIDS) == 0, [held_damage(g), entities(g, LIDS)])
    # The glass lid stays on for the reopen.
    interact(g, (a[0], Y + 3, a[2]), 'ntm:rbmk_lid_glass')
    g.check('glass_lid_on', lid(g, a) == 'Glass', lid(g, a))
    # A core taken away by a command: its dummies go.
    g.command(f'setblock 44 {Y} 4 minecraft:stone')
    time.sleep(1)
    g.check('orphaned_dummies_go', cells(g, (44, Y + 1, 4), 4) == ['minecraft:air'] * 4, cells(g, (44, Y, 4), 5))
    # Migration: the dial at 6, the 2 high column rebuilt; the 16 high one shrinks too.
    dial(g, '6')
    reply = ' '.join(g.command(f'ntmrbmk migrate 42 {Y} 4 46 {Y} 4').get('chat', []))
    g.note('migrate', reply)
    time.sleep(1)
    g.check('migrated_to_the_dial', stands(g, (42, Y, 4), 'rbmk_blank', 6)[0] and stands(g, (46, Y, 4), 'rbmk_blank', 6)[0],
            [reply, cells(g, (42, Y, 4), 8), cells(g, (46, Y, 4), 8)])
    # Hoppers on the top cells: a storage takes rods (not stone), a fuel channel shows no slot.
    dial(g, '4')
    st, ch = (66, Y, 4), (68, Y, 4)
    g.command('give @s ntm:rbmk_storage 1')
    place(g, st, 'ntm:rbmk_storage')
    g.command('give @s ntm:rbmk_rod 1')
    place(g, ch, 'ntm:rbmk_rod')
    for p in (st, ch):
        g.command(f'setblock {p[0]} {Y + 4} {p[2]} minecraft:hopper[facing=down]')
    g.command(f'item replace block {st[0]} {Y + 4} {st[2]} container.0 with ntm:rbmk_fuel_ueu 1')
    g.command(f'item replace block {st[0]} {Y + 4} {st[2]} container.1 with minecraft:stone 1')
    g.command(f'item replace block {ch[0]} {Y + 4} {ch[2]} container.0 with ntm:rbmk_fuel_ueu 1')
    time.sleep(4)
    storage = ' '.join(g.command(f'data get block {st[0]} {Y} {st[2]} Items').get('chat', []))
    hopper_st = ' '.join(g.command(f'data get block {st[0]} {Y + 4} {st[2]} Items').get('chat', []))
    hopper_ch = ' '.join(g.command(f'data get block {ch[0]} {Y + 4} {ch[2]} Items').get('chat', []))
    channel = ' '.join(g.command(f'data get block {ch[0]} {Y} {ch[2]} Items').get('chat', []))
    g.check('storage_top_takes_rods_only', 'rbmk_fuel_ueu' in storage and 'stone' not in storage and 'stone' in hopper_st, [storage, hopper_st])
    g.check('channel_top_shows_no_slot', 'rbmk_fuel_ueu' in hopper_ch and 'rbmk_fuel' not in channel, [hopper_ch, channel])
    g.evidence['heights'] = heights
    g.ctl('save')


def reopen(g):
    g.ctl('wait-ready 120')
    time.sleep(3)
    reply = ' '.join(g.command('ntmrbmk dial dialColumnHeight').get('chat', []))
    g.check('dial_kept', reply.strip().endswith('= 4'), reply)
    g.check('heights_kept', stands(g, (42, Y, 4), 'rbmk_blank', 6)[0] and stands(g, (48, Y, 4), 'rbmk_blank', 2)[0]
            and stands(g, (50, Y, 4), 'rbmk_blank', 16)[0], [cells(g, (42, Y, 4), 8), cells(g, (48, Y, 4), 3), cells(g, (50, Y, 4), 17)])
    g.check('lid_kept', lid(g, (60, Y, 4)) == 'Glass', lid(g, (60, Y, 4)))
    # A new column now: as tall as the dial (4).
    g.command('gamemode survival @s')
    g.command('give @s ntm:rbmk_blank 1')
    place(g, (72, Y, 4), 'ntm:rbmk_blank')
    ok, got = stands(g, (72, Y, 4), 'rbmk_blank', 4)
    g.check('new_column_after_reopen_as_the_dial', ok, got)


# ----------------------------------------------------------------------------------------------- scenario C
A, B, C = (44, Y, 6), (46, Y, 6), (48, Y, 6)
TOTAL = 3


def ledger(g):
    text = ' '.join(g.command('ntmledger').get('chat', []))
    found = dict(re.findall(r'(\w+)=(\d+)', text))
    return {k: int(v) for k, v in found.items()}


def barrel_lids(g, p):
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    return sum(int(n) for n in re.findall(r'count: ?(\d+)[^{}]*?id: ?"ntm:rbmk_lid"', text))


def count(g):
    held = sum(barrel_lids(g, p) for p in (A, B, C)) if VANILLA else sum(lid(g, p) == 'Standard' for p in (A, B, C))
    return {'columns': held, 'entities': entities(g, (LID,)), 'inventory': inv(g, (LID,)), 'ledger': ledger(g)}


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
    floor(g, 38, 54, 0, 14)
    g.command('clear @s')
    g.command('gamemode survival @s')
    g.command('effect give @s minecraft:resistance infinite 4 true')
    for p in (A, B, C):
        if VANILLA:
            g.command(f'setblock {p[0]} {p[1]} {p[2]} minecraft:barrel')
        else:
            g.command('give @s ntm:rbmk_blank 1')
            place(g, p, 'ntm:rbmk_blank')
    for p in (B, C):
        if VANILLA:
            g.command(f'item replace block {p[0]} {p[1]} {p[2]} container.0 with {LID} 1')
        else:
            g.command(f'give @s {LID} 1')
            interact(g, (p[0], Y + 3, p[2]), LID)
    g.command(f'give @s {LID} 1')
    g.command('give @s ntm:screwdriver 1')
    g.command('give @s minecraft:iron_pickaxe 1')
    g.command(f'tp @s 46.5 {Y} 12.5')
    time.sleep(2)
    base = count(g)
    g.check('base_lids', base['columns'] == 2 and base['inventory'] == 1 and base['entities'] == 0, base)
    g.evidence['base'] = base
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    g.command(f'tp @s 46.5 {Y} 12.5')
    time.sleep(3)
    before = count(g)
    g.check('start_with_three', before['columns'] + before['inventory'] == TOTAL, before)
    if VANILLA:
        g.command(f'item replace block {A[0]} {A[1]} {A[2]} container.0 with {LID} 1')
        g.command(f'clear @s {LID} 1')
        g.command(f'item replace block {B[0]} {B[1]} {B[2]} container.0 with minecraft:air')
        g.command(f'summon minecraft:item {B[0] + .5} {Y + 3.5} {B[2] + .5} {{Item:{{id:"{LID}",count:1}}}}')
        g.command(f'setblock {C[0]} {C[1]} {C[2]} minecraft:air destroy')
    else:
        # Each use until the column shows it (a click sent before the client sees the cell reaches nothing); a repeat
        # is harmless: a column with a lid takes no other, one without gives nothing to the screwdriver.
        # 4 away: the lid the screwdriver takes off falls out of the column's top, not into the player's reach.
        for p, item, sneak, want in [(A, LID, False, 'Standard'), (B, 'ntm:screwdriver', True, 'None')]:
            for attempt in range(4):
                interact(g, (p[0], Y + 2 if not sneak else Y + 1, p[2]), item, sneak=sneak, away=4)
                deadline = time.monotonic() + 3
                while time.monotonic() < deadline and lid(g, p) != want:
                    time.sleep(.3)
                if lid(g, p) == want:
                    break
            g.note(f'uses_{p[0]}', attempt + 1)
        g.command(f'tp @s {C[0] + .5} {Y} {C[2] + 2.5}')
        g.ctl('select minecraft:iron_pickaxe')
        g.ctl(f'break {C[0]} {C[1]} {C[2]}')
    # Away at once, in both worlds: no pickup before the save.
    g.command(f'tp @s 46.5 {Y} 18.5')
    time.sleep(1.5)
    after = count(g)
    g.evidence['after_act'] = after
    g.check('one_lid_each_way', after['columns'] == 1 and after['entities'] == 2 and after['inventory'] == 0, after)
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
    g.command(f'tp @s 46.5 {Y} 18.5')
    now = settled(g)
    now['total'] = now['columns'] + now['entities'] + now['inventory']
    g.evidence['counted' + ('_again' if again else '')] = now
    g.check('no_more_than_twice', now['total'] <= 2 * TOTAL, now)
    g.check('nothing_left_to_spawn', now['ledger'].get('pending') == 0, now['ledger'])


def drain(g):
    g.ctl('wait-ready 120')
    g.command('kill @e[type=minecraft:item]')
    time.sleep(1.5)
    led = ledger(g)
    g.check('nothing_owed_when_the_items_are_gone', led.get('pending') == 0, led)
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t02-' + mode)
    {'play': play, 'reopen': reopen, 'prepare': prepare, 'act': act, 'verify': verify,
     'again': lambda g: verify(g, True), 'drain': drain}[mode](g)
    g.evidence['ok'] = True
    g.save()
