#!/usr/bin/env python3
"""Digamma on the player and its two blocks (docs/onda-2-sublote-2.md, machine_fel) on a disposable native world.

setup          the digamma flame stays on stone and goes when its ground does; a survival player standing in it takes
               0.05 a tick (the digamma diagnostic reads it, the max health modifier is 0.5^d - 1 and the maximum
               falls with it); under Stability, and in creative, none; xanax takes 0.5 off, fmn caps it at 2, 5-HTP
               clears it and the modifier; digamma matter at metadata 6 spreads once to the
               18 cells within two steps (metadata 7) and goes, then those go too;
capture-state  the world is saved with the player's digamma;
check-state    after reopening, the digamma and its modifier are as saved; at 10 the player dies of digamma (the
               Java's death message), and the new body starts at 0 with no modifier.
No player state is injected; time passes by `wait`, the flame doses it.
"""
import json
import re
import sys
import time
from native_game import Game

X, Z = 60, 8
Y = 100
FLAME = (X, Y, Z)
LOOSE = (X + 4, Y, Z)
MATTER = (X + 8, Y + 3, Z)
SAVED = 'digamma-saved.json'


def chat(g, text):
    return ' '.join(g.command(text).get('chat', []))


def is_(g, p, block):
    return 'passed' in chat(g, f'execute if block {p[0]} {p[1]} {p[2]} {block}').lower()


CELLS = [(i, j, k) for i in (-1, 0, 1) for j in (-1, 0, 1) for k in (-1, 0, 1) if 0 < abs(i) + abs(j) + abs(k) < 3]


def count(g, center, block):
    """How many of the 18 cells within two steps of `center` hold `block`."""
    return sum(is_(g, (center[0] + i, center[1] + j, center[2] + k), block) for i, j, k in CELLS)


def diagnostic(g):
    """The digamma diagnostic's reading: (exposure DRX, influence %)."""
    g.command('item replace entity @s hotbar.0 with ntm:digamma_diagnostic 1')
    g.ctl('select 1')
    g.ctl('look-at 20 160 8')
    g.ctl('use-item')
    g.ticks(6)
    text = ' | '.join(g.ctl('chat-log 12').get('chat', []))
    exposure = re.findall(r'Digamma exposure: (?:§.)? ?([\d.]+) DRX', text)
    influence = re.findall(r'Digamma influence: (?:§.)? ?([\d.]+)%', text)
    g.command('item replace entity @s hotbar.0 with minecraft:air')
    if not exposure or not influence:
        return None, None, text[-300:]
    # The log is newest first: the reading just made is the first of each.
    return float(exposure[0]), float(influence[0]), None


def modifier(g):
    """The `ntm:digamma` modifier on max health, None when there is none."""
    text = chat(g, 'attribute @s minecraft:max_health modifier value get ntm:digamma')
    m = re.search(r'is (-?[\d.E-]+)', text)
    return float(m[1]) if m else None


def max_health(g):
    m = re.search(r'is (-?[\d.E-]+)', chat(g, 'attribute @s minecraft:max_health get'))
    return float(m[1]) if m else None


def consistent(d, m):
    """The modifier is 0.5^d - 1 of a digamma the diagnostic shows truncated to the hundredth."""
    return d is not None and m is not None and any(abs(m - (0.5 ** (d + e / 1000) - 1)) < 1e-4 for e in range(0, 11))


def eat(g, item):
    """A pill eaten by the game itself (10 ticks of use; held a second)."""
    g.command(f'item replace entity @s hotbar.0 with {item} 1')
    g.ctl('select 1')
    g.ctl('look-at 20 160 8')
    g.ctl('use-item --hold 1')
    g.ticks(10)


def stand(g, p):
    g.command(f'tp @s {p[0] + .5} {p[1]} {p[2] + .5}')


def away(g):
    g.command(f'tp @s {X + .5} {Y} {Z + 3.5}')


def build(g):
    g.close()
    g.command('gamemode creative @s')
    g.command('effect clear @s')
    # Deep enough for `away` (Z + 3.5): a fall with a digamma-lowered maximum kills.
    g.ground(X - 2, Y, Z - 2, width=16, height=12, depth=8)
    g.command('clear @s')
    # The 200 ticks of a joining body (`ticksExisted < 200`).
    g.ticks(220)


def setup(g):
    build(g)
    g.command(f'setblock {FLAME[0]} {FLAME[1]} {FLAME[2]} ntm:fire_digamma')
    g.command(f'setblock {LOOSE[0]} {LOOSE[1]} {LOOSE[2]} ntm:fire_digamma')
    g.ticks(4)
    g.check('the_flame_stands_on_stone', is_(g, FLAME, 'ntm:fire_digamma') and is_(g, LOOSE, 'ntm:fire_digamma'), None)
    g.command(f'setblock {LOOSE[0]} {LOOSE[1] - 1} {LOOSE[2]} minecraft:air')
    g.ticks(4)
    g.check('it_goes_when_its_ground_does', is_(g, LOOSE, 'minecraft:air'), None)
    g.command(f'setblock {LOOSE[0]} {LOOSE[1] - 1} {LOOSE[2]} minecraft:glass')
    g.command(f'setblock {LOOSE[0]} {LOOSE[1]} {LOOSE[2]} ntm:fire_digamma')
    g.command(f'setblock {LOOSE[0]} {LOOSE[1] - 1} {LOOSE[2]} minecraft:stone')
    g.command(f'setblock {LOOSE[0]} {LOOSE[1] - 1} {LOOSE[2]} minecraft:glass')
    g.ticks(4)
    g.check('glass_is_no_solid_top', is_(g, LOOSE, 'minecraft:air'), None)

    # Creative: nothing.
    stand(g, FLAME)
    g.ticks(40)
    away(g)
    d0, _, raw = diagnostic(g)
    g.check('a_creative_player_takes_none', d0 == 0.0 and modifier(g) is None, [d0, raw])

    # Survival, in the flame for a while: 0.05 a tick.
    g.command('gamemode survival @s')
    g.ticks(2)
    stand(g, FLAME)
    g.ticks(30)
    away(g)
    d1, h1, raw = diagnostic(g)
    m1, top1 = modifier(g), max_health(g)
    g.evidence['first'] = {'digamma': d1, 'influence': h1, 'modifier': m1, 'max_health': top1}
    g.check('the_flame_doses_a_survival_player', d1 is not None and 1.4 <= d1 <= 2.5, [d1, raw])
    g.check('the_modifier_is_half_to_the_digamma', consistent(d1, m1) and top1 is not None and abs(top1 - 20 * (1 + m1)) < 1e-3, [d1, m1, top1])
    g.check('the_influence_is_what_it_takes', h1 is not None and abs(h1 - int((1 - 0.5 ** d1) * 10000) / 100) < 1e-6, [d1, h1])
    g.ticks(40)
    d2, _, _ = diagnostic(g)
    g.check('out_of_it_the_digamma_stays', d2 == d1, [d1, d2])

    # Stability: nothing more.
    g.command('effect give @s ntm:stability 30 0')
    stand(g, FLAME)
    g.ticks(30)
    away(g)
    d3, _, _ = diagnostic(g)
    g.check('stability_holds_it', d3 == d1, [d1, d3])
    g.command('effect clear @s')

    # The pills (ItemPill.onFoodEaten): xanax takes 0.5 off, fmn caps at 2, 5-HTP clears it.
    eat(g, 'ntm:xanax')
    d4, _, raw = diagnostic(g)
    m4 = modifier(g)
    g.check('xanax_takes_half_a_point', d4 is not None and abs(d4 - (d1 - 0.5)) <= 0.0101 and consistent(d4, m4), [d1, d4, m4, raw])
    stand(g, FLAME)
    g.ticks(40)
    away(g)
    d5, _, _ = diagnostic(g)
    eat(g, 'ntm:fmn')
    d6, _, raw = diagnostic(g)
    m6 = modifier(g)
    g.check('fmn_brings_it_down_to_two', d5 is not None and d5 > 2 and d6 == 2.0 and m6 == -0.75, [d5, d6, m6, raw])
    eat(g, 'ntm:five_htp')
    d7, _, raw = diagnostic(g)
    g.check('five_htp_clears_it_and_the_modifier', d7 == 0.0 and modifier(g) is None and max_health(g) == 20.0, [d7, raw])
    g.command('effect clear @s')
    # A dose to keep over the save.
    stand(g, FLAME)
    g.ticks(30)
    away(g)
    g.command('gamemode creative @s')

    # Matter at metadata 6: once out to the 18 cells within two steps, at 7; then those go.
    # Time stopped (`tick freeze`), stepped a tick at a time until the block's own tick takes it: its 18 children
    # are there then, each waiting 10 to 49 ticks for its own; 50 ticks on, all are gone.
    g.command('tick freeze')
    try:
        g.command(f'setblock {MATTER[0]} {MATTER[1]} {MATTER[2]} ntm:digamma_matter__6')
        steps = 0
        while is_(g, MATTER, 'ntm:digamma_matter__6') and steps < 60:
            g.command('tick step 1')
            time.sleep(.3)
            steps += 1
        sevens = count(g, MATTER, 'ntm:digamma_matter__7')
        g.check('matter_spreads_once_to_18_cells', 10 <= steps <= 51 and sevens == 18 and is_(g, MATTER, 'minecraft:air'), [steps, sevens])
        g.command('tick step 50')
        time.sleep(5)
        left = count(g, MATTER, 'minecraft:air')
        g.check('at_7_it_only_goes', left == 18 and is_(g, MATTER, 'minecraft:air'), left)
    finally:
        g.command('tick unfreeze')


def capture(g):
    away(g)
    d, h, raw = diagnostic(g)
    m = modifier(g)
    g.check('the_digamma_is_there_before_the_save', d is not None and d > 1.0 and consistent(d, m), [d, m, raw])
    (g.folder / SAVED).write_text(json.dumps({'digamma': d, 'influence': h, 'modifier': m}))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    g.ticks(220)
    d, h, raw = diagnostic(g)
    m = modifier(g)
    g.check('the_digamma_is_as_saved', d == saved['digamma'] and h == saved['influence'], [saved, d, h, raw])
    g.check('the_modifier_is_as_saved', m is not None and abs(m - saved['modifier']) < 1e-9, [saved, m])
    # To 10: the digamma's blow.
    g.command('gamemode survival @s')
    stand(g, FLAME)
    g.ticks(260)
    said = ' | '.join(g.ctl('chat-log 20').get('chat', []))
    g.check('at_ten_the_player_dies_of_digamma', 'stepped into the abyss' in said, said[-300:])
    g.ctl('respawn')
    g.ctl('wait-ready 60')
    g.ticks(40)
    g.command('gamemode creative @s')
    away(g)
    d2, _, raw = diagnostic(g)
    g.check('the_new_body_starts_at_zero', d2 == 0.0 and modifier(g) is None, [d2, raw])


if __name__ == '__main__':
    g = Game(sys.argv[1], 'digamma-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()
