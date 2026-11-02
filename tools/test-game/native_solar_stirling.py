#!/usr/bin/env python3
"""Batch 4 native game acceptance: real heat/fuel and solar links, no state injection.
Usage: native_solar_stirling.py GAME [iron|steel|creative|solar ...]
After a successful full run: capture-state, quit and reopen that scratch world,
then check-state to verify the machines' saved state survived the restart.
Each Stirling draws heat from a coal firebox and charges a battery through a
side port and red cable. Without a heat source, even creative stops. Mirrors
are linked using the mirror tool; the boiler consumes water from a barrel
below and sends steam into a barrel above. At night the steam stops growing.
"""
import sys
import time
import re
import json
from native_game import Game
from native_recipes import core_of
from native_combustion import start_area, machine_state, first_int, battery_power, grows
from native_stations import fill_barrel, type_duct


def menu_data(game, at):
    text = ' '.join(game.command(f'data get block {at[0]} {at[1]} {at[2]} data').get('chat', []))
    found = re.search(r'\[I;\s*([-\d, ]+)\]', text)
    if not found:
        raise AssertionError('missing native menu data: '+text)
    return [int(v) for v in found.group(1).split(',')]


def place(game, block, x, y, z):
    game.command('clear @s')
    game.command(f'setblock {x} {y-1} {z} minecraft:stone')
    game.command(f'setblock {x} {y-1} {z+3} minecraft:stone')
    game.command(f'give @s ntm:{block} 1')
    game.command(f'tp @s {x+.5} {y} {z+3.5}')
    time.sleep(1)
    game.ctl(f'place {x} {y} {z} --item ntm:{block}')
    time.sleep(.5)
    return core_of(game, 'ntm:'+block, x, y, z, reach=(3, 3))


def stirling(game, shape):
    game.evidence['phase'] = shape
    x, y, z = {'iron': (460,101,8), 'steel': (480,101,8), 'creative': (500,101,8)}[shape]
    start_area(game,x,y,z,12)
    block = 'machine_stirling' + ('' if shape == 'iron' else '_'+shape)
    cx,cy,cz = core = place(game,block,x,y,z)
    source = (cx,cy-1,cz)
    battery = (cx+3,cy,cz)
    game.command(f'setblock {cx} {cy-1} {cz} ntm:heater_firebox[facing=south,honey_level=0]')
    game.command(f'setblock {cx+1} {cy-1} {cz} minecraft:hopper[facing=west]')
    game.command(f'item replace block {cx+1} {cy-1} {cz} container.0 with minecraft:coal 8')
    game.command(f'setblock {cx+2} {cy} {cz} ntm:red_cable')
    game.command(f'setblock {cx+3} {cy} {cz} ntm:machine_battery[facing=south]')
    before,after = grows(game,battery,10)
    state = machine_state(game,core)
    game.evidence[shape] = {'core':core, 'battery_pos':battery, 'battery':[before,after], 'state':state}
    game.check('heat_charges_battery',after > before+500,[before,after])
    game.check('gear_intact','"has_cog":true' in state,state)
    game.command(f'setblock {source[0]} {source[1]} {source[2]} minecraft:air')
    time.sleep(2)
    a,b = grows(game,battery,3)
    game.check('no_generation_without_heat',a == b,[a,b])
    game.snapshot('solar-stirling-'+shape)


def solar(game):
    game.evidence['phase'] = 'solar'
    x,y,z = 530,112,8
    start_area(game,x-6,100,z,20)
    game.command(f'fill {x-5} 107 {z-5} {x+5} 119 {z+5} minecraft:air')
    game.command(f'forceload add {x-8} {z-8} {x+8} {z+8}')
    game.command('time set noon')
    game.command('weather clear')
    game.command('gamerule minecraft:advance_time false')
    core = cx,cy,cz = place(game,'machine_solar_boiler',x,y,z)
    # Remove the placement support so the lower pipe is visible from its walkway.
    game.command(f'setblock {x} {y-1} {z} minecraft:air')
    water, steam = (cx,cy-2,cz),(cx,cy+4,cz)
    for p in (water,steam): game.command(f'setblock {p[0]} {p[1]} {p[2]} ntm:barrel_steel[facing=south]')
    for yy in (cy-1,cy+3): game.command(f'setblock {cx} {yy} {cz} ntm:fluid_duct_neo')
    # Walkways keep the player in reach of the elevated tower and its pipes.
    for yy,zz in ((cy-3,cz+3),(cy+2,cz+3),(cy-1,cz+4)):
        game.command(f'setblock {cx} {yy} {zz} minecraft:stone')
    type_duct(game,'water',[(cx,cy-1,cz)],(cx,cy-2,cz+3))
    type_duct(game,'steam',[(cx,cy+3,cz)],(cx,cy+3,cz+3))
    game.command(f'tp @s {cx+.5} {cy-2} {cz+3.5}')
    fill_barrel(game,water,1)
    # Leave room to approach each mirror and keep the walkways out of its sky column.
    mirrors = [(cx+dx,100,cz+dz) for dx in (-6,-3,3,6) for dz in (-6,-3,0,3,6)][:13]
    for px,py,pz in mirrors: game.command(f'setblock {px} {py} {pz} ntm:solar_mirror[facing=south,honey_level=0]')
    game.command('clear @s')
    game.command('give @s ntm:mirror_tool 1')
    game.ctl('select ntm:mirror_tool')
    # Open the exposed south cell of the boiler's symmetric 3x3 body,
    # regardless of the facing chosen during placement.
    game.command(f'tp @s {cx+.5} {cy} {cz+4.5}')
    game.ctl(f'interact {cx} {cy+1} {cz+1} south')
    for px,py,pz in mirrors:
        game.command(f'tp @s {px+.5} {py} {pz+2.5}')
        game.ctl(f'interact {px} {py} {pz} south')
    time.sleep(3)
    linked = machine_state(game,mirrors[0])
    game.check('tool_linked','"target":['+','.join(str(v) for v in (cx,cy+1,cz))+']' in linked,linked)
    daylight = menu_data(game,mirrors[0])
    game.check('mirror_sees_daylight',daylight[-1] == 1,daylight)
    def amount(): return first_int(machine_state(game,steam),r'"tank":\{"fluid":"steam","mb":(\d+)') or 0
    a=amount(); time.sleep(4); b=amount()
    game.evidence['solar']={'core':core,'steam_barrel':steam,'steam':[a,b],'mirror':linked,'mirrors':mirrors}
    game.check('sun_makes_steam',b > a,[a,b])
    game.command('time set midnight'); time.sleep(2)
    a=amount(); time.sleep(3); b=amount()
    game.check('night_stops_steam',a==b,[a,b])
    night = menu_data(game,mirrors[0])
    game.check('mirror_off_at_night',night[-1] == 0,night)
    # An opaque roof blocks the sky independently of the time of day.
    for px,py,pz in mirrors:
        game.command(f'setblock {px} {py+1} {pz} minecraft:stone')
    game.command('time set noon'); time.sleep(2)
    roof = menu_data(game,mirrors[0])
    game.check('opaque_roof_blocks_sun',roof[-1] == 0,roof)
    game.snapshot('solar-boiler')


def persistence(game, capture):
    proof = json.loads((game.folder/'native-solar-stirling.json').read_text())
    if not proof['ok'] or not all(shape in proof for shape in ('iron','steel','creative','solar')):
        raise AssertionError('run all four phases successfully before checking persistence')
    positions = [proof[shape][key] for shape in ('iron','steel','creative') for key in ('core','battery_pos')]
    positions += [proof['solar']['core'], proof['solar']['steam_barrel'], *proof['solar']['mirrors']]
    states = {}
    for pos in positions:
        text = machine_state(game,pos)
        states[','.join(map(str,pos))] = json.JSONDecoder().raw_decode(text[text.index('{'):])[0]
    path = game.folder/'native-solar-stirling-state.json'
    if capture:
        path.write_text(json.dumps(states,indent=2)+'\n')
        game.ctl('save')
        game.evidence['saved_entities'] = len(states)
    else:
        expected = json.loads(path.read_text())
        for pos, state in expected.items():
            game.check('saved_'+pos,states.get(pos) == state,states.get(pos))


if __name__ == '__main__':
    phases=sys.argv[2:] or ['iron','steel','creative','solar']
    mode = phases[0] if len(phases)==1 and phases[0] in ('capture-state','check-state') else None
    g=Game(sys.argv[1],'solar-stirling'+('-'+mode if mode else ''))
    g.ctl('wait-ready 120')
    if mode:
        persistence(g,mode=='capture-state')
    else:
        for phase in phases:
            if phase=='solar': solar(g)
            elif phase in ('iron','steel','creative'): stirling(g,phase)
            else: raise SystemExit('unknown phase: '+phase)
    g.evidence['ok']=True
    g.save()
