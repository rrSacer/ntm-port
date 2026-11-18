//! Server integration tests for `crate::annihilator`: WAL pool persistence across
//! all save/crash cuts, rollback on commit refusal, 3 fluid ports, chimney/core
//! chunk radiation, and slot rules.
use super::*;
use crate::annihilator::{BLOCK, DEFAULT_POOL, FLUID_ID, INPUT, MONITOR, OUTPUTS, PAYOUT, PoolKey, Pools, REQUEST, SLOTS};
use crate::server::container_bridge::{Slots, Stack};

const AT: Pos = [4, 70, 0];
const STATE: &str = "ntm:machine_annihilator[facing=south,honey_level=0]";

fn place_annihilator(g: &mut Game) {
    g.place(AT, STATE);
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), AT), (vec![None; SLOTS], vec![0]));
    g.run(2);
}

fn put(g: &mut Game, slot: usize, stack: Option<Stack>) {
    g.world
        .borrow_mut()
        .engine
        .get_mut(&(OW.to_string(), AT))
        .unwrap()
        .0[slot] = stack;
}

fn id_stack(fluid: &str) -> Stack {
    Stack {
        id: crate::server::native_server::IDENTIFIER.into(),
        count: 1,
        damage: 0,
        custom: Some(format!(r#"{{ntm_fluid:"{fluid}"}}"#)),
    }
}

fn saved_pools(w: &World) -> Pools {
    w.kv.map
        .get(crate::persist::ANNIHILATOR_KEY)
        .map(|b| serde_json::from_slice(b).unwrap())
        .unwrap_or_default()
}

fn reopen_from_snapshots(kv: BTreeMap<String, Vec<u8>>, saved_be: Option<String>, slots: Slots) -> Game {
    let mut w = World::new();
    w.kv.map = kv;
    w.set(OW, AT, STATE);
    w.engine.insert((OW.to_string(), AT), (slots, vec![0]));
    let rc = Rc::new(RefCell::new(w));
    let mut h = Game::new(rc);
    if let Some(json) = saved_be {
        assert!(h.server.native_adopt(OW, AT, &json));
    }
    h
}

#[test]
fn save_cuts_preserve_annihilator_pools_without_loss_or_duplication() {
    let mut g = game();
    place_annihilator(&mut g);

    // Phase 1: burn 5 cobblestone and confirm a waited save -> disk KV has 5, BE pending pruned.
    put(&mut g, INPUT, Some(Stack::new("minecraft:cobblestone", 5)));
    g.run(1);
    g.server.native_world_saving(&mut g.sh);
    g.server.native_world_saved(&mut g.sh, true);
    let base_kv = g.world.borrow().kv.map.clone();
    let base_be = g.world.borrow().saved[&(OW.to_string(), AT)].clone();
    assert_eq!(saved_pools(&g.world.borrow()).get(DEFAULT_POOL, &PoolKey::Item("minecraft:cobblestone".into())), 5);

    // Phase 2: burn 7 more cobblestone after the confirmed save.
    put(&mut g, INPUT, Some(Stack::new("minecraft:cobblestone", 7)));
    g.run(1);
    let after_be = g.world.borrow().saved[&(OW.to_string(), AT)].clone();

    // Cut A: `kill` / `save_begin` (neither chunk BE nor KV updated since Phase 1): returns exact base (5).
    {
        let mut h = reopen_from_snapshots(base_kv.clone(), Some(base_be.clone()), vec![None; SLOTS]);
        put(&mut h, MONITOR, Some(Stack::new("minecraft:cobblestone", 1)));
        h.run(1);
        assert_eq!(h.world.borrow().engine[&(OW.to_string(), AT)].1, vec![5]);
    }

    // Cut B: `chunks_written` (chunk BE has Phase 2 `pending`, KV still at Phase 1):
    // `native_adopt` replays `pending` once -> exact 12, and double-adopt / re-tick does NOT duplicate!
    {
        let mut h = reopen_from_snapshots(base_kv.clone(), Some(after_be.clone()), vec![None; SLOTS]);
        // Re-adopting the same BE is a no-op (`contains_key`).
        assert!(h.server.native_adopt(OW, AT, &after_be));
        put(&mut h, MONITOR, Some(Stack::new("minecraft:cobblestone", 1)));
        h.run(1);
        assert_eq!(h.world.borrow().engine[&(OW.to_string(), AT)].1, vec![12]);
    }

    // Cut C: `kv_committed` (`wait = false` save BEFORE chunks flush):
    // Even if a tick mutates `ready.annihilator` between `native_world_saving` and `native_world_saved`,
    // the save uses `taken_annihilator` (12), while `ANNIHILATOR_KEY` stays at Phase 1 = 5 until writes finish.
    {
        g.server.native_world_saving(&mut g.sh);
        put(&mut g, INPUT, Some(Stack::new("minecraft:cobblestone", 3)));
        g.run(1);
        g.server.native_world_saved(&mut g.sh, false);
        let kv_mid = g.world.borrow().kv.map.clone();
        let mut h_kill = reopen_from_snapshots(kv_mid.clone(), Some(base_be.clone()), vec![None; SLOTS]);
        put(&mut h_kill, MONITOR, Some(Stack::new("minecraft:cobblestone", 1)));
        h_kill.run(1);
        assert_eq!(h_kill.world.borrow().engine[&(OW.to_string(), AT)].1, vec![5]);

        let mut h_chunk = reopen_from_snapshots(kv_mid, Some(after_be), vec![None; SLOTS]);
        put(&mut h_chunk, MONITOR, Some(Stack::new("minecraft:cobblestone", 1)));
        h_chunk.run(1);
        assert_eq!(h_chunk.world.borrow().engine[&(OW.to_string(), AT)].1, vec![12]);
    }

    // Cut D: after `native_writes_finished`, `world_changed()` sees `annihilator_changed()` and `keep_world` writes 12 (the taken snapshot, not 15).
    g.server.native_writes_finished();
    assert!(g.server.world_changed());
    g.run(1);
    assert_eq!(saved_pools(&g.world.borrow()).get(DEFAULT_POOL, &PoolKey::Item("minecraft:cobblestone".into())), 12);

    // Cut E: failed `commit_machines` rolls back `ready.annihilator` and `chunk_rad_pending`.
    put(&mut g, INPUT, Some(Stack::new("ntm:ingot_uranium", 4)));
    put(&mut g, MONITOR, Some(Stack::new("ntm:ingot_uranium", 1)));
    g.world.borrow_mut().refuse_commits = true;
    g.run(1);
    g.world.borrow_mut().refuse_commits = false;
    // Remove uranium before the retry tick so we observe the pre-retry state cleanly.
    put(&mut g, INPUT, None);
    g.run(1);
    assert_eq!(g.world.borrow().engine[&(OW.to_string(), AT)].1, vec![0]);
}

#[test]
fn fluid_ports_accept_only_from_three_front_cells_and_require_identifier() {
    // Facing south (`f = [0,0,1]`, `fwd = [0,0,-1]`, `right = [1,0,0]`):
    // ports are `core + [0,0,4]`, `core + [-1,0,3]`, `core + [1,0,3]`.
    let duct_state = "ntm:fluid_duct_neo";
    let setup = |duct_pos: Pos, with_id: bool| {
        let mut g = game();
        place_annihilator(&mut g);
        if with_id {
            put(&mut g, FLUID_ID, Some(id_stack("water")));
        }
        // Place a creative/full barrel next to the duct so water flows through it.
        let barrel_pos = [duct_pos[0], duct_pos[1] - 1, duct_pos[2]];
        assert_ne!(duct_pos, AT);
        assert_ne!(barrel_pos, AT);
        g.place(duct_pos, duct_state);
        g.place(barrel_pos, "ntm:machine_fluidtank[facing=north,honey_level=0]");
        g.world
            .borrow_mut()
            .engine
            .insert((OW.to_string(), barrel_pos), (vec![None; 6], vec![0; 6]));
        g.run(1);
        if let Data::Duct(d) = &mut g.server.ready().unwrap().world.machines.get_mut(&(OW.to_string(), duct_pos)).unwrap().data {
            d.fluid = "water".into();
        }
        if let Data::FluidStore(f) = &mut g.server.ready().unwrap().world.machines.get_mut(&(OW.to_string(), barrel_pos)).unwrap().data {
            f.tank.set_type("water");
            f.tank.mb = 10_000;
            f.mode = crate::fluidbox::Mode::Output;
        }
        g.run(6);
        put(&mut g, MONITOR, Some(id_stack("water")));
        g.run(1);
        g.world.borrow().engine[&(OW.to_string(), AT)].1[0]
    };

    // Valid port [0, 0, -4] in placer frame -> [4, 70, 4] in world frame (`fwd = [0,0,-1]`).
    assert_eq!(setup([4, 70, 5], false), 0, "untyped tank rejects fluid");
    assert!(setup([4, 70, 5], true) > 0, "front port [0,0,-4] (+z) accepts water");
    assert!(setup([2, 70, 3], true) > 0, "left port [-1,0,-3] -> duct at x-1 accepts water");
    assert!(setup([6, 70, 3], true) > 0, "right port [1,0,-3] -> duct at x+1 accepts water");
    // Back cell (`[0, 0, 4]` -> `[4, 70, -5]`) and core side (`[5, 70, 0]`) are NOT fluid ports.
    assert_eq!(setup([4, 70, -5], true), 0, "back cell is not a fluid port");
    assert_eq!(setup([6, 70, 0], true), 0, "core side is not a fluid port");
    assert_eq!(setup([4, 69, 0], true), 0, "below core is not a fluid port");
    assert_eq!(setup([5, 70, 4], true), 0, "beside front port is not open sideways");
}

#[test]
fn radiation_goes_to_chimney_for_items_and_core_for_fluids() {
    let mut g = game();
    // Place at [24, 70, 24] facing south: core in chunk [1, 1], chimney at [24, 79, 21] (still chunk [1, 1]),
    // OR place at [24, 70, 17] facing south -> core in chunk [1, 1], chimney at [24, 79, 14] in chunk [1, 0]!
    let core = [24, 70, 17];
    g.place(core, STATE);
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), core), (vec![None; SLOTS], vec![0]));
    g.run(2);

    g.server.native_chunk_radiation_loaded(OW, [1, 1]);
    g.server.native_chunk_radiation_loaded(OW, [1, 0]);

    // Item uranium: chimney is [24, 79, 14] -> chunk [1, 0].
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), core)).unwrap().0[INPUT] =
        Some(Stack::new("ntm:ingot_uranium", 16));
    g.run(1);
    let chimney_rad = g.server.native_chunk_radiation_raw(OW, [1, 0]).unwrap_or(0.0);
    let core_rad = g.server.native_chunk_radiation_raw(OW, [1, 1]).unwrap_or(0.0);
    assert!((chimney_rad - 1.75).abs() < 1e-4, "chimney={chimney_rad}");
    assert_eq!(core_rad, 0.0, "item radiation does not hit core");

    // Radioactive fluid (`sas3`, vent_radiation = 1.0): hits core chunk [1, 1] (`mb * 2 * 1.0`).
    {
        let Data::Annihilator(a) = &mut g.server.ready().unwrap().world.machines.get_mut(&(OW.to_string(), core)).unwrap().data else { panic!() };
        a.tank.set_type("sas3");
        a.tank.mb = 25;
    }
    g.run(1);
    let core_rad2 = g.server.native_chunk_radiation_raw(OW, [1, 1]).unwrap_or(0.0);
    assert!((core_rad2 - 50.0).abs() < 1e-4, "core_rad2={core_rad2}");
}

#[test]
fn native_slot_filters_target_fluid_id_first_on_shift_click() {
    let filters = crate::native::slot_filters(BLOCK);
    assert_eq!(filters.len(), SLOTS);
    assert!(filters[INPUT].quick_reverse);
    assert!(filters[FLUID_ID].quick_reverse);
    assert_eq!(
        filters[FLUID_ID].accept,
        Some(vec![crate::server::native_server::IDENTIFIER.into()])
    );
    for out in OUTPUTS {
        assert!(filters[out].output);
    }
    assert!(filters[PAYOUT].output);
    assert_eq!(filters[MONITOR].quick_move, Some(Vec::new()));
    assert_eq!(filters[REQUEST].quick_move, Some(Vec::new()));
    assert_eq!(filters[PAYOUT].quick_move, Some(Vec::new()));
    let empty = vec![None; SLOTS];
    assert!(!crate::native::slot_allows(
        BLOCK,
        &empty,
        FLUID_ID,
        &Stack::new("minecraft:stone", 1),
        crate::native::SlotAction::Insert,
        crate::native::SlotOrigin::Manual,
    ));
    assert!(crate::native::slot_allows(
        BLOCK,
        &empty,
        FLUID_ID,
        &id_stack("water"),
        crate::native::SlotAction::Insert,
        crate::native::SlotOrigin::Manual,
    ));
}
