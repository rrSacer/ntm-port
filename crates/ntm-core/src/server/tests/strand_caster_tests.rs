//! Server tests for the continuous caster (`ntm:machine_strand_caster`, `TileEntityMachineStrandCaster.java`): a jet from
//! the rotary furnace on a funnel cell (F1, closes R9), the batch cast into the six outputs, the exact scrap of the
//! excess through the ledger (F5) and of a broken machine (D4), and its fluids.
use super::*;
use crate::fluidbox::Mode;
use crate::server::container_bridge::{Slots, Stack};
use crate::foundry::flow::Melt;
use crate::strand_caster::{BLOCK, StrandCaster, slot};

const ROTARY: Pos = [0, 70, 0];
/// Under the spout of the rotary furnace facing east: its funnel cell `(0, 70, -3)` is two over the core.
const CASTER: Pos = [0, 68, -3];

fn put_machine(g: &mut Game, at: Pos, state: &str, slots: usize) {
    g.place(at, state);
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; slots], Vec::new()));
    g.run(2);
}

fn caster(g: &mut Game, mold: &str) {
    put_machine(g, CASTER, &format!("{BLOCK}[facing=north,honey_level=0]"), slot::COUNT);
    // The funnel cell the jet strikes is a block of the multiblock.
    g.place([0, 70, -3], "minecraft:stone");
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), CASTER)).unwrap().0[slot::MOLD] = Some(Stack::new(mold, 1));
    g.run(2);
    edit_caster(g, |c| c.water.mb = 64_000);
}

fn rotary(g: &mut Game, steel: u32) {
    put_machine(g, ROTARY, &format!("{}[facing=east,honey_level=0]", crate::rotary_furnace::BLOCK), 5);
    let r = g.ready();
    let Some(Data::RotaryFurnace(f)) = r.world.machines.get_mut(&(OW.to_string(), ROTARY)).map(|m| &mut m.data) else { panic!() };
    f.output = Some(("steel".into(), steel));
}

fn edit_caster(g: &mut Game, f: impl FnOnce(&mut StrandCaster)) {
    let r = g.ready();
    let Some(Data::StrandCaster(c)) = r.world.machines.get_mut(&(OW.to_string(), CASTER)).map(|m| &mut m.data) else { panic!("no caster") };
    f(c);
}

fn caster_model(g: &mut Game) -> StrandCaster {
    match g.machine(CASTER).data {
        Data::StrandCaster(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn rotary_output(g: &mut Game) -> u32 {
    match g.machine(ROTARY).data {
        Data::RotaryFurnace(r) => r.output.map_or(0, |o| o.1),
        other => panic!("{other:?}"),
    }
}

fn slots_of(g: &Game, at: Pos) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), at)].0.clone()
}

fn outputs(g: &Game) -> u32 {
    slots_of(g, CASTER)[slot::OUT].iter().flatten().map(|s| s.count).sum()
}

/// R9: with F1 the rotary furnace over the funnel pours into the caster, which casts nine ingots as a batch. Every
/// quantum is in the furnace, the pool or an ingot (conservation, 72 a tick from the furnace).
#[test]
fn a_rotary_jet_on_a_funnel_cell_fills_the_caster_and_it_casts_the_batch() {
    let mut g = game();
    caster(&mut g, "ntm:mold__ingot");
    rotary(&mut g, 720);
    g.run(1);
    assert_eq!(rotary_output(&mut g), 720 - 72, "it pours 72 a tick");
    for _ in 0..40 {
        g.run(1);
        let (furnace, pool, ingots) = (rotary_output(&mut g), caster_model(&mut g).melt.amount, outputs(&g));
        assert_eq!(furnace + pool + ingots * 72, 720, "conservation");
    }
    // 648 reach the pool (nine molds), the batch casts, and the last 72 become the tenth ingot after 200 idle ticks.
    assert_eq!(rotary_output(&mut g), 0);
    assert_eq!(outputs(&g), 9);
    let c = caster_model(&mut g);
    assert_eq!(c.steam.mb, 9 * 360, "five water per quantum: the spent steam made");
    assert_eq!(c.water.mb, 64_000 - 9 * 360);
    assert_eq!(slots_of(&g, CASTER)[1], Some(Stack::new("ntm:ingot_steel", 9)));
    g.run(260);
    assert_eq!(outputs(&g), 10, "the leftover ingot is cast after 200 idle ticks");
    assert_eq!(caster_model(&mut g).melt.amount, 0);
}

/// F1: a jet on a cell that is not a funnel cell, or with no mold, parks: nothing pours, nothing is lost.
#[test]
fn a_jet_with_no_mold_or_off_the_funnel_parks_without_loss() {
    let mut g = game();
    caster(&mut g, "ntm:mold__ingot");
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), CASTER)).unwrap().0[slot::MOLD] = None;
    rotary(&mut g, 144);
    g.run(30);
    assert_eq!(rotary_output(&mut g), 144, "no mold: it takes none");
    assert_eq!(caster_model(&mut g).melt.amount, 0);
}

/// F5: the mold is swapped for a lighter one while the pool is full; the excess is one exact scrap above the
/// machine (`y + 2`), published by the world's ledger with the model that kept the rest.
#[test]
fn the_excess_after_a_lighter_mold_is_dropped_as_one_exact_scrap() {
    let mut g = game();
    caster(&mut g, "ntm:mold__ingot");
    edit_caster(&mut g, |c| {
        c.melt.mat = Some("steel".into());
        c.melt.amount = 700;
        c.water.mb = 0;
    });
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), CASTER)).unwrap().0[slot::MOLD] = Some(Stack::new("ntm:mold__nugget", 1));
    g.run(5);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert!(dropped[0].ends_with("0 70 -3 ntm:scraps__steel 1 {ntm_amount:620}"), "{dropped:?}");
    assert_eq!(caster_model(&mut g).melt.amount, 80);
    g.run(20);
    assert_eq!(g.world.borrow().dropped.len(), 1, "once");
}

/// D4 on the caster: breaking it drops the engine's slots (the mold and the ingots) and one scrap with every quantum
/// of the pool, never the molten metal lost to the rounding of an ingot.
#[test]
fn breaking_the_caster_drops_its_items_and_one_exact_scrap() {
    let mut g = game();
    caster(&mut g, "ntm:mold__ingot");
    edit_caster(&mut g, |c| {
        c.melt.mat = Some("steel".into());
        c.melt.amount = 100;
        c.last_progress = u64::MAX / 2; // no idle flush: it is still a pool when it breaks
    });
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), CASTER)).unwrap().0[slot::OUT.start] = Some(Stack::new("ntm:ingot_steel", 3));
    let (s, _) = g.world.borrow_mut().engine.remove(&(OW.to_string(), CASTER)).unwrap();
    {
        let mut w = g.world.borrow_mut();
        for x in s.into_iter().flatten() {
            w.dropped.push(format!("engine {} {}", x.id, x.count));
        }
        w.set(OW, CASTER, "minecraft:air");
    }
    g.break_block(CASTER);
    g.run(60);
    let dropped = g.world.borrow().dropped.clone();
    assert!(dropped.iter().any(|d| d == "engine ntm:mold__ingot 1"), "{dropped:?}");
    assert!(dropped.iter().any(|d| d == "engine ntm:ingot_steel 3"), "{dropped:?}");
    assert!(dropped.iter().any(|d| d.ends_with("ntm:scraps__steel 1 {ntm_amount:100}")), "{dropped:?}");
    assert_eq!(dropped.len(), 3, "{dropped:?}");
}

/// Its state persists in the block entity and the pool and the tanks come back after a reopening.
#[test]
fn the_pool_and_the_tanks_are_kept_across_a_save() {
    let mut g = game();
    caster(&mut g, "ntm:mold__ingot");
    edit_caster(&mut g, |c| {
        c.melt.mat = Some("steel".into());
        c.melt.amount = 144;
        c.last_progress = u64::MAX / 2;
        c.steam.mb = 777;
    });
    g.run(3);
    let m = caster_model(&mut g);
    let saved = crate::server::container_bridge::Slotted::save_state(&m);
    let mut back = StrandCaster::default();
    crate::server::container_bridge::Slotted::load_state(&mut back, &saved);
    assert_eq!((back.melt.clone(), back.steam.mb, back.water.mb), (m.melt, 777, m.water.mb));
}

/// Water comes in by a duct on a port cell (`getPorts`) and the spent steam goes out by another.
#[test]
fn water_in_and_spent_steam_out_through_the_port_cells() {
    let mut g = game();
    put_machine(&mut g, CASTER, &format!("{BLOCK}[facing=north,honey_level=0]"), slot::COUNT);
    let f = registry::facing_dir("north").unwrap();
    let cells = crate::server::container_bridge::fluid_proxy_cells(BLOCK, CASTER, Some("north"));
    assert_eq!(cells.len(), 4);
    assert_eq!(cells[0], crate::server::container_bridge::world_cell(CASTER, f, [-1, 0, 1]));
    // north: dir = (0,0,-1), rot = dir.getRotation(UP) = (1,0,0): core + rot - dir, core - dir, and 4 further back.
    assert_eq!(cells, vec![[1, 68, -2], [0, 68, -2], [1, 68, 2], [0, 68, 2]]);
    // A duct next to the first port cell carries water in from a barrel.
    let duct_at = [2, 68, -2];
    g.place(duct_at, "ntm:fluid_duct_neo");
    g.run(2);
    g.type_duct(duct_at, "water");
    let src = [3, 68, -2];
    g.place(src, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    {
        let r = g.ready();
        let Some(Data::FluidStore(s)) = r.world.machines.get_mut(&(OW.to_string(), src)).map(|m| &mut m.data) else { panic!() };
        s.tank.fill("water", 10_000);
        s.mode = Mode::Output;
    }
    g.run(30);
    assert!(caster_model(&mut g).water.mb > 0, "water reached the caster");
}

#[test]
fn the_slot_rules_of_the_caster() {
    use crate::native::{SlotAction, SlotOrigin, slot_allows};
    let mold = crate::server::container_bridge::Stack::new("ntm:mold__ingot", 1);
    let ingot = crate::server::container_bridge::Stack::new("ntm:ingot_steel", 1);
    let slots: Vec<Option<Stack>> = vec![None; slot::COUNT];
    let ok = |s: usize, i: &Stack, a, o| slot_allows(BLOCK, &slots, s, i, a, o);
    assert!(ok(0, &mold, SlotAction::Insert, SlotOrigin::Manual));
    assert!(!ok(0, &ingot, SlotAction::Insert, SlotOrigin::Manual), "only molds go in slot 0");
    assert!(!ok(1, &mold, SlotAction::Insert, SlotOrigin::Manual), "an output takes nothing");
    assert!(!ok(0, &mold, SlotAction::Insert, SlotOrigin::Automation), "a funnel never sees the mold slot");
    assert!(!ok(1, &ingot, SlotAction::Insert, SlotOrigin::Automation));
    assert!(ok(1, &ingot, SlotAction::Extract, SlotOrigin::Automation), "a hopper takes the casts");
    assert!(!ok(0, &mold, SlotAction::Extract, SlotOrigin::Automation), "and never the mold");
    assert!(ok(0, &mold, SlotAction::Extract, SlotOrigin::Manual));
    let c = crate::native::container(BLOCK).unwrap();
    assert_eq!((c.slots, c.inputs.clone(), c.outputs.clone()), (7, vec![], vec![1, 2, 3, 4, 5, 6]));
}

fn engine_slots(g: &Game) -> Slots {
    slots_of(g, CASTER)
}

/// The review's P1-1: the caster's record keeps no slots (they are the block entity's). The excess a lighter mold leaves
/// is a critical effect: it journals the machine; a chunk cut older than that journal reopens with the model from the
/// journal, which has no slots, and the bridge took them as the truth and wrote empty slots over the block entity's: the
/// mold and up to six stacks gone. The caster takes the block entity's slots, the scrap is dropped once and the pool is
/// the journal's (F5 and the restoration in one scenario).
#[test]
fn a_caster_restored_from_the_journal_keeps_its_mold_and_outputs_and_drops_the_excess_once() {
    let mut g = game();
    caster(&mut g, "ntm:mold__ingot");
    {
        let mut w = g.world.borrow_mut();
        let slots = &mut w.engine.get_mut(&(OW.to_string(), CASTER)).unwrap().0;
        slots[slot::OUT.start] = Some(Stack::new("ntm:ingot_steel", 3));
        slots[slot::OUT.start + 4] = Some(Stack::new("ntm:ingot_steel", 60));
    }
    edit_caster(&mut g, |c| {
        c.melt.mat = Some("steel".into());
        c.melt.amount = 700;
        c.water.mb = 0; // nothing casts
        c.last_progress = u64::MAX / 2;
    });
    g.run(1);
    let key = (OW.to_string(), CASTER);
    // The chunk as last saved: the ingot mold and the outputs in the block entity, the pool of 700.
    let (old_json, old_slots) = (g.world.borrow().saved[&key].clone(), engine_slots(&g));
    assert_eq!(old_slots[slot::MOLD], Some(Stack::new("ntm:mold__ingot", 1)));
    assert_eq!(old_slots[slot::OUT.start], Some(Stack::new("ntm:ingot_steel", 3)));
    // A lighter mold: the pool keeps 80, 620 go out as one scrap, journaled with the model.
    g.world.borrow_mut().engine.get_mut(&key).unwrap().0[slot::MOLD] = Some(Stack::new("ntm:mold__nugget", 1));
    g.run(1);
    assert_eq!(caster_model(&mut g).melt.amount, 80);
    assert!(g.server.ready.world.survivors.contains_key(&key), "the excess is journaled with the machine");
    // The journal is newer than the chunk's record, which still has the older model and the older slots.
    let earlier = g.server.ready.world.kept_as_an_earlier_version(0);
    let world = Rc::new(RefCell::new(g.world.borrow().clone()));
    world.borrow_mut().kv.set(crate::persist::KEY, &earlier).unwrap();
    world.borrow_mut().saved.insert(key.clone(), old_json);
    world.borrow_mut().engine.get_mut(&key).unwrap().0 = old_slots.clone();
    world.borrow_mut().dropped.clear();
    let mut reopened = Game::new(world);
    assert!(reopened.server.ready.restored.contains(&key), "restored from the journal");
    reopened.run(8);
    assert_eq!(engine_slots(&reopened), old_slots, "the mold and the outputs stay");
    assert_eq!(caster_model(&mut reopened).slots, old_slots, "the model takes them from the block entity");
    assert_eq!(caster_model(&mut reopened).melt.amount, 80, "the pool is the journal's");
    let dropped = reopened.world.borrow().dropped.clone();
    assert_eq!(dropped.len(), 1, "the excess is dropped once: {dropped:?}");
    assert!(dropped[0].ends_with("ntm:scraps__steel 1 {ntm_amount:620}"), "{dropped:?}");
}

/// The ports are `PortDef`s with a direction (`TileEntityMachineStrandCaster.getPorts`): a duct touching a port cell from
/// another side than its connection does not join the machine.
#[test]
fn a_duct_reaches_a_caster_port_only_from_the_connection_side() {
    let f = |g: &mut Game, duct_at: Pos, src: Pos| {
        g.place(duct_at, "ntm:fluid_duct_neo");
        g.run(2);
        g.type_duct(duct_at, "water");
        g.place(src, "ntm:barrel_steel[facing=north,honey_level=0]");
        g.run(2);
        let r = g.ready();
        let Some(Data::FluidStore(s)) = r.world.machines.get_mut(&(OW.to_string(), src)).map(|m| &mut m.data) else { panic!() };
        s.tank.fill("water", 10_000);
        s.mode = Mode::Output;
    };
    // North: dir = (0,0,-1), rot = (1,0,0). The third port cell is (1, y, core.z + 5) open to +x.
    let mut g = game();
    put_machine(&mut g, CASTER, &format!("{BLOCK}[facing=north,honey_level=0]"), slot::COUNT);
    f(&mut g, [1, 68, 3], [1, 68, 4]); // from the south of the cell: not its connection
    g.run(30);
    assert_eq!(caster_model(&mut g).water.mb, 0, "the wrong side takes nothing");
    let mut g = game();
    put_machine(&mut g, CASTER, &format!("{BLOCK}[facing=north,honey_level=0]"), slot::COUNT);
    f(&mut g, [2, 68, 2], [3, 68, 2]); // from the east, the connection of that port
    g.run(30);
    assert!(caster_model(&mut g).water.mb > 0, "the connection side reaches it");
}

// ---- by hand (MachineStrandCaster.onBlockActivated) ----

fn hands(g: &mut Game, inventory: Vec<Option<Stack>>) {
    let inventory = inventory.into_iter().map(|s| s.map(|stack| HeldStack { encoded: format!("{stack:?}"), stack, plain: true })).collect();
    g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Inventory(0)), inventory, ..Default::default() });
}
fn inventory(g: &Game) -> Vec<Option<Stack>> {
    g.world.borrow().hands.as_ref().unwrap().inventory.iter().map(|s| s.as_ref().map(|h| h.stack.clone())).collect()
}
fn bare_caster(g: &mut Game) {
    put_machine(g, CASTER, &format!("{BLOCK}[facing=north,honey_level=0]"), slot::COUNT);
}
fn use_caster(g: &mut Game) -> bool {
    g.server.native_caster_use(&mut g.sh, OW, CASTER, CASTER, "p1")
}

#[test]
fn a_mold_in_hand_goes_into_the_empty_mold_slot_in_one_transfer() {
    let mut g = game();
    bare_caster(&mut g);
    let named = Stack { custom: Some("{named:1b}".into()), ..Stack::new("ntm:mold__ingot", 3) };
    hands(&mut g, vec![Some(named.clone()), None]);
    // Refused: nothing moves.
    g.world.borrow_mut().refuse_commits = true;
    assert!(use_caster(&mut g));
    assert_eq!(engine_slots(&g)[slot::MOLD], None);
    assert_eq!(inventory(&g)[0], Some(named.clone()));
    g.world.borrow_mut().refuse_commits = false;
    assert!(use_caster(&mut g));
    let one = Stack { count: 1, ..named.clone() };
    assert_eq!(engine_slots(&g)[slot::MOLD], Some(one.clone()), "one, with its data (a copy of the stack)");
    assert_eq!(caster_model(&mut g).slots[slot::MOLD], Some(one));
    assert_eq!(inventory(&g)[0], Some(Stack { count: 2, ..named.clone() }));
    // The slot taken, the click goes on (the screen opens).
    assert!(!use_caster(&mut g));
    assert_eq!(inventory(&g)[0].as_ref().map(|s| s.count), Some(2));
}

#[test]
fn a_shovel_takes_the_whole_pool_as_one_exact_scrap() {
    let mut g = game();
    bare_caster(&mut g);
    edit_caster(&mut g, |c| {
        c.melt = Melt { mat: Some("steel".into()), amount: 100 };
        c.last_progress = u64::MAX / 2;
    });
    hands(&mut g, vec![Some(Stack::new("minecraft:iron_shovel", 1)), None]);
    g.world.borrow_mut().refuse_commits = true;
    assert!(use_caster(&mut g));
    assert_eq!(caster_model(&mut g).melt.amount, 100, "refused: the pool stays");
    assert_eq!(inventory(&g)[1], None);
    g.world.borrow_mut().refuse_commits = false;
    assert!(use_caster(&mut g));
    assert_eq!(caster_model(&mut g).melt, Melt::default());
    let scrap = crate::foundry::scrap::stack("steel", 100);
    assert_eq!(inventory(&g)[1], Some(scrap), "one scrap with every quantum");
    assert!(g.world.borrow().dropped.is_empty());
    // An empty pool: the shovel's click is taken and nothing happens (the Java returns true).
    assert!(use_caster(&mut g));
    assert_eq!(inventory(&g)[1].as_ref().map(|s| s.count), Some(1));
}

#[test]
fn a_scrap_that_does_not_fit_in_the_inventory_is_thrown_once() {
    let mut g = game();
    bare_caster(&mut g);
    edit_caster(&mut g, |c| {
        c.melt = Melt { mat: Some("steel".into()), amount: 30 };
        c.last_progress = u64::MAX / 2;
    });
    hands(&mut g, vec![Some(Stack::new("minecraft:wooden_shovel", 1))]);
    assert!(use_caster(&mut g));
    assert_eq!(caster_model(&mut g).melt.amount, 0);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert_eq!(dropped[0], "player ntm:scraps__steel 1 {ntm_amount:30}");
}

#[test]
fn only_a_mold_or_a_shovel_is_a_use_of_the_caster() {
    let mut g = game();
    bare_caster(&mut g);
    edit_caster(&mut g, |c| c.melt = Melt { mat: Some("steel".into()), amount: 10 });
    hands(&mut g, vec![Some(Stack::new("minecraft:stick", 1)), None]);
    assert!(!use_caster(&mut g));
    hands(&mut g, vec![None, None]);
    assert!(!use_caster(&mut g), "an empty hand opens the screen");
    assert_eq!(caster_model(&mut g).melt.amount, 10);
    assert!(crate::server::caster_server::is_shovel("ntm:titanium_shovel") && !crate::server::caster_server::is_shovel("minecraft:iron_sword"));
}
