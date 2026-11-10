#!/usr/bin/env python3
"""Real use animations, counted commits, armor-table UI and reopen acceptance."""
import json
import re
import sys
import os
import time
from pathlib import Path
from native_game import Game


FAMILIES=os.environ.get('NTM_FAMILIES','energy,medicine,foods,kits,filters,instruments,blocks,vnt,protection,masks,decon').split(',')


FAMILY_RUNS=[]


def counts(g):
    return g.ctl('inventory')['counts']


def state(g):
    text=' '.join(g.command('ntmitem inspect').get('chat',[]))
    result={key:int(value) for key,value in re.findall(r'(dose|radaway|sickness|herbal):\s*(\d+)[lL]?',text)}
    if 'dose' not in result:raise AssertionError(text)
    return result


def use(g,seconds=0):
    """Use the held item: a click, or held for `seconds` of game time (20 server ticks a second)."""
    g.close();g.ctl('look-at 20 160 8')
    if seconds:
        start=game_time(g);g.ctl('mouse right down');wait_ticks(g,start,round(seconds*20)//(4 if MUTANT=='use_short' else 1))
        try:g.ctl('mouse right up')
        except RuntimeError as error:
            # A use that killed the player leaves the death screen: nothing to release.
            if 'dead' not in str(error):raise
    else:
        # One click: a held button repeats the use of an instant item on a slow frame.
        g.ctl('mouse right click')
    time.sleep(.8)  # a loaded machine answers a click a few ticks late


# Consumables whose checks are about game time wait in server ticks, not wall time (coordinator, 10/11): on a
# loaded machine a second of wall time is any number of ticks.
EAT_TICKS=32   # ItemFood.getMaxItemUseDuration (cotton candy, vegan schnitzel)
SYNC_TICKS=10  # tolerance: the client's copy of the inventory and the effects trails the server by a few ticks
# The server's tick rate while a check needs a release inside a use: a loaded machine takes seconds a controller
# command (a release measured 15 s late), so 1 tick a second keeps it within the 22 ticks a cancel has.
SLOW_RATE=1


def game_time(g):
    text=' '.join(g.command('time query gametime').get('chat',[]))
    n=re.search(r'game time is (\d+)',text)
    if not n:raise AssertionError('missing game time: '+text)
    return int(n.group(1))


# A deliberate failure for one converted check (tools/test-game: the proof that a check moved to tick or state
# waits still catches what it caught): see each use of MUTANT. Never set outside a mutation run.
MUTANT=os.environ.get('NTM_ITEMS_MUTANT','')


def settle_ticks(g,read,done,ticks=40):
    """`read()` once `done` holds for it or `ticks` server ticks pass; the check keeps its own verdict."""
    start=game_time(g)
    while True:
        value=read()
        if done(value) or game_time(g)>=start+ticks:return value
        time.sleep(.1)


def wait_ticks(g,since,n,timeout=180):
    """Until the server's game time is `n` ticks past `since`; returns it."""
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        now=game_time(g)
        if now>=since+n:return now
        time.sleep(.05)
    raise TimeoutError(f'{n} ticks')


def cancel_use(g,item,count,ticks,duration,release_by=None,on_try=None):
    """Hold the use of the stack in hand for `ticks` server ticks and release it, well short of its `duration`
    (or of `release_by`, when the check needs the release earlier); `on_try` runs as each try starts.
    The server ticks on while the controller waits for the client thread, which a loaded machine slows to over a
    second a command: at SLOW_RATE ticks a second the release still lands inside the use. A try whose release reached
    the server after the use could complete is no cancel: it gets its stack back and is repeated. Returns the
    state while held and the ticks held; waits out `duration` so a late completion would show."""
    if MUTANT=='late_release:'+item:ticks=duration+SYNC_TICKS
    g.command(f'tick rate {SLOW_RATE}')
    try:
        for attempt in range(3):
            if on_try:on_try()
            g.close();g.ctl('look-at 20 160 8')
            start=game_time(g);g.ctl('mouse right down');wait_ticks(g,start,ticks)
            held=g.ctl('state');g.ctl('mouse right up');released=game_time(g)
            if released-start<(release_by or duration-SYNC_TICKS) or attempt==2:break
            # What the completed use left (the containers, a ring pull) goes with it; the last try's stays for
            # the check to see.
            g.command('clear @s');give_hand(g,item,count)
        wait_ticks(g,released,duration+SYNC_TICKS)
    finally:
        g.command('tick rate 20')
    return held,released-start


def use_one(g,duration):
    """Use one of a stack in hand through its `duration` and release before a second is done: at SLOW_RATE ticks a
    second (see cancel_use), so only for checks that read no effect durations afterwards."""
    ticks=duration+SYNC_TICKS if MUTANT!='second_one_too' else 2*duration+SYNC_TICKS
    g.close();g.ctl('look-at 20 160 8');g.command(f'tick rate {SLOW_RATE}')
    try:
        start=game_time(g);g.ctl('mouse right down');wait_ticks(g,start,ticks)
        g.ctl('mouse right up');wait_ticks(g,game_time(g),SYNC_TICKS)
    finally:
        g.command('tick rate 20')


def server_effects(g):
    """id -> amplifier from the server's own copy of the effects (`/data`)."""
    text=' '.join(g.command('data get entity @s active_effects').get('chat',[]))
    out={}
    for body in re.findall(r'\{[^{}]*\}',text):
        i=re.search(r'id:\s*"(?:minecraft:)?(\w+)"',body)
        a=re.search(r'amplifier:\s*(\d+)b',body)
        if i:out.setdefault(i.group(1),int(a.group(1)) if a else 0)
    return out


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
        # The old menu is resent before the answer, so a slot can show the click and blank again for a tick:
        # the screen counts as settled only when a second look, after a tick, still agrees.
        if done(screen):
            time.sleep(.15)
            again=g.ctl('screen')
            if done(again):return again
        if time.monotonic()>deadline:return screen
        time.sleep(.25)


def try_put(g,item,slot):
    """Carry `item` to table slot `slot`; a refused slot leaves it on the cursor, put back where it was."""
    g.command(f'give @s {item} 1')
    screen=settle(g,lambda s:any(x['slot']<36 and x.get('item')==item for x in s.get('slots',[])),timeout=20)
    source=next(x['slot'] for x in screen.get('slots',[]) if x['slot']<36 and x.get('item')==item)
    g.ctl(f'click-slot {source}');g.ctl(f'click-slot {300+slot}');time.sleep(.8)
    screen=g.ctl('screen')
    if screen.get('carried'):g.ctl(f'click-slot {source}');time.sleep(.3)
    return table_slot(screen,slot)


def put(g,item,count,slot):
    g.command(f'give @s {item} {count}')
    # A loaded machine syncs the given stack late: wait longer than the click answers.
    screen=settle(g,lambda s:any(x['slot']<36 and x.get('item')==item for x in s.get('slots',[])),timeout=20)
    source=next((x['slot'] for x in screen.get('slots',[]) if x['slot']<36 and x.get('item')==item),None)
    if source is None:raise AssertionError(f'item not received: {item}')
    g.ctl(f'click-slot {source}');g.ctl(f'click-slot {300+slot}')
    screen=settle(g,lambda s:not s.get('carried') and any(x['slot']==300+slot and x.get('item')==item for x in s.get('slots',[])))
    g.check(f'insert_{item}_{slot}',not screen.get('carried') and any(x['slot']==300+slot and x.get('item')==item for x in screen.get('slots',[])),screen)


def setup(g):
    g.ctl('wait-ready 120');g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.ground(20,101,8,width=12,height=8)
    if os.environ.get('NTM_ONLY'):
        # A rerun of the named families alone (on a loaded machine the proof batch's clicks are timing-sensitive).
        for name,run in FAMILY_RUNS:
            if name in FAMILIES:run(g)
        return
    probe=g.command('ntmitem transaction-check');g.check('native_cas_refusal_replay_and_restore',any('true' in line for line in probe.get('chat',[])),probe)
    # The cotton candy is always edible and carries the five Java effects.
    give_hand(g,'ntm:cotton_candy',2)
    # EntityPlayer.canEat is `!capabilities.disableDamage`: 1.7.10 creative cannot eat.
    g.ctl('look-at 20 160 8');g.ctl('mouse right down');time.sleep(.45)
    refused=g.ctl('state');g.ctl('mouse right up');time.sleep(.2)
    g.check('creative_cannot_eat_item_food',refused.get('using') is None,refused)
    g.command('gamemode survival @s')
    active,held=cancel_use(g,'ntm:cotton_candy',2,9,EAT_TICKS)
    g.check('real_eating_started',active.get('using') is not None,active)
    g.check('cancel_does_not_consume',counts(g).get('ntm:cotton_candy')==2,{'counts':counts(g),'held_ticks':held})
    # The hunger (20 ticks) drains the food; the candy is eaten held for EAT_TICKS and a margin, at SLOW_RATE ticks a
    # second so the release lands before a second one is done (EAT_TICKS more).
    # The saturation goes first (a candy eaten by a late release above leaves plenty): until the food is down.
    for _ in range(6):
        g.command('effect give @s minecraft:hunger 1 255 true');wait_ticks(g,game_time(g),20+SYNC_TICKS)
        if g.ctl('state')['player']['food']<=17:break
    before=g.ctl('state')['player'];g.close();g.ctl('look-at 20 160 8');g.command(f'tick rate {SLOW_RATE}')
    start=game_time(g)
    if MUTANT!='food_not_eaten':g.ctl('mouse right down')
    wait_ticks(g,start,EAT_TICKS+SYNC_TICKS)
    g.ctl('mouse right up');released=game_time(g);wait_ticks(g,released,SYNC_TICKS)
    # The server's copy, read while it still runs at SLOW_RATE ticks a second: the client counted the effects down at
    # 20 a second meanwhile, and at 20 a slow controller lets the 100-tick wither (ItemCottonCandy:23) run out.
    if MUTANT=='no_wither':g.command('effect clear @s minecraft:wither')
    effects=server_effects(g);g.command('tick rate 20')
    after=g.ctl('state')['player']
    g.check('food_and_saturation_apply',after['food']>before['food'] and after['saturation']>before['saturation'],
            {'before':before,'after':after,'held_ticks':released-start})
    g.check('only_one_food_consumed',counts(g).get('ntm:cotton_candy')==1,counts(g))
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
        # The dose is added every RATE_EVERY (20) ticks of the NTM server, which ticks only while the world runs:
        # frozen, stepped 200 ticks and frozen again, the window holds exactly ten increments however slow the
        # controller (it was a polled single increment, which a slow poll merged with the next).
        g.command('tick freeze');before=state(g)['dose'];start=game_time(g)
        g.command('tick step 200');wait_ticks(g,start,200)
        after=state(g)['dose'];g.command('tick unfreeze')
        return (after-before)//10
    if MUTANT=='mask_unprotective':g.command('item replace entity @s armor.head with minecraft:leather_helmet')
    mask=rate_sample()
    g.command('item replace entity @s armor.head with minecraft:air');bare=rate_sample()
    g.command('item replace entity @s armor.chest with ntm:hazmat_plate');hazmat=rate_sample()
    # The window is exact, so the values are: 350 a second bare, the M65 helmet's 0.095 makes 281 and the hazmat
    # plate's 201 (radiation_mod, 10^-resistance, in Java float); a halved exponent or another sum dies here.
    g.check('mask_and_hazmat_reduce_gamma',0<mask<bare and 0<hazmat<bare and mask==281 and hazmat==201,{'mask':mask,'bare':bare,'hazmat':hazmat})
    g.command('gamemode creative @s');g.command('clear @s');g.command('ntmitem dose 200000')
    give_hand(g,'ntm:radaway');use(g);time.sleep(7.5)
    g.check('radaway_exact_java_total',state(g)['dose']==60000,state(g))
    g.check('radaway_returns_iv_empty',counts(g).get('ntm:iv_empty')==1,counts(g))
    full_inventory(g)
    filter_destruction(g)
    armor_table(g)
    death_resets_dose(g)
    # NTM_FAMILIES narrows a rerun to the families a binary has (default: all).
    for name,run in FAMILY_RUNS:
        if name in FAMILIES:run(g)

    # All families above use real items. Assert resolution in the window rather
    # than merely checking that the server accepted a named sound event.
    from native_sounds import resolved, log_lines
    for event in ['ntm:item.syringe', 'ntm:item.radaway', 'ntm:item.unpack',
                  'ntm:player.gulp', 'ntm:player.groan', 'ntm:item.techboop']:
        lines = resolved(g, event)
        g.check('audio_' + event, bool(lines), lines[:2])
    clicks = [line for line in log_lines(g) if 'sound resolved event=ntm:item.geiger' in line]
    g.check('audio_instrument_clicks', bool(clicks) and all('decoded=Some((' in line for line in clicks), clicks[:6])
    g.check('unregistered_vomit_has_no_alias', not any('event=ntm:entity.vomit ' in line for line in log_lines(g)), 'ItemLemon original event stays silent')


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
    use_one(g,EAT_TICKS);actual=counts(g)
    g.check('full_food_consumes_one',actual.get('ntm:cotton_candy')==1 and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    g.command('effect clear @s')
    # ItemSyringe: the used-up stack becomes the empty syringe in its own slot...
    full(g,'ntm:syringe_metal_medx',1);use(g);actual=counts(g)
    g.check('full_last_syringe_becomes_empty_in_place',actual.get('ntm:syringe_metal_empty')==1 and not actual.get('ntm:syringe_metal_medx') and not dropped(g),{'counts':actual,'dropped':dropped(g)})
    # ...and with more in the stack the empty syringe is dropped, never lost. The level is frozen from before the
    # use (`/tick freeze` ticks only the players): the dropped container's pickup delay does not run while a slow
    # controller frees the slot and looks.
    g.command('tick freeze')
    full(g,'ntm:syringe_metal_medx',2);use(g);actual=counts(g);away=dropped(g)
    effects=g.ctl('state')['player']['effects']
    g.check('full_stacked_syringe_drops_container',actual.get('ntm:syringe_metal_medx')==1 and not actual.get('ntm:syringe_metal_empty') and away.get('ntm:syringe_metal_empty')==1 and any(e['id'].endswith('resistance') and e['amplifier']==2 for e in effects),{'counts':actual,'dropped':away,'effects':effects})
    # `Player.drop(stack, false)`: pickup delay 40. Still frozen, the freed slot is stepped 30 ticks (the syringe
    # stays on the ground) and 15 more (it is back): 40, and neither 60 nor 80.
    g.command('item replace entity @s inventory.26 with minecraft:air');time.sleep(.4)
    if MUTANT=='pickup_delay_run_out':g.command('tick unfreeze');wait_ticks(g,game_time(g),40+SYNC_TICKS);g.command('tick freeze')
    if MUTANT=='pickup_delay_60':g.command('data merge entity @e[type=item,distance=..8,sort=nearest,limit=1] {PickupDelay:60s}')
    def stepped(n):
        start=game_time(g);g.command(f'tick step {n}');wait_ticks(g,start,n)
        # Frozen again, the server holds still and only the client's copy may trail: read until it repeats a
        # second apart (a wall-clock bound is safe here, nothing ticks).
        deadline=time.monotonic()+10;last=None
        while True:
            now=counts(g).get('ntm:syringe_metal_empty',0)
            if now==last or time.monotonic()>deadline:return now
            last=now;time.sleep(1)
    early=stepped(30);late=stepped(15);g.command('tick unfreeze')
    g.check('thrown_container_waits_its_pickup_delay',early==0 and late==1,{'early':early,'late':late,'dropped':dropped(g)})
    g.command('effect clear @s')
    # ItemSimpleConsumable.doRadaway: the emptied slot is still taken while the
    # container is added, so even the last RadAway drops its iv_empty.
    g.command('ntmitem dose 200000')
    full(g,'ntm:radaway',1);use(g)
    def read():
        if MUTANT=='radaway_container_lost':g.command('kill @e[type=item,distance=..8]')
        return counts(g),dropped(g)
    actual,away=settle_ticks(g,read,lambda v:not v[0].get('ntm:radaway') and v[1].get('ntm:iv_empty')==1);treated=state(g)
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


def has_effects(effects,expected,elapsed=80):
    """Amplifier exact; the duration is the Java one minus the ticks since (`elapsed`, counted in game ticks
    by the callers that measure it, else 80)."""
    return all(k in effects and effects[k][0]==amp and ticks-elapsed<=effects[k][1]<=ticks for k,(amp,ticks) in expected.items())


def eat_ticks(g,item):
    """Eat `item` holding the button for EAT_TICKS and a margin of game ticks; returns the tick it started."""
    give_hand(g,item);g.close();g.ctl('look-at 20 160 8')
    start=game_time(g);g.ctl('mouse right down');wait_ticks(g,start,EAT_TICKS+SYNC_TICKS);g.ctl('mouse right up')
    wait_ticks(g,start,EAT_TICKS+2*SYNC_TICKS)
    return start


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
    active,held=cancel_use(g,'ntm:can_smart',2,10,32)  # ItemEnergy.getMaxItemUseDuration
    g.check('can_drink_animation_runs',active.get('using') is not None,active)
    g.check('can_cancel_keeps_it',counts(g).get('ntm:can_smart')==2 and not counts(g).get('ntm:ring_pull'),{'counts':counts(g),'held_ticks':held})
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
    def gulps(since):
        # The `gulp` trace carries the use tick (counted from 0) `onUsingTick` would play it on.
        log=(g.folder/'client-first.log').read_text(errors='replace')[since:]
        return [int(m) for m in re.findall(r'"event":"gulp","item":"(\d+)"',log)]
    def log_size():
        return len((g.folder/'client-first.log').read_text(errors='replace'))
    g.command('clear @s');give_hand(g,'ntm:bdcl',2);mark=log_size();use_one(g,40)  # ItemBDCL.getMaxItemUseDuration
    g.check('bdcl_is_consumed_without_a_container',counts(g).get('ntm:bdcl')==1 and len(counts(g))==1,counts(g))
    full_use=[0,5,10,15,19,26];heard=gulps(mark)
    # The button is still down when the first drink ends, so a second use may begin: its gulps start over.
    g.check('bdcl_gulps_on_the_java_counts_exactly',heard[:6]==full_use and heard[6:]==full_use[:len(heard)-6],heard)
    # Cancelled after about twelve ticks: only the gulps already due are heard, and nothing is consumed.
    # ItemBDCL.getMaxItemUseDuration is 40; at most four gulps (0, 5, 10, 15) means released before tick 19.
    give_hand(g,'ntm:bdcl',2);marks=[]
    cancel_use(g,'ntm:bdcl',2,11,40,release_by=19,on_try=lambda:marks.append(log_size()))
    heard=gulps(marks[-1])
    g.check('cancelled_bdcl_stops_its_gulps',heard==[0,5,10,15,19,26][:len(heard)] and 2<=len(heard)<=4 and counts(g).get('ntm:bdcl')==2,{'heard':heard,'counts':counts(g)})
    # The chocolate milk is the last: its blast removes the floor around the player.
    g.command('gamemode creative @s');g.command('clear @s');give_hand(g,'ntm:chocolate_milk',1)
    floor=g.ctl('block 20 100 8').get('state','');use(g,1.8);time.sleep(4)
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
    g.close();g.command(f'gamemode {mode} @s');g.command('clear @s');g.command('ntmitem cure');g.command('effect clear @s');g.command('kill @e[type=minecraft:item]')
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
    """Family 2: ItemSyringe, ItemSimpleConsumable, ItemPill.

    Cases: stimpak/psycho/super/med_bag/taint/antidote/awesome effects and containers; the
    RadAway family (strong total, flush counter, the antidote clearing it); IV blood and XP;
    Rad-X, iodine and the herbal pill (dose, effects, sickness refusing the next pill, milk
    leaving the sickness alone); fmn, chocolate, plan C and the poison syringe (Resistance V
    absorbs the euthanasia, a survival player without it dies)."""
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
    # The antidote is `clearActivePotions`: the radaway potion goes with the effect list, so the dose stops falling.
    hold(g,'ntm:syringe_antidote',0);cleared=state(g);time.sleep(1.2);later=state(g)
    g.check('antidote_clears_the_radaway_treatment',cleared.get('radaway',0)==0 and later.get('radaway',0)==0 and later['dose']==cleared['dose'],{'treated':treated,'cleared':cleared,'later':later})
    # Milk clears the ordinary potions too (`HbmPotion.radaway` has curative items): the treatment ends and the dose stays.
    reset(g,dose=500000);hold(g,'ntm:radaway_flush',0);treated=state(g);give_hand(g,'minecraft:milk_bucket');use(g,2.2);cleared=state(g);time.sleep(1.2);later=state(g)
    g.check('milk_ends_the_radaway_treatment',treated.get('radaway',0)>=400 and cleared.get('radaway',0)==0 and later.get('radaway',0)==0 and later['dose']==cleared['dose'],{'treated':treated,'cleared':cleared,'later':later})
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
    g.check('potion_sickness_blocks_the_next_pill',refused(g,'ntm:pill_herbal',1),counts(g))
    # `HbmPotion.potionsickness` has no curative items: milk clears the other effects and leaves it running.
    reset(g);hold(g,'ntm:pill_herbal',.9,1);give_hand(g,'minecraft:milk_bucket');use(g,2.2);time.sleep(.6);effects=effect_map(g)
    g.check('milk_does_not_cure_the_herbal_sickness','potion_sickness' in effects and effects['potion_sickness'][1]>11000 and 'weakness' not in effects and 'mining_fatigue' not in effects,effects)
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
    # `euthanizedSelf` only bypasses armor: Resistance V (amplifier 4) absorbs all of it, the syringe and the pill alike.
    reset(g);g.command('effect give @s minecraft:resistance 120 4 true');hold(g,'ntm:syringe_poison',0);time.sleep(.6);alive=g.ctl('state')['player']
    g.check('resistance_absorbs_the_euthanasia_syringe',alive.get('death_time') is None and alive['health']>=19.9 and counts(g).get('ntm:syringe_empty')==1,{'player':alive,'counts':counts(g)})
    reset(g);g.command('effect give @s minecraft:resistance 120 4 true');hold(g,'ntm:plan_c',.9);time.sleep(1.2);alive=g.ctl('state')['player']
    g.check('resistance_absorbs_plan_c',alive.get('death_time') is None and alive['health']>=19.9 and not counts(g),{'player':alive,'counts':counts(g)})
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
    # ItemLemon's setAlwaysEdible guard compares a field that is still null in the constructor: needs hunger.
    reset(g);g.check('ipecac_needs_hunger_at_full_food',refused(g,'ntm:med_ipecac',1),counts(g))
    reset(g);g.check('ptsd_needs_hunger_at_full_food',refused(g,'ntm:med_ptsd',1),counts(g))
    reset(g);starve(g);hold(g,'ntm:med_ipecac',1.8);effects=effect_map(g)
    g.check('ipecac_when_hungry_induces_hunger',has_effects(effects,{'hunger':(49,50)}) and not counts(g),effects)
    reset(g);starve(g);g.command('effect give @s minecraft:slow_falling 120 0 true');g.command('effect give @s minecraft:fire_resistance 120 0 true')
    start=eat_ticks(g,'ntm:schnitzel_vegan')
    # 160 ticks: the nearest whole-second duration to a 170-tick blindness that the review asked to kill.
    if MUTANT=='schnitzel_short_blindness':g.command('effect clear @s minecraft:blindness');g.command('effect give @s minecraft:blindness 8 0 true')
    effects=effect_map(g);elapsed=game_time(g)-start
    # The effects are given when the eating ends, at least EAT_TICKS after `start`, and read before `elapsed`: they
    # have run at most elapsed - EAT_TICKS. Given in one tick, they also keep the Java's differences exactly.
    remaining=lambda k:effects.get(k,(0,0))[1]
    g.check('vegan_schnitzel_java_effects',has_effects(effects,{'blindness':(0,200),'nausea':(0,600),'hunger':(4,3600)},elapsed-EAT_TICKS)
            and remaining('nausea')-remaining('blindness')==400 and remaining('hunger')-remaining('blindness')==3400
            and not counts(g).get('ntm:schnitzel_vegan'),{'effects':effects,'elapsed_ticks':elapsed})
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
    M65's blacklist still refuses blistering gas, the hazmat hoods' empty one does not.

    Cases: mono/combo/rag/piss on the M65 (install, then wear or ignore per hazard class);
    the three hazmat hoods (install, blistering and lung gas wear the filter, sneak-use
    returns it). Ambient sources are not connected: only `ntmitem expose` calls the exposure."""
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
    # `ArmorHazmatMask implements IGasMask` with an empty blacklist: every hood takes a filter,
    # wears it, lets it stop blistering gas (the M65 refuses) and gives it back when sneak-used in hand.
    for hood in ('hazmat_helmet','hazmat_helmet_red','hazmat_helmet_grey'):
        reset(g,'creative');g.command(f'item replace entity @s armor.head with ntm:{hood}');give_hand(g,'ntm:gas_mask_filter_combo');use(g)
        g.check(f'{hood}_takes_a_filter',not counts(g).get('ntm:gas_mask_filter_combo') and 'hfrFilter' in inventory_tag(g),inventory_tag(g))
        before=inventory_tag(g);g.command('ntmitem expose blister 40');after=inventory_tag(g)
        g.check(f'{hood}_filter_stops_blistering_gas',after!=before,{'before':before,'after':after})
        before=after;g.command('ntmitem expose lung 40')
        g.check(f'{hood}_filter_stops_lung_gas',inventory_tag(g)!=before,inventory_tag(g))
        g.command('item replace entity @s hotbar.3 from entity @s armor.head');g.command('item replace entity @s armor.head with minecraft:air')
        g.command('item replace entity @s hotbar.0 from entity @s hotbar.3');g.command('item replace entity @s hotbar.3 with minecraft:air');g.ctl('select 1');time.sleep(.3)
        g.ctl('key shift down');time.sleep(.4);use(g);g.ctl('key shift up');time.sleep(.3)
        g.check(f'{hood}_gives_the_filter_back_when_sneak_used',counts(g).get('ntm:gas_mask_filter_combo')==1 and 'hfrFilter' not in inventory_tag(g),{'counts':counts(g),'tag':inventory_tag(g)})
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
    # radBuf (radEnv -> radBuf once a second): a bottle's point `contaminate` joins the continuous
    # source in the same reading, once, while Rad-X and an active RadAway change nothing in it;
    # coffee_radium's direct `incrementRadiation` never enters it.
    def radbuf_window(seconds=3.4):
        seen=[];end=time.monotonic()+seconds
        while time.monotonic()<end:
            seen.append(int(' '.join(g.command('ntmitem radbuf').get('chat',[]))));time.sleep(.1)
        return seen
    def drink_and_sample(item,slot,drink=2.0,total=5.5):
        # Sample while drinking: a point event lives in one one-second reading, which a sample taken
        # only after the drink may have missed.
        give_hand(g,item,1,slot);g.close();g.ctl('look-at 20 160 8');g.ctl('mouse right down')
        seen=[];start=time.monotonic();up=False
        while time.monotonic()-start<total:
            if not up and time.monotonic()-start>=drink:g.ctl('mouse right up');up=True
            seen.append(int(' '.join(g.command('ntmitem radbuf').get('chat',[]))))
        if not up:g.ctl('mouse right up')
        return seen
    reset(g,'creative',dose=300000);g.command('item replace entity @s hotbar.5 with ntm:ingot_uranium 8');g.command('item replace entity @s hotbar.6 with ntm:bottle_opener 1');time.sleep(.5)
    base=max(radbuf_window(2.6));g.check('continuous_source_reads_in_the_buffer',base>0,base)
    g.command('effect give @s ntm:radx 600 0');hold(g,'ntm:radaway_flush',0)
    seen=drink_and_sample('ntm:bottle_nuka',1)
    g.check('bottle_contaminate_joins_the_buffer_once',max(seen)==base+5000 and seen.count(base+5000)>=1 and seen[-1]==base,{'base':base,'seen':sorted(set(seen)),'last':seen[-1]})
    seen=drink_and_sample('ntm:coffee_radium',2)
    g.check('direct_increment_stays_out_of_the_buffer',max(seen)==base,{'base':base,'seen':sorted(set(seen))})
    g.command('clear @s')
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


def on_blocks(g):
    """The match and the fertilizer used on blocks (ItemMatch, ItemFertilizer) and the AMS lens stack."""
    # ItemMatch.onItemUse: fire on the cell beside the clicked face; the match is spent outside creative.
    reset(g);g.ground(20,101,8,width=12,height=8);g.command('tp @s 20.5 101 11.5');time.sleep(.5)
    give_hand(g,'ntm:matchstick',2);g.close();g.ctl('interact 20 100 8 up');time.sleep(.6)
    state_=g.ctl('block 20 101 8').get('state','')
    g.check('match_lights_fire_on_the_clicked_face','fire' in state_ and counts(g).get('ntm:matchstick')==1,{'block':state_,'counts':counts(g)})
    g.command('setblock 20 101 8 minecraft:air')
    reset(g,'creative');give_hand(g,'ntm:matchstick',2);g.close();g.ctl('interact 22 100 8 up');time.sleep(.6)
    state_=g.ctl('block 22 101 8').get('state','')
    g.check('creative_match_is_not_spent','fire' in state_ and counts(g).get('ntm:matchstick')==2,{'block':state_,'counts':counts(g)})
    g.command('setblock 22 101 8 minecraft:air')
    # ItemFertilizer: bone meal on the 3x3x3 around the click; spent only when something grew.
    reset(g);g.command('setblock 22 100 8 minecraft:farmland[moisture=7]');g.command('setblock 22 101 8 minecraft:wheat[age=0]');time.sleep(.4)
    give_hand(g,'ntm:powder_fertilizer',3);g.close();g.ctl('interact 21 100 8 up');time.sleep(.8)
    age=re.search(r'age=(\d+)',g.ctl('block 22 101 8').get('state',''))
    g.check('fertilizer_grows_the_crop_around_the_click',age is not None and int(age.group(1))>0 and counts(g).get('ntm:powder_fertilizer')==2,{'age':age and age.group(1),'counts':counts(g)})
    g.command('setblock 22 101 8 minecraft:air')
    # Nothing to grow: the fertilizer stays.
    g.ctl('interact 19 100 9 up');time.sleep(.8)
    g.check('fertilizer_stays_when_nothing_grows',counts(g).get('ntm:powder_fertilizer')==2,counts(g))
    # The clicked cell grows by force (ItemFertilizer `force`), not by the 45% sapling roll. A valid growth
    # may only advance the sapling (`stage=0` to `stage=1`) or turn it into a tree, and either is growth.
    # Vanilla bone meal is the control: what it does to a sapling in this world is recorded, and the
    # fertilizer must change the clicked sapling in every trial, spending exactly one.
    def sapling_trial(item,stage):
        reset(g);g.command('fill 17 101 6 27 118 10 minecraft:air');g.command('setblock 22 100 8 minecraft:grass_block');g.command(f'setblock 22 101 8 minecraft:oak_sapling[stage={stage}]');time.sleep(.4)
        before=g.ctl('block 22 101 8').get('state','');give_hand(g,item,2);g.close();g.ctl('interact 22 101 8 up');time.sleep(.9)
        after=g.ctl('block 22 101 8').get('state','')
        return {'before':before,'after':after,'changed':after!=before,'tree':'sapling' not in after,'spent':counts(g).get(item)==1}
    vanilla=[sapling_trial('minecraft:bone_meal',stage) for stage in (0,0,0,0,0,1,1,1,1,1)]
    forced=[sapling_trial('ntm:powder_fertilizer',0) for _ in range(6)]
    summary={'vanilla_changed_of_10':sum(t['changed'] for t in vanilla),'vanilla_trees_of_10':sum(t['tree'] for t in vanilla),'vanilla_spent_of_10':sum(t['spent'] for t in vanilla),'vanilla_samples':[t['before']+' -> '+t['after'] for t in vanilla[:2]],
             'fertilizer_changed_of_6':sum(t['changed'] for t in forced),'fertilizer_trees_of_6':sum(t['tree'] for t in forced),'fertilizer_spent_of_6':sum(t['spent'] for t in forced),'fertilizer_samples':[t['before']+' -> '+t['after'] for t in forced[:2]]}
    if summary['vanilla_changed_of_10']:
        # The world does grow saplings: the forced cell must always change, and cost one each time.
        g.check('fertilizer_forces_the_clicked_cell',summary['fertilizer_changed_of_6']==6 and summary['fertilizer_spent_of_6']==6,summary)
    else:
        # Not a pass: vanilla bone meal changed no sapling either (stage or tree) in 10 trials, so the forced
        # cell cannot be told from the engine's own refusal in this world. The evidence says so.
        g.note('fertilizer_forced_cell_not_proven_in_game',summary)
    g.command('fill 17 101 6 27 118 10 minecraft:air')
    # The AMS lens: the tooltip line comes from the lens stack's `damage` long, read through the same
    # function as `ntmitem lens`; the socket cannot read a tooltip, the function is what it draws.
    reset(g,'creative');g.command('give @s ntm:ams_lens[minecraft:custom_data={damage:108000000L}] 1');g.ctl('select 1');time.sleep(.5)
    tag=components(g,'ntm:ams_lens') or ''
    line=' '.join(g.command('ntmitem lens').get('chat',[]))
    g.check('ams_lens_tooltip_reads_its_damage_tag','108000000' in tag and 'Durability: 324000000/432000000 (75%)' in line,{'tag':tag,'line':line})
    g.command('clear @s');g.command('give @s ntm:ams_lens 1');g.ctl('select 1');time.sleep(.4)
    line=' '.join(g.command('ntmitem lens').get('chat',[]))
    g.check('fresh_ams_lens_is_at_full_durability','Durability: 432000000/432000000 (100%)' in line,line)
    g.ground(20,101,8,width=12,height=8);reset(g,'creative')


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
    screen=settle(g,lambda s:table_slot(s,0)=='ntm:attachment_mask')
    g.check('mod_slot_shows_attachment',table_slot(screen,0)=='ntm:attachment_mask',screen.get('slots'))
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
    put(g,'ntm:hazmat_helmet',1,9);put(g,'ntm:attachment_mask',1,0);put(g,'ntm:cladding_lead',1,5)
    g.ctl('click-slot 309 shift');settle(g,lambda s:table_slot(s,10)=='ntm:hazmat_helmet' and not s.get('carried'));g.close();time.sleep(.5)
    mods=inventory_tag(g);g.check('helmet_worn_with_mod_before_reopen','ntm_armor_mods' in mods and 'attachment_mask' in mods and 'cladding_lead' in mods,mods)
    expected,_=resistance_line([0.6*0.2+0.1]);line=geiger_resistance(g)
    g.check('cladding_counts_before_reopen',line.endswith(expected),{'line':line,'expected':expected})
    # Two doses accumulate treatment time. The invariant detects timer reset,
    # lost dose or replayed treatment across the normal world close/reopen.
    g.command('ntmitem dose 500000')
    # The antidote clears the radaway potion with the rest (`clearActivePotions`); the reopen must not bring it back.
    give_hand(g,'ntm:radaway_flush',1,3);use(g);flushed=state(g).get('radaway',0)
    give_hand(g,'ntm:syringe_antidote',1,3);use(g);cleared=state(g).get('radaway',0)
    g.check('antidote_clears_radaway_before_reopen',flushed>=400 and cleared==0,{'flushed':flushed,'cleared':cleared})
    g.command('ntmitem dose 500000');give_hand(g,'ntm:radaway',2,1);use(g);use(g)
    current=state(g);invariant=current['dose']-1000*current.get('radaway',0)
    g.check('radaway_treatment_accumulates',invariant==220000,current)
    # Family 1-3 state that must survive the reopen: the canteen's cooldown (stack damage),
    # the Rad-X and herbal effects (mob effects) and the dose from coffee_radium.
    g.command('item replace entity @s hotbar.7 with ntm:canteen_vodka 1');g.ctl('select 8');use(g,.8);g.command('effect clear @s')
    canteen=stack_damage(g,'ntm:canteen_vodka')
    g.check('canteen_cooldown_before_reopen',canteen is not None and 150<=canteen<=180,canteen)
    # The herbal pill's sickness has no curative items: milk leaves it. The pill's -100 RAD moves the dose,
    # so the invariant is read after it.
    give_hand(g,'ntm:pill_herbal',1,3);use(g,.9);give_hand(g,'ntm:radaway_flush',1,3);use(g);treated=state(g).get('radaway',0)
    give_hand(g,'minecraft:milk_bucket',1,3);use(g,2.2);time.sleep(.6)
    effects=effect_map(g);g.check('herbal_sickness_survives_milk_before_reopen','potion_sickness' in effects and 'weakness' not in effects,effects)
    g.check('milk_ends_radaway_before_reopen',treated>=400 and state(g).get('radaway',0)==0,{'treated':treated,'after':state(g)})
    g.command('effect give @s ntm:radx 6000 0')
    current=state(g);invariant=current['dose']-1000*current.get('radaway',0)
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
    # The remembered sickness survived the reopen with its effect: milk still cannot take it away.
    give_hand(g,'minecraft:milk_bucket',1,3);use(g,2.2);time.sleep(.6);effects=effect_map(g)
    g.check('herbal_sickness_survives_milk_after_reopen','potion_sickness' in effects and 'radx' not in effects,effects)
    # No dose of RadAway was applied between clearing the counter (milk, with the antidote before it) and the reopen.
    g.check('cleared_radaway_stays_zero_after_the_reopen',expected['radaway']==0 and current.get('radaway',0)==0 and current['dose']==expected['dose'],{'before':expected,'after':current})
    tag=inventory_tag(g)
    g.check('filter_wear_and_stack_charge_survive','777' in tag and '123456789' in tag and 'ntm_charge' in tag and 'hfrFilter' in tag,tag)
    g.check('armor_mods_survive','ntm_armor_mods' in tag and 'attachment_mask' in tag and 'hazmat_helmet' in tag and 'cladding_lead' in tag,tag)
    expected,_=resistance_line([0.6*0.2+0.1]);line=geiger_resistance(g)
    g.check('cladding_still_counts_after_reopen',line.endswith(expected),{'line':line,'expected':expected})
    g.snapshot('items-reopened')


def java_float(x):
    import struct
    return struct.unpack('f',struct.pack('f',x))[0]


def java_resistance(pieces,radx=False):
    """HazmatRegistry.getResistance(player): a float that narrows after each piece, summed over
    `armorInventory[0..3]`, boots first; `pieces` is given as worn, helmet first."""
    res=0.0
    for piece in reversed(pieces):res=java_float(res+piece)
    if radx:res=java_float(res+java_float(0.2))
    return res


def resistance_line(pieces,radx=False):
    """The fifth Geiger line, computed here from the Java formulas (not from the port)."""
    res=java_resistance(pieces,radx);factor=java_float(10**-res)
    shown=int(10000-factor*10000)/100;koeff=int(res*100)/100
    # The chat log keeps the text without the § colour codes.
    return f"Player resistance: {shown}% ({koeff})",factor


GEIGER=[0]


def geiger_resistance(g):
    """The resistance line of this click's Geiger report. `chat-log` lists the newest message first
    (ChatHud.recent_lines), so the reports newer than a fresh marker are the ones before it. On a loaded machine
    an answer can come many seconds late: every click is counted, and the reading waits until there is one
    report per click; the newest is then this click's (the server answers in order)."""
    g.command('item replace entity @s hotbar.8 with ntm:geiger_counter');g.ctl('select 9')
    GEIGER[0]+=1;mark=f'geiger-mark-{GEIGER[0]}'
    g.command(f'tellraw @s "{mark}"')
    clicks=0
    for _ in range(3):
        use(g);clicks+=1
        deadline=time.monotonic()+30
        while time.monotonic()<deadline:
            chat=g.ctl('chat-log 60').get('chat',[])
            at=next((i for i,l in enumerate(chat) if mark in l),None)
            newer=[l for l in chat[:at] if 'Player resistance' in l] if at is not None else []
            if len(newer)>=clicks:
                return newer[0]
            time.sleep(.5)
    return ''


def wear(g,pieces):
    for slot,item in zip(('head','chest','legs','feet'),pieces):
        g.command(f'item replace entity @s armor.{slot} with {item}')
    # The worn armor must have reached the player before an instrument reads it.
    want={part:item.split('[')[0] for part,item in zip(('head','chest','legs','feet'),pieces) if item!='minecraft:air'}
    deadline=time.monotonic()+20
    while time.monotonic()<deadline:
        worn={s.get('part'):s.get('item') for s in g.ctl('inventory').get('armor',[])}
        if all(worn.get(p)==i for p,i in want.items()):break
        time.sleep(.3)
    time.sleep(.5)


def protection(g):
    """Onda 1, fatia 1: the whole HazmatRegistry, cladding and the Java armor stats."""
    hz={'hazmat_grey':[0.4,0.8,0.6,0.2],'steel':[0.045*f for f in (0.2,0.4,0.3,0.1)],'titanium':[0.045*f for f in (0.2,0.4,0.3,0.1)],
        'alloy':[0.07*f for f in (0.2,0.4,0.3,0.1)],'cobalt':[0.125*f for f in (0.2,0.4,0.3,0.1)],'starmetal':[1.0*f for f in (0.2,0.4,0.3,0.1)],
        'iron':[0.0225*f for f in (0.2,0.4,0.3,0.1)],'golden':[0.0225*f for f in (0.2,0.4,0.3,0.1)]}
    sets={'steel':'ntm:steel_{}','titanium':'ntm:titanium_{}','alloy':'ntm:alloy_{}','cobalt':'ntm:cobalt_{}','starmetal':'ntm:starmetal_{}'}
    parts=['helmet','plate','legs','boots'];vanilla=['helmet','chestplate','leggings','boots']
    # Every set's Geiger line against the Java float arithmetic.
    for name,pattern in sets.items():
        reset(g,'creative');wear(g,[pattern.format(p) for p in parts])
        expected,_=resistance_line(hz[name]);line=geiger_resistance(g)
        g.check(f'geiger_resistance_{name}_set',line.endswith(expected),{'line':line,'expected':expected})
    for name in ('iron','golden'):
        reset(g,'creative');wear(g,[f'minecraft:{name}_{p}' for p in vanilla])
        expected,_=resistance_line(hz[name]);line=geiger_resistance(g)
        g.check(f'geiger_resistance_vanilla_{name}_set',line.endswith(expected),{'line':line,'expected':expected})
    # Mixed sets, where the order of the float sum shows: the expected line is the Java oracle's
    # (tools/hazmat-oracle/fixture.json, cases 0 and 1), not a recomputation here.
    oracle=json.loads((Path(__file__).resolve().parents[1]/'hazmat-oracle/fixture.json').read_text())['cases']
    for n,worn in [(0,['minecraft:iron_helmet','ntm:steel_plate','ntm:cobalt_legs','ntm:hazmat_boots_red']),
                   (1,['minecraft:iron_helmet[minecraft:custom_data={ntm_armor_mods:{mod_slot_5:{id:"ntm:cladding_desh",count:1}}}]','ntm:steel_plate','ntm:cobalt_legs','ntm:hazmat_boots_grey'])]:
        java=oracle[n]['java'];expected=f"Player resistance: {java['res']}% ({java['koeff']})"
        reset(g,'creative');wear(g,worn);line=geiger_resistance(g)
        g.check(f'geiger_resistance_mixed_set_{n}_as_the_java_oracle',line.endswith(expected),{'line':line,'expected':expected})
    # NBTTagCompound.getFloat reads any numeric tag: a double hfr_cladding counts as the float one does.
    reset(g,'creative');wear(g,['ntm:hazmat_helmet_grey[minecraft:custom_data={hfr_cladding:0.25d}]'])
    expected,_=resistance_line([0.4+java_float(0.25)]);line=geiger_resistance(g)
    g.check('hfr_cladding_double_tag_counts',line.endswith(expected),{'line':line,'expected':expected})
    reset(g,'creative');wear(g,['ntm:hazmat_helmet_grey','ntm:hazmat_plate_grey','ntm:hazmat_legs_grey','ntm:hazmat_boots_grey'])
    expected,_=resistance_line(hz['hazmat_grey']);line=geiger_resistance(g)
    g.check('geiger_resistance_grey_hazmat_set',line.endswith(expected) and '99.0% (2.0)' in line,{'line':line,'expected':expected})
    # ArmorMaterial HBM_STEEL {3, 8, 6, 3}, durability 30: 20 armor points and 16 * 30 = 480 on the plate.
    reset(g,'creative');wear(g,[sets['steel'].format(p) for p in parts])
    armor=' '.join(g.command('attribute @s minecraft:armor get').get('chat',[]))
    g.check('steel_set_has_the_java_armor_points',re.search(r'\b20(\.0)?\b',armor) is not None,armor)
    # Durability 16 * 30 = 480: a hit takes one point from each piece, so a plate at 478 survives one cactus
    # hit at 479 and breaks on the next (an off-by-one max_damage changes which hit breaks it).
    reset(g,'survival');wear(g,['minecraft:air','ntm:steel_plate[minecraft:damage=478]'])
    g.command('damage @s 1 minecraft:cactus');time.sleep(.6);first=g.ctl('inventory')
    g.command('effect give @s minecraft:instant_health 1 5 true');g.command('damage @s 1 minecraft:cactus');time.sleep(.6);second=g.ctl('inventory')
    worn=lambda inv:[s for s in inv.get('armor',[]) if s.get('item')=='ntm:steel_plate']
    g.check('steel_plate_has_the_java_durability',[s.get('damage') for s in worn(first)]==[479] and not worn(second),{'first':first.get('armor'),'second':second.get('armor')})
    # The real gamma dose follows the same factor (bare step times the float factor, truncated).
    reset(g,'survival');give_hand(g,'ntm:ingot_uranium');time.sleep(11)
    def step():
        # The dose grows once a second; on a slow machine two seconds can land between samples,
        # so the smallest increment is the one-second step (a merged one is a multiple of it).
        last=state(g)['dose'];steps=[];deadline=time.monotonic()+30
        while len(steps)<4 and time.monotonic()<deadline:
            time.sleep(.2);now=state(g)['dose']
            if now!=last:steps.append(now-last);last=now
        return min(steps) if steps else 0
    bare=step();wear(g,[sets['steel'].format(p) for p in parts]);steel=step()
    _,factor=resistance_line(hz['steel'])
    g.check('steel_set_reduces_the_real_dose_by_the_java_factor',bare>0 and steel==int(bare*factor),{'bare':bare,'steel':steel,'factor':factor})
    # hfr_cladding (a float tag) adds to the piece and wins over the cladding slot.
    reset(g,'creative');wear(g,['ntm:hazmat_helmet_grey[minecraft:custom_data={hfr_cladding:0.25f}]'])
    expected,_=resistance_line([0.4+java_float(0.25)]);line=geiger_resistance(g)
    g.check('hfr_cladding_adds_to_the_helmet',line.endswith(expected),{'line':line,'expected':expected})
    # The cladding goes in slot 5 of the real table, only with an armor piece in it, and counts when worn.
    g.close();g.command('clear @s');g.command('setblock 24 101 8 ntm:machine_armor_table');g.command('tp @s 24.5 101 11.5')
    open_table(g)
    g.check('cladding_refused_without_armor',try_put(g,'ntm:cladding_lead',5) is None,g.slots())
    put(g,'ntm:hazmat_helmet_grey',1,9)
    g.check('cladding_refused_in_the_mask_slot',try_put(g,'ntm:cladding_lead',0) is None,g.slots())
    g.close();time.sleep(.5);g.command('clear @s');g.command('kill @e[type=minecraft:item]');open_table(g)
    put(g,'ntm:hazmat_helmet_grey',1,9);put(g,'ntm:cladding_lead',1,5)
    screen=settle(g,lambda s:table_slot(s,5)=='ntm:cladding_lead')
    g.check('cladding_installed_in_slot_5',table_slot(screen,5)=='ntm:cladding_lead',screen.get('slots'))
    g.ctl('click-slot 309 shift');settle(g,lambda s:table_slot(s,10)=='ntm:hazmat_helmet_grey' and not s.get('carried'));g.close();time.sleep(.5)
    tag=inventory_tag(g);g.check('cladding_kept_in_the_helmet_mods','ntm_armor_mods' in tag and 'cladding_lead' in tag,tag)
    expected,_=resistance_line([0.4+0.1]);line=geiger_resistance(g)
    g.check('cladding_mod_adds_to_the_worn_helmet',line.endswith(expected),{'line':line,'expected':expected})
    g.command('item replace entity @s hotbar.4 from entity @s armor.head')
    reset(g,'creative')


def expose(g,kind,wear):
    """`/ntmitem expose`: True when the helmet protects (ArmorRegistry.hasProtection)."""
    return 'true' in ' '.join(g.command(f'ntmitem expose {kind} {wear}').get('chat',[])).lower()


def masks(g):
    """Onda 1, fatia 2: the masks and suits, ArmorRegistry protection and filter wear."""
    parts=['helmet','plate','legs','boots']
    # HazmatRegistry and setRadResist: each set's Geiger line against the Java formula.
    for name,pieces,values in [('paa',[f'ntm:hazmat_paa_{p}' for p in parts],[1.7*f for f in (0.2,0.4,0.3,0.1)]),
                               ('liquidator',[f'ntm:liquidator_{p}' for p in parts],[2.4*f for f in (0.2,0.4,0.3,0.1)]),
                               ('gas_mask',['ntm:gas_mask'],[0.07]),('jackt',['minecraft:air','ntm:jackt'],[0.1])]:
        reset(g,'creative');wear(g,pieces)
        expected,_=resistance_line(values);line=geiger_resistance(g)
        g.check(f'geiger_resistance_{name}',line.endswith(expected),{'line':line,'expected':expected})
    # ArmorLiquidator: knockback 100 (clamped to 1) and speed -0.1 multiplied per piece.
    reset(g,'survival');wear(g,[f'ntm:liquidator_{p}' for p in parts])
    knock=' '.join(g.command('attribute @s minecraft:knockback_resistance get').get('chat',[]))
    speed=' '.join(g.command('attribute @s minecraft:movement_speed get').get('chat',[]))
    g.check('liquidator_attributes',re.search(r'is 1(\.0)?\b',knock) is not None and re.search(r'is 0\.0(6|59999)',speed) is not None,{'knockback':knock,'speed':speed})
    # A filter goes in every new IGasMask; the mono mask refuses lung gas even with the combo filter.
    for mask in ['ntm:gas_mask','ntm:gas_mask_olde','ntm:gas_mask_mono','ntm:hazmat_paa_helmet','ntm:liquidator_helmet']:
        reset(g,'creative');wear(g,[mask])
        give_hand(g,'ntm:gas_mask_filter_combo')
        # A click lost on a loaded machine is clicked again; once installed the hand is empty and a click does nothing.
        for _ in range(3):
            use(g)
            if not counts(g).get('ntm:gas_mask_filter_combo'):break
        g.check(f'{mask}_takes_a_filter','hfrFilter' in inventory_tag(g) and not counts(g).get('ntm:gas_mask_filter_combo'),inventory_tag(g))
        before=inventory_tag(g);lung=expose(g,'lung',40)
        if mask=='ntm:gas_mask_mono':
            g.check('mono_never_protects_lung_gas',not lung and inventory_tag(g)==before,{'lung':lung,'tag':inventory_tag(g)})
            co=expose(g,'co',40);g.check('mono_stops_monoxide_and_wears',co and inventory_tag(g)!=before,{'co':co,'tag':inventory_tag(g)})
        else:
            g.check(f'{mask}_stops_lung_gas_and_wears',lung and inventory_tag(g)!=before,{'lung':lung,'tag':inventory_tag(g)})
    # Own classes (ArmorUtil.register): no filter, protection, nothing to wear.
    for item,kind,expected in [('ntm:mask_rag','coarse',True),('ntm:mask_rag','lung',False),('ntm:mask_piss','lung',True),
                               ('ntm:goggles','light',True),('ntm:ashglasses','sand',True),('ntm:asbestos_helmet','light',True),
                               ('ntm:gas_mask_m65','sand',True),('ntm:gas_mask_m65','light',False),('ntm:steel_helmet','sand',False)]:
        reset(g,'creative');wear(g,[item]);before=inventory_tag(g)
        got=expose(g,kind,10)
        g.check(f'{item.split(":")[1]}_{kind}_{"protects" if expected else "does_not_protect"}',got==expected and inventory_tag(g)==before,{'got':got,'tag':inventory_tag(g)})
    filter_copy_bug(g)
    reset(g,'creative')


def filter_copy_bug(g):
    """JAVA_FILTER_COPY_BUG (ArmorUtil.java:332-352): an IGasMask helmet without a filter of its own and an
    attachment mask with one: the exposure is stopped, the worn copy lands in the helmet, the mod keeps its
    filter, and a sneak-use of the helmet hands the copy out: two filters from one."""
    reset(g,'survival')
    module='{id:"ntm:attachment_mask",count:1,components:{"minecraft:custom_data":{hfrFilter:{id:"ntm:gas_mask_filter",count:1}}}}'
    wear(g,['ntm:hazmat_helmet[minecraft:custom_data={ntm_armor_mods:{mod_slot_0:'+module+'}}]'])
    before=inventory_tag(g)
    g.check('copy_bug_setup_one_filter',before.count('gas_mask_filter')==1,before)
    stopped=expose(g,'lung',40);tag=inventory_tag(g)
    g.check('copy_bug_protects_through_the_mod',stopped,tag)
    g.check('copy_bug_writes_the_worn_copy_into_the_helmet',tag.count('gas_mask_filter')==2 and '40' in tag,tag)
    g.command('item replace entity @s hotbar.0 from entity @s armor.head');g.command('item replace entity @s armor.head with minecraft:air');g.ctl('select 1');time.sleep(.5)
    g.ctl('key shift down');time.sleep(.4);use(g);g.ctl('key shift up');time.sleep(.6)
    helmet=components(g,'ntm:hazmat_helmet') or '';loose=counts(g).get('ntm:gas_mask_filter',0)+dropped(g).get('ntm:gas_mask_filter',0)
    # The copy is out; the helmet keeps only the mod's filter (one, nested in mod_slot_0).
    g.check('copy_bug_sneak_hands_out_the_copy_and_the_mod_keeps_its_filter',loose==1 and helmet.count('gas_mask_filter')==1 and 'mod_slot_0' in helmet,{'loose':loose,'helmet':helmet})


def dose_drop(g,seconds=2.0):
    before=state(g)['dose'];time.sleep(seconds);return before-state(g)['dose']


def decon(g):
    """Onda 1, fatia 3: TileEntityDecon, -0.5 RAD a tick for each decontaminator whose box reaches the player,
    and the end of the radiation potion (HbmPotion.radiation, the port's irradiation)."""
    g.ground(20,101,8,width=12,height=8)
    under='20 100 11'
    # One under the feet: the dose falls by half a RAD per tick, so by a multiple of 500 milliRAD.
    reset(g,'survival',dose=300000);g.command(f'setblock {under} ntm:decon');time.sleep(.5)
    drop=dose_drop(g)
    g.check('one_decon_takes_half_a_rad_a_tick',drop>0 and drop%500==0 and drop<=500*20*6,{'drop':drop})
    # Creative too: incrementRadiation asks nothing.
    reset(g,'creative',dose=300000);g.command(f'setblock {under} ntm:decon');time.sleep(.5)
    g.check('decon_works_in_creative',dose_drop(g)>0,state(g))
    # Two decontaminators whose boxes both reach a player standing on their border take twice as much.
    reset(g,'survival',dose=300000);g.command(f'setblock {under} ntm:decon');g.command('setblock 21 100 11 ntm:decon')
    g.command('tp @s 21.0 101 11.5');time.sleep(.5)
    two=dose_drop(g)
    g.check('two_decons_take_a_rad_a_tick',two>0 and two%1000==0,{'drop':two})
    g.command('setblock 21 100 11 minecraft:stone')
    # Off the box (the decontaminator one block beyond the reach) nothing changes.
    reset(g,'survival',dose=300000);g.command(f'setblock {under} ntm:decon');g.command('tp @s 22.4 101 11.5');time.sleep(.5)
    g.check('out_of_the_box_nothing',dose_drop(g)==0,state(g))
    # Glowing mycelium irradiates (600 ticks of the radiation potion); the decontaminator ends it, so once off it
    # the dose no longer grows.
    reset(g,'survival',dose=0);g.command(f'setblock {under} ntm:waste_mycelium');time.sleep(2.5)
    g.command(f'setblock {under} minecraft:stone');g.command('tp @s 25.5 101 11.5');time.sleep(.5)
    growing=-dose_drop(g,1.5)
    g.check('mycelium_irradiation_keeps_growing_the_dose',growing>0,{'growth':growing})
    g.command('setblock 25 100 11 ntm:decon');time.sleep(1.5);g.command('setblock 25 100 11 minecraft:stone');time.sleep(.5)
    after=-dose_drop(g,2.0)
    g.check('decon_ends_the_irradiation',after==0,{'growth_after':after})
    g.command(f'setblock {under} minecraft:stone');reset(g,'creative')


FAMILY_RUNS[:]=[('energy',energy),('medicine',medicine),('foods',foods),('kits',kits),('filters',other_filters),('instruments',instruments),('blocks',on_blocks),('protection',protection),('masks',masks),('decon',decon)]


if __name__=='__main__':
    g=Game(sys.argv[1],'items-'+(sys.argv[2] if len(sys.argv)>2 else 'setup'));mode=sys.argv[2] if len(sys.argv)>2 else 'setup';g.evidence['phase']=mode
    {'setup':setup,'capture-state':capture,'check-state':reopened}[mode](g)
    g.evidence['ok']=True;g.save()

