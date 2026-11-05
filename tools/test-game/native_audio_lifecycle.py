#!/usr/bin/env python3
"""Active software mixer acceptance through NTM's real client loop owner.

Uses the normal device mixer/decoder with PCM discarded, never hardware output.
Machines and nuclear messages come from ordinary blocks, slots and interaction.
"""
import json
from pathlib import Path
import re
import sys
import time
from native_game import Game
from native_recipes import screen_json
from native_solar_stirling import place
from native_combustion import facing_of, FACING
from native_bombs import assemble, lever


def audio(g):
    return g.ctl('status')['sounds']['diagnostic']


def voices(g, event=None):
    return [v for v in audio(g)['voices'] if v['event'] and v['event'].startswith('ntm:')
            and (event is None or v['event'] == event)]


def wait(g, name, predicate, actual, seconds=25):
    until = time.monotonic() + seconds
    value = actual()
    while not predicate(value) and time.monotonic() < until:
        time.sleep(.15)
        value = actual()
    g.check(name, predicate(value), value)
    return value


def destroyed(g, ids):
    history = audio(g)['audit']
    return {channel: [e['action'] for e in history if e['channel'] == channel] for channel in ids}


def await_destroy(g, ids, name, require_stop=False):
    return wait(g, name, lambda actions: all('destroy' in a and (not require_stop or 'stop' in a) for a in actions.values()),
                lambda: destroyed(g, ids))


def replace(g, at, slot, item, count=1):
    if getattr(g, 'audio_arc_core', None) == at:
        fx, _, fz = FACING[facing_of(g, at)]
        g.close()
        g.command(f'tp @s {at[0]-4*fx+.5} {at[1]} {at[2]-4*fz+.5}')
        g.ctl(f'interact {at[0]-2*fx} {at[1]} {at[2]-2*fz}')
        time.sleep(.3)
    else:
        g.open(*at)
    g.put(item, count, slot)
    g.close()


def diesel(g, x):
    g.command(f'setblock {x} 100 8 ntm:machine_diesel[facing=south,honey_level=0]')
    g.command(f'setblock {x+1} 100 8 ntm:red_cable')
    g.command(f'setblock {x+2} 100 8 ntm:machine_battery[facing=south]')
    g.open(x, 100, 8)
    g.put('ntm:canister_full__diesel', 64, 0)
    screen = screen_json('machine_diesel')
    g.button(next(b['rect'] for b in screen['buttons'] if b['id'] == 0), screen['size'])
    g.close()


def loop_lifecycle(g):
    g.evidence['phase'] = 'owner-lifecycle'
    g.ground(256, 100, 8, width=20, depth=12)
    diesel(g, 260)
    diesel(g, 264)
    g.command('tp @s 262.5 100 11.5')
    pair = wait(g, 'two_distinct_diesel_channels', lambda v: len(v) == 2 and all(x['decoded'] and x['state'] == 'Playing' for x in v),
                lambda: voices(g, 'ntm:block.engine'))
    ids = {v['channel'] for v in pair}
    g.check('owner_positions_are_distinct', {tuple(v['at']) for v in pair} == {(260., 100., 8.), (264., 100., 8.)}, pair)
    time.sleep(1)
    same = voices(g, 'ntm:block.engine')
    g.check('no_duplicate_or_restarted_loops', {v['channel'] for v in same} == ids and len(same) == 2, same)
    g.check('real_pcm_is_mixed', audio(g)['output_peak'] > 0, audio(g)['output_peak'])
    original = {v['channel']: v['gain'] for v in same}
    g.command('tp @s 262.5 100 15.5')
    farther = wait(g, 'active_gain_updates', lambda v: len(v) == 2 and all(0 < x['gain'] < original[x['channel']] for x in v),
                   lambda: voices(g, 'ntm:block.engine'))
    g.ctl('key esc tap')
    paused = wait(g, 'pause_keeps_channels', lambda v: len(v) == 2 and {x['channel'] for x in v} == ids and all(x['state'] == 'Paused' for x in v),
                  lambda: voices(g, 'ntm:block.engine'))
    cursor = {v['channel']: v['sample'] for v in paused}
    time.sleep(.7)
    held = voices(g, 'ntm:block.engine')
    g.check('pause_preserves_sample_offsets', {v['channel']: v['sample'] for v in held} == cursor, held)
    g.ctl('resume')
    wait(g, 'resume_keeps_ids_and_advances_offsets', lambda v: len(v) == 2 and {x['channel'] for x in v} == ids and all(x['state'] == 'Playing' and x['sample'] != cursor[x['channel']] for x in v),
         lambda: voices(g, 'ntm:block.engine'))
    g.command('forceload remove all')
    g.command('tp @s 2048.5 100 8.5')
    wait(g, 'client_chunk_really_unloaded', lambda b: b['loaded'] is False, lambda: g.ctl('block 260 100 8'), 60)
    wait(g, 'chunk_unload_stops_loops', lambda v: not v, lambda: voices(g, 'ntm:block.engine'), 60)
    await_destroy(g, ids, 'unloaded_channels_destroyed')
    g.command('tp @s 262.5 100 11.5')
    returned = wait(g, 'chunk_return_creates_exactly_two_new_channels', lambda v: len(v) == 2 and not ids.intersection(x['channel'] for x in v),
                    lambda: voices(g, 'ntm:block.engine'), 60)
    ids = {v['channel'] for v in returned}
    g.command('execute in minecraft:the_nether run tp @s 8.5 140 8.5')
    wait(g, 'dimension_change_stops_loops', lambda v: not v, lambda: voices(g, 'ntm:block.engine'), 60)
    await_destroy(g, ids, 'dimension_channels_destroyed')
    g.command('execute in minecraft:overworld run tp @s 262.5 100 11.5')
    pair = wait(g, 'dimension_return_no_duplicates', lambda v: len(v) == 2, lambda: voices(g, 'ntm:block.engine'), 60)
    for x in [260, 264]:
        g.command(f'setblock {x} 100 8 minecraft:air')
    await_destroy(g, {v['channel'] for v in pair}, 'removed_machine_channels_destroyed', require_stop=True)


def pitch_and_arc(g):
    g.evidence['phase'] = 'active-parameters'
    g.ground(280, 100, 8, width=22, depth=10)
    g.command('setblock 280 100 8 ntm:machine_centrifuge[facing=south]')
    g.command('setblock 281 100 8 ntm:machine_battery[facing=south]')
    replace(g, (281, 100, 8), 0, 'ntm:battery_creative')
    g.battery_output(281, 100, 8)
    g.command('tp @s 280.5 100 11.5')
    time.sleep(1)
    replace(g, (280, 100, 8), 0, 'ntm:chunk_ore__rare', 64)
    first = wait(g, 'centrifuge_channel_created_from_real_processing', lambda v: len(v) == 1 and v[0]['decoded'], lambda: voices(g, 'ntm:block.centrifugeoperate'))
    initial = first[0]
    wait(g, 'active_pitch_changes_on_same_channel', lambda v: len(v) == 1 and v[0]['channel'] == initial['channel'] and v[0]['pitch'] > initial['pitch'],
         lambda: voices(g, 'ntm:block.centrifugeoperate'))
    g.command('setblock 280 100 8 minecraft:air')
    await_destroy(g, {initial['channel']}, 'centrifuge_channel_destroyed', require_stop=True)
    core = tuple(place(g, 'machine_arc_furnace_large', 292, 100, 8))
    g.audio_arc_core = core
    for slot in range(3): replace(g, core, slot, 'ntm:arc_electrode__graphite')
    replace(g, core, 3, 'ntm:battery_creative')
    g.command(f'tp @s {core[0]+.5} {core[1]} {core[2]+4.5}')
    replace(g, core, 25, 'minecraft:sand', 64)
    lid = wait(g, 'arc_lid_slot_creates_channel', lambda v: len(v) == 1, lambda: voices(g, 'ntm:door.wgh_start'))[0]
    hum = wait(g, 'arc_processing_slot_creates_independent_channel', lambda v: len(v) == 1 and v[0]['decoded'], lambda: voices(g, 'ntm:block.electrichum'))[0]
    g.check('arc_slots_use_distinct_instances_and_channels', lid['instance'] != hum['instance'] and lid['channel'] != hum['channel'], [lid, hum])
    await_destroy(g, {lid['channel']}, 'stopping_lid_destroys_only_its_channel', require_stop=True)
    live = voices(g, 'ntm:block.electrichum')
    g.check('arc_processing_survives_lid_stop', len(live) == 1 and live[0]['channel'] == hum['channel'] and live[0]['state'] == 'Playing', live)
    g.command(f'setblock {core[0]} {core[1]} {core[2]} minecraft:air')
    await_destroy(g, {hum['channel']}, 'arc_processing_channel_destroyed', require_stop=True)


def door(g):
    g.evidence['phase'] = 'door-cardinality'
    g.ground(20, 100, 8, width=8)
    g.command('give @s ntm:door_metal 1')
    g.ctl('place 20 100 8 --item ntm:door_metal')
    g.command('clear @s'); g.command('tp @s 22.5 100 11.5')
    for action, state in [('open', 'true'), ('close', 'false')]:
        before = max((e['seq'] for e in audio(g)['audit']), default=0)
        g.ctl('interact 20 100 8')
        def starts():
            return [e for e in audio(g)['audit'] if e['seq'] > before and e['action'] == 'start' and (e['file'] or '').startswith('ntm:sounds/block/door_open_')]
        wait(g, 'two_channels_per_' + action, lambda v: len(v) == 2, starts)
        time.sleep(.5)
        g.check('exactly_two_requests_per_' + action, len(starts()) == 2, starts())
        block = g.ctl('block 20 100 8')
        g.check('one_state_transition_per_' + action, f'open={state}' in block.get('state', ''), block)
        await_destroy(g, {e['channel'] for e in starts()}, 'door_channels_finish_' + action)


def nuclear_and_close(g):
    g.evidence['phase'] = 'nuclear-message-and-world-close'
    g.command('ntmbomba config mundo raiomax 12')
    core = assemble(g, 500)
    offset = len((g.folder/'client-first.log').read_text(errors='replace'))
    lever(g, core)
    wait(g, 'actual_nuclear_message_reaches_client', lambda s: 'ntm nuclear received' in s and 'ntm nuclear arrived' in s,
         lambda: (g.folder/'client-first.log').read_text(errors='replace')[offset:], 90)
    text = (g.folder/'client-first.log').read_text(errors='replace')[offset:]
    age, distance = re.search(r'ntm nuclear arrived age=(\d+) distance=([\d.eE+-]+)', text).groups()
    g.check('nuclear_distance_and_age_gate', 0 < int(age) < 150 and float(distance) < (int(age)*1.5+1)*1.5, [age, distance])
    nuke = wait(g, 'message_creates_decoded_stream_channel', lambda v: len(v) == 1 and v[0]['decoded'] and v[0]['state'] == 'Playing', lambda: voices(g, 'ntm:weapon.nuclearexplosion'))
    g.check('nuclear_not_a_playsound_shortcut', not any('playsound' in step['command'] for step in g.evidence['steps']), core)
    # Keep a real NTM-owned loop alive while leaving the world, too.
    g.ground(256, 100, 8, width=20)
    diesel(g, 260)
    active = wait(g, 'loop_before_world_close', lambda v: len(v) == 1, lambda: voices(g, 'ntm:block.engine'))
    ids = {v['channel'] for v in active+nuke}
    closing_offset = len((g.folder/'client-first.log').read_text(errors='replace'))
    g.ctl('key esc tap'); time.sleep(.3)
    widgets = g.ctl('ui')['widgets']
    quit_button = next(w for w in widgets if any(word in str(w.get('label', '')).lower() for word in ['quit to', 'salvar e sair']))
    g.ctl('click ' + quit_button['id'])
    wait(g, 'world_close_releases_all_ntm_channels', lambda v: not v, lambda: voices(g), 60)
    await_destroy(g, ids, 'world_close_destroys_active_channels')
    wait(g, 'world_close_releases_retained_instances', lambda n: n == 0, lambda: g.ctl('status')['sounds']['instances'])
    text = (g.folder/'client-first.log').read_text(errors='replace')[closing_offset:]
    g.check('ntm_owner_releases_instances_on_close', 'ntm audio release' in text, text[-3000:])


def setup(g):
    g.ctl('wait-ready 120'); g.close()
    g.check('software_output_is_active', audio(g)['active'], audio(g))
    loop_lifecycle(g)
    pitch_and_arc(g)
    door(g)
    nuclear_and_close(g)
    g.evidence['ok'] = True; g.save()


if __name__ == '__main__': setup(Game(Path(sys.argv[1]), 'audio-lifecycle'))
