#!/usr/bin/env python3
"""Real use animations, counted commits, armor-table UI and reopen acceptance."""
import json
import re
import sys
import time
from pathlib import Path
from native_game import Game


def counts(g):
    return g.ctl('inventory')['counts']


def state(g):
    text=' '.join(g.command('ntmitem inspect').get('chat',[]))
    result={key:int(value) for key,value in re.findall(r'(dose|radaway|sickness):\s*(\d+)[lL]?',text)}
    if 'dose' not in result:raise AssertionError(text)
    return result


def use(g,seconds=0):
    g.close();g.ctl('look-at 20 160 8')
    g.ctl('mouse right down');time.sleep(seconds or .12);g.ctl('mouse right up');time.sleep(.4)


def give_hand(g,item,count=1,slot=0):
    g.command(f'item replace entity @s hotbar.{slot} with {item} {count}')
    g.ctl(f'select {slot+1}')


def inventory_tag(g):
    return ' '.join(g.command('data get entity @s Inventory').get('chat',[]))


def setup(g):
    g.ctl('wait-ready 120');g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.ground(20,101,8,width=12,height=8)
    # The cotton candy is always edible and carries the five Java effects.
    give_hand(g,'ntm:cotton_candy',2)
    g.ctl('look-at 20 160 8');g.ctl('mouse right down');time.sleep(.45)
    active=g.ctl('state');g.check('real_eating_started',active.get('using') is not None,active)
    g.ctl('mouse right up');time.sleep(.4)
    g.check('cancel_does_not_consume',counts(g).get('ntm:cotton_candy')==2,counts(g))
    g.command('gamemode survival @s')
    g.command('effect give @s minecraft:hunger 1 255 true');time.sleep(1.6)
    before=g.ctl('state')['player'];use(g,1.8)
    after=g.ctl('state')['player']
    g.check('food_and_saturation_apply',after['food']>before['food'] and after['saturation']>before['saturation'],{'before':before,'after':after})
    g.check('only_one_food_consumed',counts(g).get('ntm:cotton_candy')==1,counts(g))
    effects={e['id'].split(':')[-1]:e['amplifier'] for e in after['effects']}
    g.check('java_food_effects',all(effects.get(k)==v for k,v in {'poison':0,'wither':0,'weakness':2,'speed':2,'resistance':4}.items()),effects)
    g.command('effect clear @s');g.command('clear @s');g.command('gamemode creative @s')
    # A stacked syringe cannot lose its container in a full inventory.
    give_hand(g,'ntm:syringe_metal_medx',2)
    for slot in [f'hotbar.{i}' for i in range(1,9)]+[f'inventory.{i}' for i in range(27)]:
        g.command(f'item replace entity @s {slot} with minecraft:stone 64')
    use(g)
    g.check('full_inventory_refuses_entire_syringe',counts(g).get('ntm:syringe_metal_medx')==2 and not counts(g).get('ntm:syringe_metal_empty'),counts(g))
    g.command('clear @s minecraft:stone 64');use(g)
    actual=counts(g);g.check('syringe_count_and_container',actual.get('ntm:syringe_metal_medx')==1 and actual.get('ntm:syringe_metal_empty')==1,actual)
    effects=g.ctl('state')['player']['effects']
    g.check('medx_resistance_three',any(e['id'].endswith('resistance') and e['amplifier']==2 for e in effects),effects)
    g.command('effect clear @s');g.command('clear @s')
    # Kit yields the four equipped pieces, no partial loot.
    give_hand(g,'ntm:hazmat_kit');use(g)
    g.check('kit_yields_four_pieces',all(counts(g).get('ntm:hazmat_'+p)==1 for p in ['helmet','plate','legs','boots']) and not counts(g).get('ntm:hazmat_kit'),counts(g))
    g.command('clear @s');g.command('item replace entity @s armor.head with ntm:gas_mask_m65')
    give_hand(g,'ntm:gas_mask_filter');use(g)
    g.check('filter_installed_once',not counts(g).get('ntm:gas_mask_filter') and 'hfrFilter' in inventory_tag(g),inventory_tag(g))
    g.command('ntmitem expose lung 123')
    worn=inventory_tag(g);g.check('exposure_wears_filter','minecraft:damage' in worn and '123' in worn,worn)
    before=worn;g.command('ntmitem expose blister 100');g.check('mask_blacklist_refuses_blister',inventory_tag(g)==before,inventory_tag(g))
    # Real gamma dose uses the same equipped-armor resolver as other sources.
    g.command('gamemode survival @s');give_hand(g,'ntm:ingot_uranium');time.sleep(11)
    def rate_sample():
        a=state(g)['dose'];time.sleep(3.1);b=state(g)['dose'];return b-a
    mask=rate_sample()
    g.command('item replace entity @s armor.head with minecraft:air');bare=rate_sample()
    g.command('item replace entity @s armor.chest with ntm:hazmat_plate');hazmat=rate_sample()
    g.check('mask_and_hazmat_reduce_gamma',0<mask<bare and 0<hazmat<bare,{'mask':mask,'bare':bare,'hazmat':hazmat})
    g.command('gamemode creative @s');g.command('clear @s');g.command('ntmitem dose 200000')
    give_hand(g,'ntm:radaway');use(g);time.sleep(7.5)
    g.check('radaway_exact_java_total',state(g)['dose']==60000,state(g))
    g.check('radaway_returns_iv_empty',counts(g).get('ntm:iv_empty')==1,counts(g))
    armor_table(g)


def armor_table(g):
    g.close();g.command('clear @s');g.command('setblock 24 101 8 ntm:machine_armor_table')
    g.command('tp @s 24.5 101 11.5');g.ctl('interact 24 101 8 south');time.sleep(.7)
    g.check('table_has_exact_14_slots',len(g.slots())==14,g.ctl('screen'))
    g.put('ntm:hazmat_helmet',1,9);g.put('ntm:attachment_mask',1,0)
    g.check('mod_slot_shows_attachment',g.slots()[0].get('item')=='ntm:attachment_mask',g.slots())
    g.ctl('click-slot 309 shift');time.sleep(.5)
    g.check('shift_click_equips_armor_and_absorbs_views',not g.slots()[0].get('item') and not g.slots()[9].get('item') and g.slots()[10].get('item')=='ntm:hazmat_helmet',g.slots())
    g.snapshot('items-armor-table')
    g.close();give_hand(g,'ntm:gas_mask_filter');use(g)
    g.command('ntmitem expose lung 321')
    tag=inventory_tag(g);g.check('nested_mod_filter_is_active','ntm_armor_mods' in tag and 'hfrFilter' in tag and '321' in tag,tag)
    # Taking the module out changes the helmet NBT in the same UI operation.
    g.ctl('interact 24 101 8 south');time.sleep(.5)
    g.ctl('click-slot 310 shift');time.sleep(.4)
    g.ctl('click-slot 300');time.sleep(.3)
    target=next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot']<36 and not s.get('item'))
    g.ctl(f'click-slot {target}');time.sleep(.4)
    g.check('module_removed_into_inventory',not g.slots()[0].get('item') and counts(g).get('ntm:attachment_mask')==1,{'slots':g.slots(),'counts':counts(g)})
    # Reinstall, then leave the central armor in the menu on close. Java drops
    # the separated module and armor, so count both pickups and inventory.
    source=next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot']<36 and s.get('item')=='ntm:attachment_mask')
    g.ctl(f'click-slot {source}');g.ctl('click-slot 300');time.sleep(.4);g.close();time.sleep(.5)
    entities=g.ctl('entities')['entities'];drops=[e.get('item') or {} for e in entities]
    total=lambda item:counts(g).get(item,0)+sum(x.get('count',0) for x in drops if x.get('item')==item)
    g.check('table_close_conserves_armor_and_mod',total('ntm:hazmat_helmet')==1 and total('ntm:attachment_mask')==1,{'counts':counts(g),'entities':entities})


def capture(g):
    g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.command('item replace entity @s armor.head with ntm:gas_mask_m65')
    give_hand(g,'ntm:gas_mask_filter');use(g);g.command('ntmitem expose lung 777')
    give_hand(g,'minecraft:stick');g.command('ntmitem charge 123456789')
    # Two doses accumulate treatment time. The invariant detects timer reset,
    # lost dose or replayed treatment across the normal world close/reopen.
    g.command('ntmitem dose 500000');give_hand(g,'ntm:radaway',2,1);use(g);use(g)
    current=state(g);invariant=current['dose']-1000*current.get('radaway',0)
    g.check('radaway_treatment_accumulates',invariant==220000,current)
    expected={'invariant':invariant,'dose':current['dose'],'radaway':current.get('radaway',0)}
    (g.folder/'items-expected.json').write_text(json.dumps(expected,indent=2))
    g.snapshot('items-before-reopen')


def reopened(g):
    g.ctl('wait-ready 120');g.close()
    expected=json.loads((g.folder/'items-expected.json').read_text());current=state(g)
    g.check('treatment_resumes_without_replay',current['dose']-1000*current.get('radaway',0)==expected['invariant'] and current.get('radaway',0)<=expected['radaway'],{'before':expected,'after':current})
    tag=inventory_tag(g)
    g.check('filter_wear_and_stack_charge_survive','777' in tag and '123456789' in tag and 'ntm_charge' in tag and 'hfrFilter' in tag,tag)
    g.snapshot('items-reopened')


if __name__=='__main__':
    g=Game(sys.argv[1],'items');mode=sys.argv[2] if len(sys.argv)>2 else 'setup';g.evidence['phase']=mode
    {'setup':setup,'capture-state':capture,'check-state':reopened}[mode](g)
    g.evidence['ok']=True;g.save()
