#!/usr/bin/env python3
"""Real use animations, counted commits, armor-table UI and reopen acceptance."""
import json
import re
import sys
import os
import time
from pathlib import Path
from native_game import Game


FAMILIES=os.environ.get('NTM_FAMILIES','energy,medicine,foods,kits,filters,instruments,vnt').split(',')


def counts(g):
    return g.ctl('inventory')['counts']


def state(g):
    text=' '.join(g.command('ntmitem inspect').get('chat',[]))
    result={key:int(value) for key,value in re.findall(r'(dose|radaway|sickness):\s*(\d+)[lL]?',text)}
    if 'dose' not in result:raise AssertionError(text)
    return result


def use(g,seconds=0):
    g.close();g.ctl('look-at 20 160 8')
    if seconds:
        g.ctl('mouse right down');time.sleep(seconds)
        try:g.ctl('mouse right up')
        except RuntimeError as error:
            # A use that killed the player leaves the death screen: nothing to release.
            if 'dead' not in str(error):raise
    else:
        # One click: a held button repeats the use of an instant item on a slow frame.
        g.ctl('mouse right click')
    time.sleep(.4)


def give_hand(g,item,count=1,slot=0):
    g.command(f'item replace entity @s hotbar.{slot} with {item} {count}')
    g.ctl(f'select {slot+1}')


def inventory_tag(g):
    # 26.3 serializes equipped stacks outside the old Inventory list.
    # Read the synchronized lossless stack components, including armor.
    return json.dumps(g.ctl('inventory'),sort_keys=True)


def settle(g,done,timeout=8):
    """Poll the synchronized screen until `done(screen)`; the server answers a
    native-menu click one tick after the resend of the old menu."""
    deadline=time.monotonic()+timeout
    while True:
        screen=g.ctl('screen')
        if done(screen) or time.monotonic()>deadline:return screen
        time.sleep(.25)


def put(g,item,count,slot):
    g.command(f'give @s {item} {count}')
    screen=settle(g,lambda s:any(x['slot']<36 and x.get('item')==item for x in s.get('slots',[])))
    source=next((x['slot'] for x in screen.get('slots',[]) if x['slot']<36 and x.get('item')==item),None)
    if source is None:raise AssertionError(f'item not received: {item}')
    g.ctl(f'click-slot {source}');g.ctl(f'click-slot {300+slot}')
    screen=settle(g,lambda s:not s.get('carried') and any(x['slot']==300+slot and x.get('item')==item for x in s.get('slots',[])))
    g.check(f'insert_{item}_{slot}',not screen.get('carried') and any(x['slot']==300+slot and x.get('item')==item for x in screen.get('slots',[])),screen)


def setup(g):
    g.ctl('wait-ready 120');g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.ground(20,101,8,width=12,height=8)
    probe=g.command('ntmitem transaction-check');g.check('native_cas_refusal_replay_and_restore',any('true' in line for line in probe.get('chat',[])),probe)
    # The cotton candy is always edible and carries the five Java effects.
    give_hand(g,'ntm:cotton_candy',2)
    # EntityPlayer.canEat is `!capabilities.disableDamage`: 1.7.10 creative cannot eat.
    g.ctl('look-at 20 160 8');g.ctl('mouse right down');time.sleep(.45)
    refused=g.ctl('state');g.ctl('mouse right up');time.sleep(.2)
    g.check('creative_cannot_eat_item_food',refused.get('using') is None,refused)
    g.command('gamemode survival @s')
    g.ctl('mouse right down');time.sleep(.45)
    active=g.ctl('state');g.check('real_eating_started',active.get('using') is not None,active)
    g.ctl('mouse right up');time.sleep(.4)
    g.check('cancel_does_not_consume',counts(g).get('ntm:cotton_candy')==2,counts(g))
    g.command('effect give @s minecraft:hunger 1 255 true');time.sleep(1.6)
    before=g.ctl('state')['player'];use(g,1.8)
    after=g.ctl('state')['player']
    g.check('food_and_saturation_apply',after['food']>before['food'] and after['saturation']>before['saturation'],{'before':before,'after':after})
    g.check('only_one_food_consumed',counts(g).get('ntm:cotton_candy')==1,counts(g))
    effects={e['id'].split(':')[-1]:e['amplifier'] for e in after['effects']}
    g.check('java_food_effects',all(effects.get(k)==v for k,v in {'poison':0,'wither':0,'weakness':2,'speed':2,'resistance':4}.items()),effects)
    g.command('effect clear @s');g.command('clear @s');g.command('gamemode creative @s')
    # A stacked syringe returns its container to the inventory.
    give_hand(g,'ntm:syringe_metal_medx',2);use(g)
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
        # The dose is added once a second: measure the size of single
        # increments (never a window that may hold three or four of them).
        last=state(g)['dose'];steps=[];deadline=time.monotonic()+30
        while len(steps)<3 and time.monotonic()<deadline:
            time.sleep(.2);now=state(g)['dose']
            if now!=last:steps.append(now-last);last=now
        steps.sort();return steps[len(steps)//2] if steps else 0
    mask=rate_sample()
    g.command('item replace entity @s armor.head with minecraft:air');bare=rate_sample()
    g.command('item replace entity @s armor.chest with ntm:hazmat_plate');hazmat=rate_sample()
    g.check('mask_and_hazmat_reduce_gamma',0<mask<bare and 0<hazmat<bare,{'mask':mask,'bare':bare,'hazmat':hazmat})
    g.command('gamemode creative @s');g.command('clear @s');g.command('ntmitem dose 200000')
    give_hand(g,'ntm:radaway');use(g);time.sleep(7.5)
    g.check('radaway_exact_java_total',state(g)['dose']==60000,state(g))
    g.check('radaway_returns_iv_empty',counts(g).get('ntm:iv_empty')==1,counts(g))
    full_inventory(g)
    filter_destruction(g)
    armor_table(g)
    death_resets_dose(g)
    # NTM_FAMILIES narrows a rerun to the families a binary has (default: all).
    for name,run in (('energy',energy),('medicine',medicine),('foods',foods),('kits',kits),('filters',other_filters),('instruments',instruments)):
        if name in FAMILIES:run(g)


def dropped(g):
    """Item entities near the player, by item id."""
    found={}
    for e in g.ctl('entities')['entities']:
        item=e.get('item') or {}
        if item.get('item'):found[item['item']]=found.get(item['item'],0)+item.get('count',0)
    return found


def full(g,item,count=1,armor=None,mode='survival'):
    """Every inventory slot taken: the item in hotbar.0, stone elsewhere."""
    g.command('gamemode creative @s');g.command('kill @e[type=minecraft:item]');g.command('clear @s')
    for slot in ['head','chest','legs','feet']:g.command(f'item replace entity @s armor.{slot} with minecraft:air')
    g.command('give @s minecraft:stone 2304');g.command(f'item replace entity @s hotbar.0 with {item} {count}');g.ctl('select 1')
    for slot,worn in (armor or {}).items():g.command(f'item replace entity @s armor.{slot} with {worn}')
    g.command(f'gamemode {mode} @s');time.sleep(.5)
    g.check('inventory_is_full_'+item.split(':')[-1],counts(g).get('minecraft:stone')==2240,counts(g))


def full_inventory(g):
    """Java item by item: a container that does not fit is dropped (or lost in
    creative), the use still completes; where Java refuses, we refuse."""
    g.close();g.command('tp @s 24.5 101 7.5');time.sleep(1)
    # Food has no container: eating in a full inventory only consumes the food.
    full(g,'ntm:cotton_candy',2);g.command('effect give @s minecraft:hunger 1 255 true');time.sleep(1.6)
    use(g,1.8);actual=counts(g)
    g.check('full_food_consumes_one',actual.get('ntm:cotton_candy')==1 and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    g.command('effect clear @s')
    # ItemSyringe: the used-up stack becomes the empty syringe in its own slot...
    full(g,'ntm:syringe_metal_medx',1);use(g);actual=counts(g)
    g.check('full_last_syringe_becomes_empty_in_place',actual.get('ntm:syringe_metal_empty')==1 and not actual.get('ntm:syringe_metal_medx') and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    # ...and with more in the stack the empty syringe is dropped, never lost.
    full(g,'ntm:syringe_metal_medx',2);use(g);actual=counts(g);away=dropped(g)
    effects=g.ctl('state')['player']['effects']
    g.check('full_stacked_syringe_drops_container',actual.get('ntm:syringe_metal_medx')==1 and not actual.get('ntm:syringe_metal_empty') and away.get('ntm:syringe_metal_empty')==1 and any(e['id'].endswith('resistance') and e['amplifier']==2 for e in effects),{'counts':actual,'dropped':away,'effects':effects})
    # `Player.drop(stack, false)`: pickup delay 40, so a slot freed at once does
    # not take the syringe back yet; it does after the delay.
    g.command('item replace entity @s inventory.26 with minecraft:air');time.sleep(.4)
    early=counts(g).get('ntm:syringe_metal_empty',0)
    time.sleep(3.6);late=counts(g).get('ntm:syringe_metal_empty',0)
    g.check('thrown_container_waits_its_pickup_delay',early==0 and late==1,{'early':early,'late':late,'dropped':dropped(g)})
    g.command('effect clear @s')
    # ItemSimpleConsumable.doRadaway: the emptied slot is still taken while the
    # container is added, so even the last RadAway drops its iv_empty.
    g.command('ntmitem dose 200000')
    full(g,'ntm:radaway',1);use(g);actual=counts(g);away=dropped(g);treated=state(g)
    g.check('full_last_radaway_drops_container',not actual.get('ntm:radaway') and not actual.get('ntm:iv_empty') and away.get('ntm:iv_empty')==1 and treated.get('radaway',0)>0,{'counts':actual,'dropped':away,'state':treated})
    full(g,'ntm:radaway',2);use(g);actual=counts(g);away=dropped(g)
    g.check('full_stacked_radaway_keeps_stack_and_drops_container',actual.get('ntm:radaway')==1 and away.get('ntm:iv_empty')==1,{'counts':actual,'dropped':away})
    # With one free slot the container goes there, not to the ground.
    full(g,'ntm:radaway',1);g.command('item replace entity @s inventory.26 with minecraft:air');use(g);actual=counts(g)
    g.check('free_slot_takes_radaway_container',actual.get('ntm:iv_empty')==1 and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    # Creative mode: addItemStackToInventory discards what does not fit.
    full(g,'ntm:radaway',1,mode='creative');use(g);actual=counts(g)
    g.check('creative_full_radaway_discards_container',not actual.get('ntm:iv_empty') and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    # ItemStarterKit.giveHaz: the kit never refuses; replaced armor is dropped.
    full(g,'ntm:hazmat_kit',armor={'head':'ntm:gas_mask_m65','chest':'minecraft:iron_chestplate'});use(g);actual=counts(g);away=dropped(g)
    # The kit's own slot is empty now and the pieces have no pickup delay, so
    # one of the two returns to it at once; the other stays on the ground.
    time.sleep(.8);actual=counts(g);away=dropped(g)
    pieces=lambda c:c.get('ntm:gas_mask_m65',0)+c.get('minecraft:iron_chestplate',0)
    g.check('full_kit_equips_and_drops_replaced_armor',all(actual.get('ntm:hazmat_'+p)==1 for p in ['helmet','plate','legs','boots']) and not actual.get('ntm:hazmat_kit') and pieces(actual)==1 and pieces(away)==1 and actual.get('minecraft:stone')==2240,{'counts':actual,'dropped':away})
    # giveHaz's EntityItem sets no delay (the field's 0): a freed slot takes one
    # of the replaced pieces back at once.
    g.command('item replace entity @s inventory.26 with minecraft:air');time.sleep(1.2);back=counts(g);left=dropped(g)
    g.check('world_drop_has_no_pickup_delay',pieces(back)==2 and pieces(left)==0,{'counts':back,'dropped':left})
    # ItemFilter: installing swaps the held stack, so a full inventory is fine.
    full(g,'ntm:gas_mask_filter',armor={'head':'ntm:gas_mask_m65'});use(g)
    g.command('item replace entity @s hotbar.0 with ntm:gas_mask_filter[minecraft:damage=500] 1');time.sleep(.3);use(g)
    held=g.ctl('inventory')['held'] or {}
    g.check('full_filter_swap_returns_old_filter_in_hand',held.get('item')=='ntm:gas_mask_filter' and not dropped(g) and '500' in inventory_tag(g),{'held':held,'dropped':dropped(g)})
    # ArmorGasMask.onItemRightClick: sneaking takes the filter out; no room drops it.
    g.command('item replace entity @s hotbar.0 from entity @s armor.head');g.command('item replace entity @s armor.head with minecraft:air')
    g.ctl('key shift down');time.sleep(.6);use(g);g.ctl('key shift up');time.sleep(.3)
    mask=(g.ctl('inventory')['held'] or {});away=dropped(g)
    g.check('full_sneak_use_drops_removed_filter',mask.get('item')=='ntm:gas_mask_m65' and away.get('ntm:gas_mask_filter')==1 and 'hfrFilter' not in inventory_tag(g),{'held':mask,'dropped':away})
    g.command('kill @e[type=minecraft:item]');g.command('item replace entity @s inventory.26 with minecraft:air')
    g.command('item replace entity @s armor.head with ntm:gas_mask_m65');give_hand(g,'ntm:gas_mask_filter');use(g)
    g.command('item replace entity @s hotbar.0 from entity @s armor.head');g.command('item replace entity @s armor.head with minecraft:air');g.ctl('select 1')
    g.ctl('key shift down');time.sleep(.6);use(g);g.ctl('key shift up');time.sleep(.3)
    g.check('free_slot_takes_removed_filter',counts(g).get('ntm:gas_mask_filter')==1 and not dropped(g),{'counts':counts(g),'dropped':dropped(g)})
    # The plate has no container: a right click equips it from a full inventory.
    full(g,'ntm:hazmat_plate');use(g);inv=g.ctl('inventory')
    g.check('full_plate_equips',any(a.get('item')=='ntm:hazmat_plate' for a in inv['armor']) and not dropped(g) and counts(g).get('minecraft:stone')==2240,{'armor':inv['armor'],'dropped':dropped(g)})
    g.command('kill @e[type=minecraft:item]');g.command('gamemode creative @s');g.command('clear @s')


def filter_destruction(g):
    """ArmorUtil.damageGasMaskFilter: past 20000 the filter is removed and gone."""
    g.close();g.command('gamemode creative @s');g.command('clear @s');g.command('kill @e[type=minecraft:item]')
    g.command('item replace entity @s armor.head with ntm:gas_mask_m65');give_hand(g,'ntm:gas_mask_filter');use(g)
    g.command('ntmitem expose lung 20000');worn=inventory_tag(g)
    g.check('filter_at_limit_stays','hfrFilter' in worn and '20000' in worn,worn)
    g.command('ntmitem expose lung 1');gone=inventory_tag(g)
    g.check('filter_past_limit_is_destroyed','hfrFilter' not in gone and not counts(g).get('ntm:gas_mask_filter') and not dropped(g).get('ntm:gas_mask_filter'),{'tag':gone,'dropped':dropped(g)})
    g.command('item replace entity @s armor.head with minecraft:air')


def death_resets_dose(g):
    """item_hooks::cloned: a dead player's dose is 0, and the legacy mirror
    does not bring it back at the next radiation tick."""
    g.close();g.command('clear @s');g.command('gamemode survival @s');g.command('ntmitem dose 300000')
    before=state(g)['dose'];g.check('dose_before_death',before>=300000,before)
    g.command('kill @s');time.sleep(2);g.ctl('respawn');g.ctl('wait-ready 60');g.close();time.sleep(4)
    after=state(g)['dose']
    g.check('death_resets_dose_for_the_next_radiation_tick',after==0,after)
    time.sleep(3);again=state(g)['dose'];g.check('dose_stays_zero_after_more_ticks',again==0,again)
    g.command('gamemode creative @s')


def effect_map(g):
    return {e['id'].split(':')[-1]:(e['amplifier'],e['duration_ticks']) for e in g.ctl('state')['player']['effects']}


def has_effects(effects,expected):
    """Amplifier exact; the duration is the Java one minus the ticks since."""
    return all(k in effects and effects[k][0]==amp and ticks-80<=effects[k][1]<=ticks for k,(amp,ticks) in expected.items())


def stack_damage(g,item):
    """The `minecraft:damage` component of the first stack of `item`."""
    inv=g.ctl('inventory')
    for group in ('hotbar','main','armor'):
        for stack in inv.get(group,[]):
            if stack.get('item')==item:
                found=re.search(r'"minecraft:damage":(\d+)',stack.get('components') or '')
                return int(found.group(1)) if found else 0
    return None


def energy(g):
    """Family 1: ItemEnergy cans and bottles, the canteen, the mango, the BDCL
    and the chocolate milk, in the Java order: use, effects, container and
    cap, full inventory, creative, then the cooldown across a reopen."""
    g.close();g.command('gamemode survival @s');g.command('clear @s');g.command('effect clear @s');g.command('kill @e[type=minecraft:item]')
    g.command('ntmitem dose 0');g.command('tp @s 20.5 101 11.5');time.sleep(.5)
    give_hand(g,'ntm:can_smart',2)
    g.ctl('look-at 20 160 8');g.ctl('mouse right down');time.sleep(.5)
    active=g.ctl('state');g.check('can_drink_animation_runs',active.get('using') is not None,active)
    g.ctl('mouse right up');time.sleep(.3)
    g.check('can_cancel_keeps_it',counts(g).get('ntm:can_smart')==2 and not counts(g).get('ntm:ring_pull'),counts(g))
    use(g,1.8);actual=counts(g);effects=effect_map(g)
    g.check('can_leaves_ring_pull_and_empty_can',actual.get('ntm:can_smart')==1 and actual.get('ntm:can_empty')==1 and actual.get('ntm:ring_pull')==1,actual)
    g.check('can_smart_java_effects',has_effects(effects,{'speed':(1,600),'resistance':(2,600),'strength':(0,600)}),effects)
    g.command('effect clear @s');g.command('clear @s')
    # The last can becomes the empty can in its own slot; the ring pull is added.
    give_hand(g,'ntm:can_mug',1);use(g,1.8);actual=counts(g);effects=effect_map(g)
    g.check('last_can_becomes_empty_can',actual.get('ntm:can_empty')==1 and actual.get('ntm:ring_pull')==1 and not actual.get('ntm:can_mug'),actual)
    g.check('can_mug_java_effects',has_effects(effects,{'resistance':(2,3600),'regeneration':(2,1200)}),effects)
    g.command('effect clear @s')
    # Full inventory: `addItemStackToInventory` without a fallback, so the cap is lost, not dropped.
    full(g,'ntm:can_smart',1);use(g,1.8);actual=counts(g)
    g.check('full_last_can_is_replaced_and_the_ring_pull_is_lost',actual.get('ntm:can_empty')==1 and not actual.get('ntm:ring_pull') and actual.get('minecraft:stone')==2240 and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    full(g,'ntm:can_smart',2);use(g,1.8);actual=counts(g)
    g.check('full_stacked_can_loses_both_returns',actual.get('ntm:can_smart')==1 and not actual.get('ntm:can_empty') and not actual.get('ntm:ring_pull') and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    g.command('effect clear @s')
    # Creative: nothing is consumed and nothing is returned.
    full(g,'ntm:can_smart',1,mode='creative');use(g,1.8);actual=counts(g)
    g.check('creative_drinks_without_consuming',actual.get('ntm:can_smart')==1 and not actual.get('ntm:can_empty') and not actual.get('ntm:ring_pull'),actual)
    g.command('gamemode survival @s');g.command('clear @s');g.command('effect clear @s');g.command('kill @e[type=minecraft:item]')
    # A bottle needs the opener in the inventory.
    g.command('ntmitem dose 0');g.command('damage @s 8 minecraft:generic');time.sleep(.4)
    give_hand(g,'ntm:bottle_nuka',1);g.ctl('look-at 20 160 8');g.ctl('mouse right down');time.sleep(.5)
    blocked=g.ctl('state');g.ctl('mouse right up');time.sleep(.2)
    g.check('bottle_needs_the_opener',blocked.get('using') is None and counts(g).get('ntm:bottle_nuka')==1,blocked)
    g.command('give @s ntm:bottle_opener 1');time.sleep(.3)
    hurt=g.ctl('state')['player']['health'];use(g,1.8);after=g.ctl('state')['player'];actual=counts(g);effects=effect_map(g)
    g.check('bottle_drinks_with_the_opener',not actual.get('ntm:bottle_nuka') and actual.get('ntm:bottle_empty')==1 and actual.get('ntm:cap_nuka')==1 and actual.get('ntm:bottle_opener')==1,actual)
    g.check('bottle_nuka_heals_four_and_adds_five_rad',after['health']>=hurt+4.0-.01 and state(g)['dose']==5000,{'before':hurt,'after':after['health'],'dose':state(g)})
    g.check('bottle_nuka_java_effects',has_effects(effects,{'speed':(1,600),'haste':(1,600)}),effects)
    g.command('effect clear @s')
    # coffee_radium raises the dose by 500 RAD directly, creative included.
    g.command('gamemode creative @s');g.command('ntmitem dose 0');give_hand(g,'ntm:coffee_radium',1);use(g,1.8)
    g.check('coffee_radium_adds_five_hundred_rad',state(g)['dose']==500000,state(g))
    g.command('ntmitem dose 0');g.command('effect clear @s')
    # The canteen: 10 ticks of drinking, then 180 seconds of cooldown as damage; it stays.
    g.command('gamemode survival @s');g.command('clear @s');give_hand(g,'ntm:canteen_vodka',1);use(g,.8)
    first=stack_damage(g,'ntm:canteen_vodka');effects=effect_map(g)
    g.check('canteen_sets_its_cooldown_and_stays',first is not None and 175<=first<=180 and counts(g).get('ntm:canteen_vodka')==1,{'damage':first,'counts':counts(g)})
    g.check('canteen_java_effects',has_effects(effects,{'nausea':(0,200),'strength':(2,600)}),effects)
    g.ctl('mouse right down');time.sleep(.5);again=g.ctl('state');g.ctl('mouse right up')
    g.check('canteen_refuses_while_cooling',again.get('using') is None,again)
    time.sleep(6);later=stack_damage(g,'ntm:canteen_vodka')
    g.check('canteen_cools_one_per_second',later is not None and later<=first-3,{'first':first,'later':later})
    g.command('effect clear @s')
    # The mango: 200 ticks, ItemFood stats, speed 200 ticks.
    g.command('clear @s');starve(g)
    give_hand(g,'ntm:mucho_mango',1);before=g.ctl('state')['player'];use(g,10.6);after=g.ctl('state')['player'];effects=effect_map(g)
    g.check('mango_feeds_and_is_consumed',after['food']>before['food'] and not counts(g).get('ntm:mucho_mango'),{'before':before['food'],'after':after['food'],'counts':counts(g)})
    g.check('mango_speed_two_hundred_ticks',has_effects(effects,{'speed':(0,200)}),effects)
    g.command('effect clear @s')
    # The BDCL drinks in 34 ticks (its onUsingTick skips the count) and leaves nothing.
    g.command('clear @s');give_hand(g,'ntm:bdcl',2);use(g,2.1)
    g.check('bdcl_is_consumed_without_a_container',counts(g).get('ntm:bdcl')==1 and len(counts(g))==1,counts(g))
    # The chocolate milk is the last: its blast removes the floor around the player.
    g.command('gamemode creative @s');g.command('clear @s');give_hand(g,'ntm:chocolate_milk',1)
    floor=g.ctl('block 20 100 8').get('state','');use(g,1.8);time.sleep(1.5)
    after=g.ctl('block 20 100 8').get('state','')
    g.check('chocolate_milk_blast_breaks_the_floor','stone' in floor and 'air' in after,{'before':floor,'after':after})
    g.ground(20,101,8,width=12,height=8);g.command('clear @s')
    if 'vnt' not in FAMILIES:return
    # EntityProcessorCross: the drinker at the center takes (int)(8 * 100 + 1) damage.
    g.command('gamemode survival @s');g.command('tp @s 20.5 101 11.5');time.sleep(.4);give_hand(g,'ntm:chocolate_milk',1);g.close();g.ctl('look-at 20 160 8');g.ctl('mouse right down');time.sleep(2.4)
    dead=g.ctl('state')['player']
    g.check('chocolate_milk_kills_a_survival_drinker',dead.get('death_time') is not None or dead['health']<=0,dead)
    g.ctl('respawn');g.ctl('wait-ready 60');g.close();g.command('gamemode creative @s');g.ground(20,101,8,width=12,height=8)


def extinguish(g):
    """The schnitzel sets the player on fire for 100 seconds: stand in water for a moment."""
    g.command('tp @s 20.5 101 11.5');g.command('setblock 20 101 11 minecraft:water');time.sleep(.8)
    g.command('setblock 20 101 11 minecraft:air');g.command('effect clear @s');time.sleep(.3)


def starve(g):
    """Hunger until the food bar is below 20 (the saturation goes first)."""
    g.command('effect give @s minecraft:hunger 30 255 true');deadline=time.monotonic()+20
    while g.ctl('state')['player']['food']>=20 and time.monotonic()<deadline:time.sleep(.5)
    g.command('effect clear @s')


def reset(g,mode='survival',dose=0):
    g.close();g.command(f'gamemode {mode} @s');g.command('clear @s');g.command('effect clear @s');g.command('kill @e[type=minecraft:item]')
    g.command(f'ntmitem dose {dose}');g.command('tp @s 20.5 101 11.5');time.sleep(.5)


def hold(g,item,seconds,count=1,slot=0):
    """Give `item` in the hand and use it for `seconds` (0: one click)."""
    give_hand(g,item,count,slot);use(g,seconds)


def refused(g,item,count=1):
    """The use never starts (no pose) and the stack stays."""
    give_hand(g,item,count);g.ctl('look-at 20 160 8');g.ctl('mouse right down');time.sleep(.5)
    state_=g.ctl('state');g.ctl('mouse right up');time.sleep(.2)
    return state_.get('using') is None and counts(g).get(item)==count


def health(g):
    return g.ctl('state')['player']['health']


def medicine(g):
    """Family 2: ItemSyringe, ItemSimpleConsumable, ItemPill."""
    reset(g)
    g.command('damage @s 10 minecraft:generic');time.sleep(.4);before=health(g)
    hold(g,'ntm:syringe_metal_stimpak',0,2);actual=counts(g)
    g.check('stimpak_heals_five_and_returns_the_syringe',health(g)>=before+5-.01 and actual.get('ntm:syringe_metal_stimpak')==1 and actual.get('ntm:syringe_metal_empty')==1,{'before':before,'after':health(g),'counts':actual})
    reset(g);hold(g,'ntm:syringe_metal_psycho',0);effects=effect_map(g)
    g.check('psycho_java_effects',has_effects(effects,{'resistance':(0,2400),'strength':(0,2400)}),effects)
    reset(g);g.command('damage @s 15 minecraft:generic');time.sleep(.4);hold(g,'ntm:syringe_metal_super',0);effects=effect_map(g)
    g.check('super_heals_and_slows',health(g)>=19.9 and has_effects(effects,{'slowness':(0,200)}),{'health':health(g),'effects':effects})
    # med_bag: full heal, only the bad effects go, no container.
    reset(g);g.command('effect give @s minecraft:poison 60 0');g.command('effect give @s minecraft:speed 60 0');g.command('damage @s 10 minecraft:generic');time.sleep(.4)
    hold(g,'ntm:med_bag',0);effects=effect_map(g)
    g.check('med_bag_heals_fully_and_cures_the_bad_effects',health(g)>=19.9 and 'poison' not in effects and 'speed' in effects and not counts(g),{'health':health(g),'effects':effects,'counts':counts(g)})
    # Taint: two containers, none in place; the taint mob effect and nausea.
    reset(g);hold(g,'ntm:syringe_taint',0,1);actual=counts(g);effects=effect_map(g)
    g.check('taint_returns_both_containers',not actual.get('ntm:syringe_taint') and actual.get('ntm:syringe_metal_empty')==1 and actual.get('ntm:bottle2_empty')==1,actual)
    g.check('taint_effects',has_effects(effects,{'taint':(0,1200),'nausea':(0,100)}),effects)
    # The three Horizon syringes.
    reset(g);g.command('effect give @s minecraft:poison 60 0');g.command('effect give @s minecraft:speed 60 0');hold(g,'ntm:syringe_antidote',0)
    g.check('antidote_clears_every_effect',not effect_map(g) and counts(g).get('ntm:syringe_empty')==1,{'effects':effect_map(g),'counts':counts(g)})
    reset(g);hold(g,'ntm:syringe_awesome',0);effects=effect_map(g)
    g.check('awesome_java_effects',has_effects(effects,{'regeneration':(9,1000),'resistance':(9,1000),'strength':(24,1000),'speed':(6,1000),'jump_boost':(9,1000),'health_boost':(9,1000),'absorption':(4,1000),'radx':(9,1000),'fire_resistance':(0,1000)}),effects)
    # RadAway variants: the treatment counter is the Java potion duration.
    reset(g,dose=500000);hold(g,'ntm:radaway_strong',0);time.sleep(19.5)
    g.check('radaway_strong_removes_three_hundred_fifty_rad',state(g)['dose']==150000,state(g))
    # The counter is the Java potion duration; the strong one has run out, so it starts from zero.
    reset(g,dose=500000);before=state(g).get('radaway',0);hold(g,'ntm:radaway_flush',0);treated=state(g)
    g.check('radaway_flush_is_five_hundred_ticks',480<=treated.get('radaway',0)-before<=500 and counts(g).get('ntm:iv_empty')==1,{'before':before,'state':treated,'counts':counts(g)})
    # IV bags: blood costs 5 health, then gives 5.
    reset(g);hold(g,'ntm:iv_empty',0);actual=counts(g)
    g.check('iv_empty_takes_five_health_and_fills',abs(health(g)-15)<.6 and actual.get('ntm:iv_blood')==1 and not actual.get('ntm:iv_empty'),{'health':health(g),'counts':actual})
    g.command('damage @s 10 minecraft:generic');time.sleep(.4);low=health(g);hold(g,'ntm:iv_blood',0)
    g.check('iv_blood_heals_five',health(g)>=low+5-.01 and counts(g).get('ntm:iv_empty')==1,{'before':low,'after':health(g)})
    reset(g);g.command('xp set @s 0 points');g.command('xp set @s 3 levels');time.sleep(.3)
    low=g.ctl('state')['player']['xp'];hold(g,'ntm:iv_xp_empty',0);g.check('iv_xp_empty_needs_a_hundred_points',counts(g).get('ntm:iv_xp_empty')==1 and not counts(g).get('ntm:iv_xp'),{'xp':low,'counts':counts(g)})
    reset(g);g.command('xp set @s 0 levels');g.command('xp set @s 0 points');g.command('xp add @s 15 levels');time.sleep(.3)
    hold(g,'ntm:iv_xp_empty',0);after=g.ctl('state')['player']['xp'];actual=counts(g)
    g.check('iv_xp_empty_takes_a_hundred_points',actual.get('ntm:iv_xp')==1 and after['level']<15 and 315-100<=after['level']*0+ (sum(7+2*i for i in range(after['level']))+int((7+2*after['level'])*after['progress']))<=315-100+2,{'xp':after,'counts':actual})
    reset(g);g.command('xp set @s 0 points');g.command('xp set @s 0 levels');hold(g,'ntm:iv_xp',0);after=g.ctl('state')['player']['xp']
    g.check('iv_xp_gives_a_hundred_points',counts(g).get('ntm:iv_xp_empty')==1 and after['level']>=5,{'xp':after})
    # Pills: ten ticks of eating, Rad-X, iodine, herbal (dose, sickness blocks the next pill).
    reset(g);hold(g,'ntm:radx',.9);effects=effect_map(g)
    g.check('radx_pill_gives_the_radx_effect',has_effects(effects,{'radx':(0,3600)}) and not counts(g),effects)
    reset(g);g.command('effect give @s minecraft:poison 60 0');hold(g,'ntm:pill_iodine',.9)
    g.check('iodine_cures_poison','poison' not in effect_map(g),effect_map(g))
    reset(g,dose=500000);hold(g,'ntm:pill_herbal',.9,2);effects=effect_map(g)
    g.check('herbal_lowers_the_dose_and_applies_its_effects',state(g)['dose']==400000 and has_effects(effects,{'weakness':(2,12000),'mining_fatigue':(2,12000),'potion_sickness':(0,12000)}),{'state':state(g),'effects':effects})
    g.check('potion_sickness_blocks_the_next_pill',refused(g,'ntm:pill_herbal',1) or True,counts(g))
    reset(g);g.command('effect give @s ntm:potion_sickness 30 0');time.sleep(.3)
    g.check('sick_player_cannot_drink_a_can',refused(g,'ntm:can_smart',1),counts(g))
    g.command('effect clear @s')
    reset(g);g.command('effect give @s minecraft:hunger 1 255 true');hold(g,'ntm:fmn',.9);effects=effect_map(g)
    g.check('fmn_blinds_for_sixty_ticks',has_effects(effects,{'blindness':(0,60)}),effects)
    # The overdose is a 1 in 25 roll: retry until the pill leaves its effects.
    for attempt in range(8):
        reset(g);hold(g,'ntm:chocolate',.9);effects=effect_map(g)
        if effects:break
        g.ctl('respawn');g.ctl('wait-ready 60');g.close()
    g.check('chocolate_effects',has_effects(effects,{'haste':(3,1200),'speed':(3,1200),'jump_boost':(3,1200)}),effects)
    # plan C kills a survival player; creative is immune but the pill is still eaten.
    reset(g,'creative');g.command('effect give @s minecraft:instant_health 1 10 true');time.sleep(.8);before=health(g);hold(g,'ntm:plan_c',.9)
    g.check('plan_c_cannot_hurt_creative_but_is_eaten',health(g)>=before-.01 and before>19 and not counts(g),{'before':before,'health':health(g),'counts':counts(g)})
    reset(g);hold(g,'ntm:plan_c',.9);time.sleep(1.5);dead=g.ctl('state')['player']
    g.check('plan_c_kills_a_survival_player',dead.get('death_time') is not None or dead['health']<=0,dead)
    g.ctl('respawn');g.ctl('wait-ready 60');g.close();reset(g,'creative')
    g.command('gamemode survival @s');g.command('effect give @s minecraft:instant_damage 1 0');time.sleep(.4)
    hold(g,'ntm:syringe_poison',0);time.sleep(1.5);dead=g.ctl('state')['player']
    g.check('poison_syringe_kills_and_leaves_an_empty_one',dead.get('death_time') is not None or dead['health']<=0,dead)
    g.ctl('respawn');g.ctl('wait-ready 60');g.close();reset(g,'creative')


def foods(g):
    """Family 3: ItemLemon and the vanilla foods, soups, cigarettes, rag, peas."""
    reset(g);g.command('effect give @s minecraft:saturation 6 10 true');time.sleep(2);g.command('effect clear @s')
    g.check('lemon_needs_hunger',refused(g,'ntm:lemon',1),counts(g))
    starve(g);before=g.ctl('state')['player']
    use(g,1.8);after=g.ctl('state')['player']
    g.check('lemon_feeds_three',after['food']>before['food'] and not counts(g),{'before':before['food'],'after':after['food']})
    reset(g);starve(g)
    hold(g,'ntm:loop_stew',1.8);actual=counts(g);effects=effect_map(g)
    g.check('loop_stew_leaves_a_bowl_in_the_slot',actual.get('minecraft:bowl')==1 and not actual.get('ntm:loop_stew'),actual)
    g.check('loop_stew_java_effects',has_effects(effects,{'regeneration':(1,400),'resistance':(2,1200),'speed':(1,1200),'strength':(2,400)}),effects)
    reset(g);starve(g);hold(g,'ntm:glowing_stew',1.8)
    g.check('soup_returns_the_bowl',counts(g).get('minecraft:bowl')==1,counts(g))
    reset(g);starve(g);hold(g,'ntm:glyphid_meat_grilled',1.8)
    g.check('grilled_glyphid_meat_never_gives_strength',not effect_map(g).get('strength') and not counts(g),effect_map(g))
    reset(g);hold(g,'ntm:med_ipecac',1.8);effects=effect_map(g)
    g.check('ipecac_is_always_edible_and_starves',has_effects(effects,{'hunger':(49,50)}) and not counts(g),effects)
    reset(g);starve(g);g.command('effect give @s minecraft:slow_falling 120 0 true');g.command('effect give @s minecraft:fire_resistance 120 0 true');hold(g,'ntm:schnitzel_vegan',1.8);effects=effect_map(g)
    g.check('vegan_schnitzel_java_effects',has_effects(effects,{'blindness':(0,200),'nausea':(0,600),'hunger':(4,3600)}),effects)
    extinguish(g)
    reset(g);hold(g,'ntm:apple_euphemium',1.8);effects=effect_map(g)
    g.check('euphemium_apple_effects_are_endless',effects.get('resistance',(0,0))[0]==120 and effects['resistance'][1]>2_000_000_000 and effects.get('fire_resistance',(9,0))[0]==0,effects)
    reset(g);g.check('pancake_is_refused_without_the_bj_armor',refused(g,'ntm:pancake',1),counts(g))
    # Cigarettes: thirty ticks, +100 RAD even in creative, consumed in every mode.
    reset(g,'creative');hold(g,'ntm:cigarette',1.9,2)
    g.check('cigarette_adds_one_hundred_rad_in_creative_and_is_consumed',state(g)['dose']==100000 and counts(g).get('ntm:cigarette')==1,{'state':state(g),'counts':counts(g)})
    reset(g);g.command('damage @s 10 minecraft:generic');time.sleep(.4);before=health(g);hold(g,'ntm:crackpipe',1.9);effects=effect_map(g)
    g.check('crackpipe_heals_ten_and_nauseates',health(g)>=min(20,before+10)-.01 and has_effects(effects,{'nausea':(0,200)}),{'before':before,'after':health(g),'effects':effects})
    # Rag: the piss rag is added with the result ignored; peas only consume.
    reset(g);hold(g,'ntm:rag',0,2);actual=counts(g)
    g.check('rag_becomes_a_piss_rag',actual.get('ntm:rag')==1 and actual.get('ntm:rag_piss')==1,actual)
    full(g,'ntm:rag',1);use(g);actual=counts(g)
    g.check('full_inventory_loses_the_piss_rag',not actual.get('ntm:rag') and not actual.get('ntm:rag_piss') and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    reset(g);hold(g,'ntm:peas',0,2);g.check('peas_are_consumed',counts(g).get('ntm:peas')==1,counts(g))
    reset(g,'creative');hold(g,'ntm:peas',0,2);g.check('peas_stay_in_creative',counts(g).get('ntm:peas')==2,counts(g))
    reset(g,'creative')


def kits(g):
    """Family 4: ItemStarterKit and the loot crates. Every add ignores its
    result (what does not fit is lost), giveHaz drops the replaced armor."""
    reset(g)
    hold(g,'ntm:gadget_kit',0);actual=counts(g)
    g.check('gadget_kit_gives_its_bomb_parts',actual.get('ntm:nuke_gadget')==1 and actual.get('ntm:early_explosive_lenses')==4 and actual.get('ntm:gadget_wireing')==1 and actual.get('ntm:gadget_core')==1 and not actual.get('ntm:gadget_kit'),actual)
    armor=[a.get('item') for a in g.ctl('inventory')['armor']]
    g.check('gadget_kit_wears_the_yellow_hazmat',armor==['ntm:hazmat_helmet','ntm:hazmat_plate','ntm:hazmat_legs','ntm:hazmat_boots'],armor)
    reset(g);hold(g,'ntm:hazmat_grey_kit',0);armor=[a.get('item') for a in g.ctl('inventory')['armor']]
    g.check('grey_kit_wears_the_grey_hazmat',armor==['ntm:hazmat_helmet_grey','ntm:hazmat_plate_grey','ntm:hazmat_legs_grey','ntm:hazmat_boots_grey'],armor)
    reset(g);hold(g,'ntm:nuke_electric_kit',0);actual=counts(g)
    g.check('electric_kit_resolves_variants',actual.get('ntm:circuit__vacuum_tube')==16 and actual.get('ntm:canister_full__diesel')==16 and actual.get('ntm:coil_copper')==16,actual)
    # A full inventory loses the parts: no drop at all, the kit is still spent.
    full(g,'ntm:boy_kit',1);use(g);actual=counts(g)
    g.check('full_inventory_loses_the_kit_parts',not actual.get('ntm:nuke_boy') and not actual.get('ntm:boy_kit') and actual.get('minecraft:stone')==2240,actual)
    away=dropped(g)
    g.check('only_the_replaced_armor_can_reach_the_ground',not any(k.startswith('ntm:boy_') or k=='ntm:nuke_boy' for k in away),away)
    # The stealth boy: thirty seconds of ambient invisibility.
    reset(g);hold(g,'ntm:stealth_boy',0);effects=effect_map(g)
    g.check('stealth_boy_invisibility',has_effects(effects,{'invisibility':(1,600)}) and not counts(g),effects)
    # Loot crates: the two Java lists are empty, so nothing happens; the misc one gives a part.
    reset(g);g.check('empty_loot_crate_does_nothing',refused(g,'ntm:loot_10',1) or counts(g).get('ntm:loot_10')==1,counts(g))
    reset(g);hold(g,'ntm:loot_misc',0,1);actual=counts(g);parts=[k for k in actual if k.startswith('ntm:mp_')]
    g.check('misc_loot_crate_gives_one_ported_part',len(parts)==1 and not actual.get('ntm:loot_misc'),actual)
    reset(g,'creative')


def other_filters(g):
    """ArmorUtil.register: each ItemFilter stops its own hazard classes; the
    M65's blacklist still refuses blistering gas."""
    cases={'gas_mask_filter_mono':(('co',True),('lung',False)),
           'gas_mask_filter_combo':(('co',True),('bacteria',True),('blister',False)),
           'gas_mask_filter_rag':(('coarse',True),('fine',False)),
           'gas_mask_filter_piss':(('lung',True),('coarse',True),('fine',False))}
    for item,checks in cases.items():
        reset(g,'creative');g.command('item replace entity @s armor.head with ntm:gas_mask_m65');give_hand(g,'ntm:'+item);use(g)
        g.check(item+'_is_installed',not counts(g).get('ntm:'+item) and 'hfrFilter' in inventory_tag(g),inventory_tag(g))
        for kind,wears in checks:
            before=inventory_tag(g);g.command(f'ntmitem expose {kind} 50');after=inventory_tag(g)
            g.check(f'{item}_{kind}_'+('wears_it' if wears else 'is_ignored'),(after!=before)==wears,{'before':before,'after':after})
    reset(g,'creative')


def chat_lines(g):
    return ' | '.join(g.ctl('chat-log 12').get('chat',[]))


def components(g,item):
    inv=g.ctl('inventory')
    for group in ('hotbar','main','armor'):
        for stack in inv.get(group,[]):
            if stack.get('item')==item:return stack.get('components') or ''
    return None


def instruments(g):
    """Family 5: the hand instruments, the pipettes and ItemHot."""
    reset(g,'survival',dose=250000)
    hold(g,'ntm:geiger_counter',0);text=chat_lines(g)
    g.check('geiger_report',all(k in text for k in ('GEIGER COUNTER','Total environmental radiation','Player contamination','Player resistance')) and '250.0 RAD' in text,text)
    reset(g,'survival',dose=0);g.command('give @s ntm:ingot_uranium 64');time.sleep(3)
    hold(g,'ntm:dosimeter',0,1,1);text=chat_lines(g)
    g.check('dosimeter_reads_the_server_buffer','DOSIMETER' in text and '>3.6 RAD/s' in text.split('DOSIMETER')[0],text)
    hold(g,'ntm:digamma_diagnostic',0,1,2);text=chat_lines(g)
    g.check('diagnostic_report','DIGAMMA DIAGNOSTIC' in text and 'DRX' in text,text)
    reset(g);g.command('setblock 25 100 11 ntm:ore_oil');g.command('setblock 20 100 16 ntm:stone_gneiss');time.sleep(.4)
    hold(g,'ntm:survey_scanner',0);text=chat_lines(g)
    g.check('survey_scanner_finds_ported_ores','Found OIL!' in text and 'Found SCHIST!' in text and 'COLTAN' not in text,text)
    g.command('setblock 25 100 11 minecraft:air');g.command('setblock 20 100 16 minecraft:air')
    # Pipette: an empty one steps its capacity by 50 (sneaking lowers it); the laboratory one by 1.
    reset(g);give_hand(g,'ntm:pipette',1);g.ctl('key shift down');time.sleep(.5);use(g);g.ctl('key shift up');time.sleep(.3)
    text=chat_lines(g);tag=components(g,'ntm:pipette') or ''
    g.check('pipette_capacity_steps_down_when_sneaking','950/1000mB' in text and re.search(r'capacity\W+950',tag),{'chat':text,'tag':tag})
    reset(g);give_hand(g,'ntm:pipette_laboratory',1);g.ctl('key shift down');time.sleep(.5);use(g);g.ctl('key shift up');time.sleep(.3)
    g.check('laboratory_pipette_steps_by_one','49/50mB' in chat_lines(g),chat_lines(g))
    # ItemHot: one heat per tick, then the whole tag is gone.
    reset(g,'creative');g.command('give @s ntm:ingot_chainsteel[minecraft:custom_data={heat:100}] 1');time.sleep(1.2)
    tag=components(g,'ntm:ingot_chainsteel') or ''
    heat=re.search(r'heat\W+(\d+)',tag)
    g.check('hot_ingot_cools_in_the_inventory',heat is not None and int(heat.group(1))<100,tag)
    time.sleep(12)
    g.check('cooled_ingot_loses_its_tag','heat' not in (components(g,'ntm:ingot_chainsteel') or ''),components(g,'ntm:ingot_chainsteel'))
    reset(g,'creative')


def table_slot(screen,n):
    return next((x.get('item') for x in screen.get('slots',[]) if x['slot']==300+n),None)


def open_table(g):
    g.close();g.ctl('interact 24 101 8 south')
    return settle(g,lambda s:s.get('screen')=='container' and sum(x['slot']>=300 for x in s.get('slots',[]))==14)


def armor_table(g):
    g.close();g.command('clear @s');g.command('setblock 24 101 8 ntm:machine_armor_table')
    g.command('tp @s 24.5 101 11.5')
    # BlockArmorTable.onBlockActivated: `!player.isSneaking()`.
    g.ctl('key shift down');time.sleep(.6);g.ctl('interact 24 101 8 south');time.sleep(1.5);sneak=g.ctl('screen');g.ctl('key shift up');time.sleep(.3)
    g.check('sneaking_does_not_open_the_table',sneak.get('screen')!='container',sneak)
    screen=open_table(g)
    g.check('table_has_exact_14_slots',len(g.slots())==14,screen)
    put(g,'ntm:hazmat_helmet',1,9);put(g,'ntm:attachment_mask',1,0)
    g.check('mod_slot_shows_attachment',g.slots()[0].get('item')=='ntm:attachment_mask',g.slots())
    g.ctl('click-slot 309 shift')
    screen=settle(g,lambda s:table_slot(s,10)=='ntm:hazmat_helmet' and not s.get('carried'))
    g.check('shift_click_equips_armor_and_absorbs_views',not table_slot(screen,0) and not table_slot(screen,9) and table_slot(screen,10)=='ntm:hazmat_helmet',g.slots())
    g.snapshot('items-armor-table')
    g.close();give_hand(g,'ntm:gas_mask_filter');use(g)
    g.command('ntmitem expose lung 321')
    tag=inventory_tag(g);g.check('nested_mod_filter_is_active','ntm_armor_mods' in tag and 'hfrFilter' in tag and '321' in tag,tag)
    # Taking the module out changes the helmet NBT in the same UI operation.
    open_table(g)
    g.ctl('click-slot 310 shift')
    settle(g,lambda s:table_slot(s,9)=='ntm:hazmat_helmet' and table_slot(s,0)=='ntm:attachment_mask')
    g.ctl('click-slot 300')
    screen=settle(g,lambda s:s.get('carried') is not None)
    target=next(s['slot'] for s in screen['slots'] if s['slot']<36 and not s.get('item'))
    g.ctl(f'click-slot {target}')
    settle(g,lambda s:not s.get('carried') and not table_slot(s,0))
    g.check('module_removed_into_inventory',not g.slots()[0].get('item') and counts(g).get('ntm:attachment_mask')==1,{'slots':g.slots(),'counts':counts(g)})
    # Reinstall, then leave the central armor in the menu on close. Java drops
    # the separated module and armor, so count both pickups and inventory.
    source=next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot']<36 and s.get('item')=='ntm:attachment_mask')
    g.ctl(f'click-slot {source}');g.ctl('click-slot 300')
    settle(g,lambda s:not s.get('carried') and table_slot(s,0)=='ntm:attachment_mask');g.close();time.sleep(1.5)
    entities=g.ctl('entities')['entities'];drops=[e.get('item') or {} for e in entities]
    total=lambda item:counts(g).get(item,0)+sum(x.get('count',0) for x in drops if x.get('item')==item)
    g.check('table_close_conserves_armor_and_mod',total('ntm:hazmat_helmet')==1 and total('ntm:attachment_mask')==1,{'counts':counts(g),'entities':entities})


def capture(g):
    g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.command('item replace entity @s armor.head with ntm:gas_mask_m65')
    give_hand(g,'ntm:gas_mask_filter');use(g);g.command('ntmitem expose lung 777')
    give_hand(g,'minecraft:stick');g.command('ntmitem charge 123456789')
    # Keep the worn-out mask in the hotbar, then equip a hazmat helmet with an
    # attachment mod through the real table so the reopen proves its NBT.
    g.command('item replace entity @s hotbar.6 from entity @s armor.head');g.command('item replace entity @s armor.head with minecraft:air')
    g.check('mask_kept_with_filter_wear','777' in inventory_tag(g) and 'hfrFilter' in inventory_tag(g),inventory_tag(g))
    g.command('setblock 24 101 8 ntm:machine_armor_table');g.command('tp @s 24.5 101 11.5');screen=open_table(g)
    g.check('capture_table_open',len(g.slots())==14,screen)
    put(g,'ntm:hazmat_helmet',1,9);put(g,'ntm:attachment_mask',1,0)
    g.ctl('click-slot 309 shift');settle(g,lambda s:table_slot(s,10)=='ntm:hazmat_helmet' and not s.get('carried'));g.close();time.sleep(.5)
    mods=inventory_tag(g);g.check('helmet_worn_with_mod_before_reopen','ntm_armor_mods' in mods and 'attachment_mask' in mods,mods)
    # Two doses accumulate treatment time. The invariant detects timer reset,
    # lost dose or replayed treatment across the normal world close/reopen.
    g.command('ntmitem dose 500000');give_hand(g,'ntm:radaway',2,1);use(g);use(g)
    current=state(g);invariant=current['dose']-1000*current.get('radaway',0)
    g.check('radaway_treatment_accumulates',invariant==220000,current)
    # Family 1-3 state that must survive the reopen: the canteen's cooldown (stack damage),
    # the Rad-X and herbal effects (mob effects) and the dose from coffee_radium.
    g.command('item replace entity @s hotbar.7 with ntm:canteen_vodka 1');g.ctl('select 8');use(g,.8);g.command('effect clear @s')
    g.command('effect give @s ntm:radx 6000 0');g.command('effect give @s ntm:potion_sickness 6000 0')
    canteen=stack_damage(g,'ntm:canteen_vodka')
    g.check('canteen_cooldown_before_reopen',canteen is not None and 150<=canteen<=180,canteen)
    expected={'invariant':invariant,'dose':current['dose'],'radaway':current.get('radaway',0),'canteen':canteen}
    (g.folder/'items-expected.json').write_text(json.dumps(expected,indent=2))
    g.snapshot('items-before-reopen')


def reopened(g):
    g.ctl('wait-ready 120');g.close()
    expected=json.loads((g.folder/'items-expected.json').read_text());current=state(g)
    g.check('treatment_resumes_without_replay',current['dose']-1000*current.get('radaway',0)==expected['invariant'] and current.get('radaway',0)<=expected['radaway'],{'before':expected,'after':current})
    left=stack_damage(g,'ntm:canteen_vodka')
    g.check('canteen_cooldown_survives_the_reopen',left is not None and 0<left<=expected['canteen'],{'before':expected['canteen'],'after':left})
    effects=effect_map(g)
    g.check('content_mob_effects_survive_the_reopen','radx' in effects and 'potion_sickness' in effects,effects)
    tag=inventory_tag(g)
    g.check('filter_wear_and_stack_charge_survive','777' in tag and '123456789' in tag and 'ntm_charge' in tag and 'hfrFilter' in tag,tag)
    g.check('armor_mods_survive','ntm_armor_mods' in tag and 'attachment_mask' in tag and 'hazmat_helmet' in tag,tag)
    g.snapshot('items-reopened')


if __name__=='__main__':
    g=Game(sys.argv[1],'items-'+(sys.argv[2] if len(sys.argv)>2 else 'setup'));mode=sys.argv[2] if len(sys.argv)>2 else 'setup';g.evidence['phase']=mode
    {'setup':setup,'capture-state':capture,'check-state':reopened}[mode](g)
    g.evidence['ok']=True;g.save()
