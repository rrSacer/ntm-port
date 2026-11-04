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
    if seconds:
        g.ctl('mouse right down');time.sleep(seconds);g.ctl('mouse right up')
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
    energy(g)


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
    g.command('clear @s');g.command('effect give @s minecraft:hunger 1 255 true');time.sleep(1.6)
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
    expected={'invariant':invariant,'dose':current['dose'],'radaway':current.get('radaway',0)}
    (g.folder/'items-expected.json').write_text(json.dumps(expected,indent=2))
    g.snapshot('items-before-reopen')


def reopened(g):
    g.ctl('wait-ready 120');g.close()
    expected=json.loads((g.folder/'items-expected.json').read_text());current=state(g)
    g.check('treatment_resumes_without_replay',current['dose']-1000*current.get('radaway',0)==expected['invariant'] and current.get('radaway',0)<=expected['radaway'],{'before':expected,'after':current})
    tag=inventory_tag(g)
    g.check('filter_wear_and_stack_charge_survive','777' in tag and '123456789' in tag and 'ntm_charge' in tag and 'hfrFilter' in tag,tag)
    g.check('armor_mods_survive','ntm_armor_mods' in tag and 'attachment_mask' in tag and 'hazmat_helmet' in tag,tag)
    g.snapshot('items-reopened')


if __name__=='__main__':
    g=Game(sys.argv[1],'items-'+(sys.argv[2] if len(sys.argv)>2 else 'setup'));mode=sys.argv[2] if len(sys.argv)>2 else 'setup';g.evidence['phase']=mode
    {'setup':setup,'capture-state':capture,'check-state':reopened}[mode](g)
    g.evidence['ok']=True;g.save()
