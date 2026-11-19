//! Items in machines on the engine's slots (`container_bridge`, patch
//! `ntm-visual`): no dupe, no loss. The fake world keeps each machine's
//! engine slots and values in `World::engine`, saved with the world (a
//! test's autosave clones them), apart from the mod's own save: what a
//! crash between the two does is what these tests are about. Shift-clicks
//! are what they leave in the slots: the engine moves the item, the bridge
//! sees the slot change.
use super::*;
use crate::server::container_bridge::{Slots, Stack};

const AT: Pos = [4, 70, 0];

/// The block at `at` with `n` engine slots, as the engine places it.
fn engine_block(g: &mut Game, at: Pos, state: &str, n: usize) {
    g.place(at, state);
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; n], Vec::new()));
    g.run(2);
}

fn slots(g: &Game, at: Pos) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), at)].0.clone()
}

/// A player's shift-click into slot `i` (the engine moved it).
fn put(g: &mut Game, at: Pos, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = s;
}

fn count(s: &Slots, id: &str) -> u32 {
    s.iter().flatten().filter(|x| x.id == id).map(|x| x.count).sum()
}

/// The engine breaks the block: its slots spill (as `dropped` lines), then
/// the mod hears of it.
fn engine_break(g: &mut Game, at: Pos) {
    let (s, _) = g.world.borrow_mut().engine.remove(&(OW.to_string(), at)).unwrap();
    {
        let mut w = g.world.borrow_mut();
        for x in s.into_iter().flatten() {
            w.dropped.push(format!("engine {} {}", x.id, x.count));
        }
        w.set(OW, at, "minecraft:air");
    }
    g.break_block(at);
    g.run(60);
}

fn battery(g: &mut Game, bat: Pos) {
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(2);
    let r = g.ready();
    if let Some(Data::Battery(b)) = r.world.machines.get_mut(&(OW.to_string(), bat)).map(|m| &mut m.data) {
        b.power = b.max;
        b.mode = crate::machine::BatteryMode::Output;
    }
}

/// The shredder grinds while the player takes the dust out at odd times:
/// ingots left + dust (in the slots and taken) is always what went in.
#[test]
fn a_shredder_conserves_items_while_the_player_takes_its_output() {
    let mut g = game();
    battery(&mut g, [3, 70, 0]);
    engine_block(&mut g, AT, "ntm:machine_shredder[facing=north,honey_level=0]", 30);
    put(&mut g, AT, 0, Some(Stack::new("minecraft:iron_ingot", 20)));
    put(&mut g, AT, 27, Some(Stack::new("ntm:blades_steel", 1)));
    put(&mut g, AT, 28, Some(Stack::new("ntm:blades_desh", 1)));
    let mut taken = 0;
    for t in 0..1500 {
        g.run(1);
        let s = slots(&g, AT);
        assert_eq!(count(&s, "minecraft:iron_ingot") + count(&s, "ntm:powder_iron") + taken, 20, "tick {t}: {s:?}");
        if t % 37 == 5 {
            // The player takes the output pile (the engine empties the slot).
            let s = slots(&g, AT);
            if let Some(i) = (9..27).find(|&i| s[i].as_ref().is_some_and(|x| x.id == "ntm:powder_iron")) {
                taken += s[i].as_ref().unwrap().count;
                put(&mut g, AT, i, None);
            }
        }
    }
    let s = slots(&g, AT);
    assert_eq!(count(&s, "minecraft:iron_ingot"), 0, "não terminou");
    assert_eq!(count(&s, "ntm:powder_iron") + taken, 20);
    // The blade's wear is in the engine's slot.
    assert_eq!(s[27].as_ref().map(|b| b.damage), Some(20));
}

/// Breaking a machine whose slots the engine spills: the mod drops none of
/// them again.
#[test]
fn breaking_a_slotted_machine_drops_its_items_once() {
    let mut g = game();
    engine_block(&mut g, AT, "ntm:machine_shredder[facing=north,honey_level=0]", 30);
    put(&mut g, AT, 0, Some(Stack::new("minecraft:iron_ingot", 5)));
    put(&mut g, AT, 27, Some(Stack::new("ntm:blades_steel", 1)));
    g.run(3);
    engine_break(&mut g, AT);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped.len(), 2, "{dropped:?}");
    assert!(dropped.iter().all(|d| d.starts_with("engine ")), "o mod soltou de novo: {dropped:?}");
}

/// Breaking a crucible holding molten metal: the engine spills its slots,
/// the metal (the mod's) comes out as scraps; nothing is lost.
#[test]
fn breaking_a_crucible_keeps_its_molten_metal_as_scraps() {
    let mut g = game();
    let core = [0, 71, 0];
    engine_block(&mut g, core, "ntm:machine_crucible[facing=north,honey_level=0]", 10);
    {
        let r = g.ready();
        let Some(Data::Crucible(c)) = r.world.machines.get_mut(&(OW.to_string(), core)).map(|m| &mut m.data) else { panic!() };
        c.waste_stack = vec![("iron".into(), 144)];
    }
    g.run(3);
    engine_break(&mut g, core);
    let dropped = g.world.borrow().dropped.clone();
    assert!(dropped.iter().any(|d| d.ends_with("ntm:scraps__iron 1 {ntm_amount:144}")), "o metal sumiu: {dropped:?}");
}

/// The world's save holds a machine's slots and its state together (its
/// block entity's): an ingot melted before the save comes back as metal,
/// not as an ingot too.
#[test]
fn a_reopened_world_neither_dupes_nor_loses_molten_metal() {
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone());
    g.run(10);
    let core = [0, 71, 0];
    engine_block(&mut g, core, "ntm:machine_crucible[facing=north,honey_level=0]", 10);
    put(&mut g, core, 1, Some(Stack::new("minecraft:iron_ingot", 1)));
    g.run(3);
    for _ in 0..20 {
        let r = g.ready();
        if let Some(Data::Crucible(c)) = r.world.machines.get_mut(&(OW.to_string(), core)).map(|m| &mut m.data) {
            c.heat = 100_000;
        }
        g.run(1);
    }
    let molten = |g: &mut Game| match g.machine(core).data {
        Data::Crucible(c) => c.waste_stack.iter().map(|m| m.1).sum::<u32>(),
        _ => 0,
    };
    assert_eq!(molten(&mut g), 72, "não fundiu");
    let autosave = w.borrow().clone();
    drop(g);
    *w.borrow_mut() = autosave;
    let mut g = Game::new(w.clone());
    g.run(40);
    let total = molten(&mut g) + 72 * count(&slots(&g, core), "minecraft:iron_ingot");
    assert_eq!(total, 72, "ferro depois de reabrir: {total} quanta (era 72)");
}

/// From slot to slot through the metal: ingots melt in a crucible, pour
/// into an ingot mold and come out cast, while the player takes the casts.
/// Every tick, the iron in ingots, in the crucible, in the mold and taken
/// is what went in.
#[test]
fn iron_is_conserved_from_crucible_to_mold() {
    let mut g = game();
    let core = [0, 71, 0];
    // No alloy: everything goes to the waste stream, south of the core.
    let mold = [0, 70, 2];
    engine_block(&mut g, core, "ntm:machine_crucible[facing=north,honey_level=0]", 10);
    engine_block(&mut g, mold, "ntm:foundry_mold[facing=north,honey_level=0]", 2);
    put(&mut g, core, 1, Some(Stack::new("minecraft:iron_ingot", 5)));
    put(&mut g, mold, 0, Some(Stack::new("ntm:mold__ingot", 1)));
    let mut taken = 0;
    for t in 0..2000 {
        {
            let r = g.ready();
            if let Some(Data::Crucible(c)) = r.world.machines.get_mut(&(OW.to_string(), core)).map(|m| &mut m.data) {
                c.heat = 100_000;
            }
        }
        g.run(1);
        let molten = match g.machine(core).data {
            Data::Crucible(c) => c.waste_stack.iter().map(|m| m.1).sum::<u32>(),
            _ => 0,
        };
        let in_mold = match g.machine(mold).data {
            Data::Mold(c) => c.melt.amount,
            _ => 0,
        };
        let (cs, ms) = (slots(&g, core), slots(&g, mold));
        let ingots = count(&cs, "minecraft:iron_ingot") + count(&ms, "minecraft:iron_ingot") + taken;
        assert_eq!(ingots * 72 + molten + in_mold, 5 * 72, "tick {t}: lingotes {ingots}, fundido {molten}, no molde {in_mold}");
        // The player takes a cast when there is one.
        if count(&ms, "minecraft:iron_ingot") > 0 && t % 3 == 0 {
            taken += count(&ms, "minecraft:iron_ingot");
            put(&mut g, mold, 1, None);
        }
    }
    assert_eq!(taken, 5, "não fundiu e moldou os 5");
}

/// A centrifuge on the engine's slots: its input slot feeds it, its outputs
/// spread over the four output slots, and a player taking one of them takes
/// it from the machine too.
#[test]
fn a_centrifuge_shows_its_outputs_in_four_slots() {
    let mut g = game();
    let state = "ntm:machine_centrifuge[facing=north,honey_level=0]";
    engine_block(&mut g, AT, state, 5);
    let r = &crate::crafter::Station::Centrifuge.recipes()[0];
    let input = r.items_in[0].ids[0].clone();
    put(&mut g, AT, 0, Some(Stack::new(&input, 2)));
    g.run(2);
    {
        let r = g.ready();
        if let Some(Data::Crafter(c)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) {
            c.power = 10_000_000;
        }
    }
    g.run(2 * 200 + 10);
    let s = slots(&g, AT);
    assert_eq!(s[0], None, "the input was used");
    for (item, n, _) in &r.items_out {
        assert!(count(&s[1..5].to_vec(), item) >= 2 * n, "{item}: {s:?}");
    }
    // The player takes the first output: the machine has it no more.
    let taken = s[1].clone().unwrap();
    put(&mut g, AT, 1, None);
    g.run(2);
    let left = match g.ready().world.machines.get(&(OW.to_string(), AT)).map(|m| m.data.clone()) {
        Some(Data::Crafter(c)) => c.out.iter().filter(|o| o.0 == taken.id).map(|o| o.1).sum::<u32>(),
        _ => panic!("no centrifuge"),
    };
    assert_eq!(left + taken.count, count(&s, &taken.id), "{taken:?}");
}
