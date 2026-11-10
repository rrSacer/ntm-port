#!/usr/bin/env python3
"""The game side of run-filter-copy-proof.py: the Java's filter-copy bug (JAVA_FILTER_COPY_BUG) cut by a save.

Modes (the runner owns the process and the kill):
  prepare  a survival player wearing a hazmat helmet whose attachment mask holds the only gas mask filter; save
  act      the mod: a lung gas exposure (`/ntmitem expose lung 40`): the worn copy goes into the helmet, the mod keeps
           its filter (2 filters). The vanilla control (NTM_CRASH_VANILLA=1): `/give` of one filter (2 filters).
           Then a save the armed failpoint may end (NTM_CRASH_NO_SAVE=1: no save; NTM_CRASH_PAUSE_SAVE=1: a pause)
  verify   after a reopen: count every filter the player holds, nested ones included
  again    verify once more after a second clean reopen
"""
import os
import re
import sys
import time
from native_game import Game

FILTER = 'ntm:gas_mask_filter'
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
MODULE = '{id:"ntm:attachment_mask",count:1,components:{"minecraft:custom_data":{hfrFilter:{id:"' + FILTER + '",count:1}}}}'
HELMET = 'ntm:hazmat_helmet[minecraft:custom_data={ntm_armor_mods:{mod_slot_0:' + MODULE + '}}]'


def filters(g):
    """Every filter the player holds: loose stacks, and the ones nested in a stack's components (a mask's
    `hfrFilter`, a mod's)."""
    inv = g.ctl('inventory')
    total = 0
    for group in ('hotbar', 'main', 'armor', 'offhand'):
        stacks = inv.get(group) or []
        stacks = stacks if isinstance(stacks, list) else [stacks]
        for s in stacks:
            if not isinstance(s, dict):
                continue
            if s.get('item') == FILTER:
                total += s.get('count', 0)
            total += len(re.findall(r'id:"' + re.escape(FILTER) + '"', s.get('components') or ''))
    return total


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    g.command('gamemode survival @s')
    g.command('clear @s')
    g.command('item replace entity @s armor.head with ' + HELMET)
    time.sleep(1.5)
    g.evidence['base'] = filters(g)
    g.check('one_filter_in_the_mod', g.evidence['base'] == 1, g.ctl('inventory'))
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    # The grace after joining protects the player from nothing here: the exposure is the filter path.
    before = filters(g)
    g.check('start_from_one_filter', before == 1, g.ctl('inventory'))
    if VANILLA:
        g.command('give @s ' + FILTER + ' 1')
    else:
        reply = ' '.join(g.command('ntmitem expose lung 40').get('chat', []))
        g.check('the_mod_filter_protects', 'true' in reply.lower(), reply)
    time.sleep(1.5)
    g.evidence['after_act'] = {'total': filters(g)}
    g.check('two_filters_after_the_act', g.evidence['after_act']['total'] == 2, g.ctl('inventory'))
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
    except Exception as error:  # the failpoint ends the process inside the save
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    time.sleep(2)
    now = {'total': filters(g)}
    g.evidence['counted' + ('_again' if again else '')] = now
    # Only the Java's own copy: never more than two, never fewer than the one the base held.
    g.check('one_or_two_filters', now['total'] in (1, 2), now)


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'filter-copy-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True)}[mode](g)
    g.evidence['ok'] = True
    g.save()
