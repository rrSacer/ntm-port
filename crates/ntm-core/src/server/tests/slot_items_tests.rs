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
fn engine_break(g: &mut Game, at: Pos, state: &str) {
    let (s, _) = g.world.borrow_mut().engine.remove(&(OW.to_string(), at)).unwrap();
    {
        let mut w = g.world.borrow_mut();
        for x in s.into_iter().flatten() {
            w.dropped.push(format!("engine {} {}", x.id, x.count));
        }
        w.set(OW, at, "minecraft:air");
    }
    g.server.block_broken(OW, at, state);
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
    engine_break(&mut g, AT, "ntm:machine_shredder[facing=north,honey_level=0]");
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
    engine_break(&mut g, core, "ntm:machine_crucible[facing=north,honey_level=0]");
    let dropped = g.world.borrow().dropped.clone();
    assert!(dropped.iter().any(|d| d.contains("scraps__iron") && d.contains("count:2")), "o metal sumiu: {dropped:?}");
}

/// The engine's world (slots) and the mod's save are written at different
/// times: a crash between them rolls one back and not the other. An ingot
/// that melts after the world's autosave, and is in the mod's save as
/// metal, comes back as an ingot too.
#[test]
fn a_crash_between_the_saves_neither_dupes_nor_loses_molten_metal() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let core = [0, 71, 0];
    engine_block(&mut g, core, "ntm:machine_crucible[facing=north,honey_level=0]", 10);
    put(&mut g, core, 1, Some(Stack::new("minecraft:iron_ingot", 1)));
    g.run(3);
    // The world's autosave: the ingot still in its slot.
    let autosave = w.borrow().clone();
    {
        let r = g.ready();
        let Some(Data::Crucible(c)) = r.world.machines.get_mut(&(OW.to_string(), core)).map(|m| &mut m.data) else { panic!() };
        c.heat = 100_000;
    }
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
    // The mod's save has the metal.
    g.run(400);
    drop(g);
    *w.borrow_mut() = autosave; // the crash: the world from before the melt
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(40);
    let total = molten(&mut g) + 72 * count(&slots(&g, core), "minecraft:iron_ingot");
    assert_eq!(total, 72, "ferro depois da queda: {total} quanta (era 72)");
}

/// The other way round: the mod's save is the older one (from before the
/// melt), the world's the newer. The metal comes back from the world's
/// save with the empty slot: nothing is lost.
#[test]
fn a_crash_with_an_older_mod_save_loses_no_molten_metal() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let core = [0, 71, 0];
    engine_block(&mut g, core, "ntm:machine_crucible[facing=north,honey_level=0]", 10);
    put(&mut g, core, 1, Some(Stack::new("minecraft:iron_ingot", 1)));
    g.run(300);
    // The mod's last save: the ingot not melted yet.
    let old_save = kv.borrow().clone();
    for _ in 0..20 {
        let r = g.ready();
        if let Some(Data::Crucible(c)) = r.world.machines.get_mut(&(OW.to_string(), core)).map(|m| &mut m.data) {
            c.heat = 100_000;
        }
        g.run(1);
    }
    // The world's autosave: melted (the slot empty, the metal in the
    // block entity's values).
    let autosave = w.borrow().clone();
    drop(g);
    *w.borrow_mut() = autosave;
    *kv.borrow_mut() = old_save;
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(40);
    let molten = match g.machine(core).data {
        Data::Crucible(c) => c.waste_stack.iter().map(|m| m.1).sum::<u32>(),
        _ => 0,
    };
    let total = molten + 72 * count(&slots(&g, core), "minecraft:iron_ingot");
    assert_eq!(total, 72, "ferro depois da queda: {total} quanta (era 72)");
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

/// A bomb assembled through transactions before its block had engine slots:
/// on its first view with empty slots, its parts go to the slots, once each,
/// and the bomb keeps them (`Layout::Bomb`).
#[test]
fn a_bomb_assembled_before_the_slots_shows_its_parts_in_them() {
    for id in ["ntm:nuke_boy", "ntm:nuke_gadget", "ntm:nuke_man", "ntm:nuke_mike", "ntm:nuke_tsar", "ntm:nuke_n2", "ntm:nuke_fstbmb"] {
        let mut g = game();
        let state = format!("{id}[lit=false]");
        g.place(AT, &state);
        g.run(2);
        let kind = crate::bomb::bomb_type(id).unwrap();
        g.open(AT);
        for s in kind.slots {
            g.hold(s.item, 1);
            g.click(crate::bomb::actions::INSERT);
            for _ in 0..200 {
                if g.held().is_none() && g.server.txs().is_empty() {
                    break;
                }
                g.run(1);
            }
            assert_eq!(g.held(), None, "{id}: {} not taken", s.item);
        }
        // The patched engine: the block gets its (empty) slots.
        g.world.borrow_mut().engine.insert((OW.to_string(), AT), (vec![None; kind.slots.len()], Vec::new()));
        g.run(5);
        let slots = slots(&g, AT);
        let want: Vec<Option<Stack>> = kind.slots.iter().map(|s| Some(Stack::new(s.item, 1))).collect();
        assert_eq!(slots, want, "{id}");
        let parts = match g.ready().world.machines.get(&(OW.to_string(), AT)).map(|m| m.data.clone()) {
            Some(Data::Bomb(b)) => b.slots.iter().flatten().count(),
            _ => 0,
        };
        assert_eq!(parts, kind.slots.len(), "{id}: the bomb lost parts");
    }
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
