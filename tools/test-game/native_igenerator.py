#!/usr/bin/env python3
"""The industrial generator memorial (docs/onda-2-sublote-2.md, group 2b) on a disposable native world.

setup          placed, it is one cell (no dummy around it); a click opens nothing (its openGui is commented out in the
               1.7.10); looked at from close, the client draws its ILookOverlay title and line right of the crosshair
               (the screenshot is the evidence; the HUD has no widget to read);
capture-state  the world is saved;
check-state    after reopening, it is still there, one cell, and a click still opens nothing.
"""
import sys
import time
from native_game import Game

X, Y, Z = 60, 100, 8
AT = (X, Y, Z)


def is_(g, p, block):
    return 'passed' in ' '.join(g.command(f'execute if block {p[0]} {p[1]} {p[2]} {block}').get('chat', [])).lower()


def one_cell(g):
    return not any(is_(g, (X + dx, Y + dy, Z + dz), 'ntm:dummy_machine') for dx in (-1, 0, 1) for dy in (0, 1) for dz in (-1, 0, 1))


def click_opens_nothing(g):
    g.close()
    g.command(f'tp @s {X + .5} {Y} {Z + 2.5} facing {X + .5} {Y + .25} {Z + .5}')
    time.sleep(1)
    g.ctl(f'interact {X} {Y} {Z} south')
    time.sleep(1.5)
    return g.ctl('screen').get('screen')


def setup(g):
    g.close()
    g.ground(X, Y, Z, width=8)
    g.command('clear @s')
    g.command('give @s ntm:machine_industrial_generator 1')
    g.command(f'tp @s {X + .5} {Y} {Z + 3.5} facing {X + .5} {Y} {Z}')
    time.sleep(1)
    g.ctl(f'place {X} {Y} {Z} --item ntm:machine_industrial_generator')
    time.sleep(1.5)
    g.command('clear @s')
    g.check('it_is_placed_in_one_cell', is_(g, AT, 'ntm:machine_industrial_generator') and one_cell(g), None)
    screen = click_opens_nothing(g)
    g.check('a_click_opens_nothing', screen != 'container', screen)
    # Looked at from 2 blocks (within reach): the overlay.
    g.command('gamemode survival @s')
    g.command(f'tp @s {X + .5} {Y} {Z + 2.5}')
    time.sleep(1)
    g.evidence['look'] = g.ctl(f'look-at block {X} {Y} {Z}')
    time.sleep(1)
    g.snapshot('igenerator-overlay')
    g.command('gamemode creative @s')


def capture(g):
    g.check('it_is_there_before_the_save', is_(g, AT, 'ntm:machine_industrial_generator'), None)
    g.ctl('save')


def check(g):
    g.check('it_is_there_after_reopening_in_one_cell', is_(g, AT, 'ntm:machine_industrial_generator') and one_cell(g), None)
    screen = click_opens_nothing(g)
    g.check('a_click_still_opens_nothing', screen != 'container', screen)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'igenerator-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()
