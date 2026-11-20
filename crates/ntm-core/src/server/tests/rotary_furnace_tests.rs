//! Server tests for the rotary furnace (`ntm:machine_rotary_furnace`,
//! `TileEntityMachineRotaryFurnace.java`): steam/spentsteam & fluid proxy ports,
//! fluid identifier typing, molten pouring (`rot * 3`, `y + 1`, fall 6),
//! persistence (`save_state`/`load_state` & journal), chunk-border pour, and
//! mandatory R8 (molten `output` lost on break).
use super::*;
use crate::fluidbox::Mode;
use crate::rotary_furnace::{BLOCK, RotaryFurnace, slot};
use crate::server::container_bridge::{Slots, Stack, world_cell};

const CORE: Pos = [0, 70, 0];

fn rotary(g: &mut Game, at: Pos, facing: &str) {
    let state = format!("{BLOCK}[facing={facing},honey_level=0]");
    g.place(at, &state);
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), at), (vec![None; slot::COUNT], Vec::new()));
    g.run(2);
}

fn mold(g: &mut Game, at: Pos, mold_item: &str) {
    g.place(at, "ntm:foundry_mold[facing=north,honey_level=0]");
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), at), (vec![Some(Stack::new(mold_item, 1)), None], Vec::new()));
    g.run(2);
}

fn basin(g: &mut Game, at: Pos) {
    g.place(at, "ntm:foundry_basin[facing=north,honey_level=0]");
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), at), (vec![Some(Stack::new("ntm:mold__block", 1)), None], Vec::new()));
    g.run(2);
}

fn barrel(g: &mut Game, at: Pos, fluid: &str, mb: u32, mode: Mode) {
    let blk = if mb > 16_000 { "ntm:machine_fluidtank[facing=north,honey_level=0]" } else { "ntm:barrel_steel[facing=north,honey_level=0]" };
    g.place(at, blk);
    g.run(2);
    let r = g.ready();
    let Some(Data::FluidStore(s)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) else {
        panic!("no barrel at {at:?}");
    };
    if mb > 0 {
        s.tank.fill(fluid, mb);
    } else if !fluid.is_empty() {
        s.tank.set_type(fluid);
    }
    s.mode = mode;
}

fn duct(g: &mut Game, at: Pos, fluid: &str) {
    g.place(at, "ntm:fluid_duct_neo");
    g.run(2);
    g.type_duct(at, fluid);
}

fn put(g: &mut Game, at: Pos, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = s;
}

fn slots(g: &Game, at: Pos) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), at)].0.clone()
}

fn furnace(g: &mut Game, at: Pos) -> RotaryFurnace {
    match g.machine(at).data {
        Data::RotaryFurnace(r) => *r,
        other => panic!("{other:?}"),
    }
}

fn id_stack(fluid: &str) -> Stack {
    Stack {
        id: "ntm:fluid_identifier_multi".into(),
        count: 1,
        damage: 0,
        custom: Some(format!("{{ntm_fluid:\"{fluid}\"}}")),
    }
}

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

/// Facing north: `rot = [-1, 0, 0]` (west), `-dir = [0, 0, 1]` (south), so steam
/// proxies `[-2, 0, 1]` & `[-1, 0, 1]` map to `[2, 70, 1]` & `[1, 70, 1]`
/// (`getSteamPorts`: `-dir - 2*rot` and `-dir - rot`), and pour spout is
/// `rot * 3` = `[-3, 71, 0]`.
/// Steam enters by duct, steel melts, pours 72/tick into a mold below `[-3, 69, 0]`,
/// and spent steam exits by duct to a receiving barrel.
#[test]
fn rotary_furnace_melts_steel_with_steam_ducts_and_pours_into_mold() {
    let mut g = game();
    rotary(&mut g, CORE, "north");
    let f = registry::facing_dir("north").unwrap();
    let steam_in_port = world_cell(CORE, f, [-2, 0, 1]);
    let spent_out_port = world_cell(CORE, f, [-1, 0, 1]);
    assert_eq!(steam_in_port, [2, 70, 1]);
    assert_eq!(spent_out_port, [1, 70, 1]);

    // Ducts reaching steam_in_port (z+1 -> [2, 70, 2]) and spent_out_port (z+1 -> [1, 70, 2]).
    let steam_duct = [steam_in_port[0], steam_in_port[1], steam_in_port[2] + 1];
    let steam_src = [steam_duct[0], steam_duct[1], steam_duct[2] + 1];
    duct(&mut g, steam_duct, "steam");
    barrel(&mut g, steam_src, "steam", 50_000, Mode::Output);

    let spent_duct = [spent_out_port[0], spent_out_port[1], spent_out_port[2] + 1];
    let spent_dst = [spent_duct[0], spent_duct[1], spent_duct[2] + 1];
    duct(&mut g, spent_duct, "spentsteam");
    barrel(&mut g, spent_dst, "spentsteam", 0, Mode::Input);

    // Mold 2 blocks below spout [-3, 71, 0] -> at [-3, 69, 0].
    let mold_at = [-3, 69, 0];
    mold(&mut g, mold_at, "ntm:mold__ingot");

    put(&mut g, CORE, 0, Some(Stack::new("minecraft:iron_ingot", 2)));
    put(&mut g, CORE, 1, Some(Stack::new("minecraft:coal", 2)));
    put(&mut g, CORE, slot::FUEL, Some(Stack::new("minecraft:coal", 1)));

    g.run(450);

    let ms = slots(&g, mold_at);
    assert!(
        ms[1].as_ref().is_some_and(|s| s.id == "ntm:ingot_steel"),
        "esperava lingote de aço no molde: {ms:?}, forno: {:?}",
        furnace(&mut g, CORE)
    );
    let Data::FluidStore(dst) = g.machine(spent_dst).data else { panic!() };
    assert!(dst.tank.mb > 0, "spentsteam não saiu pelo duto da proxy");
}

/// Facing east (`d = [1, 0, 0]`): `rot = dir.getRotation(DOWN) = [0, 0, -1]` (north),
/// so spout is at `[0, 71, -3]`, pouring 72 quanta/tick (`POUR`) into a foundry basin
/// to cast a full steel block (`648` quanta = `9 * 72`).
#[test]
fn facing_east_pours_north_into_basin_and_casts_block() {
    let mut g = game();
    rotary(&mut g, CORE, "east");
    let basin_at = [0, 70, -3];
    basin(&mut g, basin_at);

    // Preload 648 quanta of steel + verify POUR rate and full block cast.
    {
        let r = g.ready();
        let Some(Data::RotaryFurnace(f)) = r.world.machines.get_mut(&(OW.to_string(), CORE)).map(|m| &mut m.data) else {
            panic!()
        };
        f.output = Some(("steel".into(), 648));
    }
    g.run(1);
    let first_poured = 648 - furnace(&mut g, CORE).output.map_or(0, |o| o.1);
    assert_eq!(first_poured, 72, "taxa de vazamento deve ser 72 quanta/tick (INGOT.q(1))");

    g.run(230);
    assert_eq!(furnace(&mut g, CORE).output, None);
    let bs = slots(&g, basin_at);
    assert_eq!(bs[1], Some(Stack::new("ntm:block_steel", 1)), "bacia não moldou bloco: {bs:?}");
}

/// Fluid recipe (`lightoil` + `powder_desh_ready` -> `desh`): typed by the fluid
/// identifier in slot 3, fed through fluid proxy `[2, 0, -1]` (`[-2, 70, -1]` for north,
/// on the `rot * 2` side), while a duct touching core `[0, 70, 0]` directly delivers
/// nothing (core ports closed).
#[test]
fn fluid_proxy_delivers_lightoil_and_core_rejects_direct_duct() {
    let mut g = game();
    rotary(&mut g, CORE, "north");
    let f = registry::facing_dir("north").unwrap();
    let fluid_port = world_cell(CORE, f, [2, 0, -1]);
    assert_eq!(fluid_port, [-2, 70, -1]);

    // First: try feeding via a duct directly touching the core at [0, 70, -1] (not a proxy).
    let core_duct = [0, 70, -1];
    let core_src = [0, 70, -2];
    duct(&mut g, core_duct, "lightoil");
    barrel(&mut g, core_src, "lightoil", 2_000, Mode::Output);
    put(&mut g, CORE, slot::FLUID_ID, Some(id_stack("lightoil")));
    g.run(25);
    assert_eq!(furnace(&mut g, CORE).tank_in.fluid, "lightoil");
    assert_eq!(furnace(&mut g, CORE).tank_in.mb, 0, "núcleo não deve aceitar duto direto");

    // Now connect a lightoil duct to fluid_port [-2, 70, -1] (outside in rot = -X -> [-3, 70, -1]).
    let proxy_duct = [fluid_port[0] - 1, fluid_port[1], fluid_port[2]];
    let proxy_src = [fluid_port[0] - 2, fluid_port[1], fluid_port[2]];
    duct(&mut g, proxy_duct, "lightoil");
    barrel(&mut g, proxy_src, "lightoil", 2_000, Mode::Output);
    g.run(25);
    assert_eq!(furnace(&mut g, CORE).tank_in.mb, 2_000, "proxy de fluido deveria receber 2000 mB");

    // Supply steam, drain spentsteam (recipe uses 200 mB/t * 100 = 20 000 mB -> 200 mB spent > 120 cap),
    // and melt desh powder + coal -> pours into mold.
    let mold_at = [-3, 69, 0];
    mold(&mut g, mold_at, "ntm:mold__ingot");
    let steam_port = world_cell(CORE, f, [-2, 0, 1]);
    let steam_duct = [steam_port[0], steam_port[1], steam_port[2] + 1];
    let steam_src = [steam_port[0], steam_port[1], steam_port[2] + 2];
    duct(&mut g, steam_duct, "steam");
    barrel(&mut g, steam_src, "steam", 50_000, Mode::Output);
    let spent_port = world_cell(CORE, f, [-1, 0, 1]);
    let spent_duct = [spent_port[0], spent_port[1], spent_port[2] + 1];
    let spent_dst = [spent_port[0], spent_port[1], spent_port[2] + 2];
    duct(&mut g, spent_duct, "spentsteam");
    barrel(&mut g, spent_dst, "spentsteam", 0, Mode::Input);
    put(&mut g, CORE, 0, Some(Stack::new("ntm:powder_desh_ready", 1)));
    put(&mut g, CORE, slot::FUEL, Some(Stack::new("minecraft:coal", 1)));

    g.run(400);
    let ms = slots(&g, mold_at);
    assert_eq!(ms[1], Some(Stack::new("ntm:ingot_desh", 1)), "esperava lingote de desh: {ms:?}");
    assert_eq!(furnace(&mut g, CORE).tank_in.mb, 1_900);
}

/// R8 (mandatory): breaking `ntm:machine_rotary_furnace` drops only `slots[0..5]`
/// via engine spill; molten `output` is NOT dropped as scraps and vanishes.
#[test]
fn r8_breaking_rotary_furnace_loses_molten_output_and_keeps_only_slots() {
    let mut g = game();
    rotary(&mut g, CORE, "north");
    put(&mut g, CORE, 0, Some(Stack::new("minecraft:iron_ingot", 3)));
    put(&mut g, CORE, slot::FUEL, Some(Stack::new("minecraft:coal", 2)));
    {
        let r = g.ready();
        let Some(Data::RotaryFurnace(f)) = r.world.machines.get_mut(&(OW.to_string(), CORE)).map(|m| &mut m.data) else {
            panic!()
        };
        f.output = Some(("steel".into(), 144));
    }
    g.run(2);

    engine_break(&mut g, CORE);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped.len(), 2, "apenas slots 0 e 4 devem cair: {dropped:?}");
    assert!(dropped.iter().any(|d| d == "engine minecraft:iron_ingot 3"), "{dropped:?}");
    assert!(dropped.iter().any(|d| d == "engine minecraft:coal 2"), "{dropped:?}");
    assert!(
        !dropped.iter().any(|d| d.contains("scraps")),
        "R8 violado: gerou sucata ao quebrar rotativo: {dropped:?}"
    );
    assert!(g.ready().world.machines.is_empty());
}

/// Persistence: `save_state`/`load_state` in BE restores `output` and tanks across
/// a reopen even when the mod's saved JSON didn't have the latest state, and
/// `Restored` journal wins over a stale BE.
#[test]
fn rotary_furnace_persists_state_in_be_and_honors_restored_journal() {
    // 1. BE newer than empty saved machine JSON -> load_state restores BE state.
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone());
    g.run(4);
    rotary(&mut g, CORE, "north");
    {
        let r = g.ready();
        let Some(Data::RotaryFurnace(f)) = r.world.machines.get_mut(&(OW.to_string(), CORE)).map(|m| &mut m.data) else {
            panic!()
        };
        f.tank_in.fill("lightoil", 1234);
        f.steam.mb = 7777;
        f.spent_steam.mb = 42;
        f.burn_time = 500;
        f.max_burn_time = 800;
        f.burn_heat = 1.5;
        f.progress = 0.25;
        f.output = Some(("steel".into(), 216));
    }
    g.run(1); // writes BE engine (slots, screen_data + save_state)
    let saved_engine = w.borrow().engine[&(OW.to_string(), CORE)].clone();
    assert_eq!(saved_engine.1.len(), 27);
    drop(g);

    // Replace saved JSON with an earlier default RotaryFurnace (no molten output),
    // keeping the newer BE engine state: on reopen, BE load_state restores output.
    {
        let mut wb = w.borrow_mut();
        let blank = Machine::new(crate::machine::Kind::RotaryFurnace, "ntm:machine_rotary_furnace", Some("north".into()));
        wb.saved.insert((OW.to_string(), CORE), serde_json::to_string(&blank).unwrap());
        wb.engine.insert((OW.to_string(), CORE), saved_engine.clone());
    }
    let mut g = Game::new(w.clone());
    g.run(3);
    let loaded = furnace(&mut g, CORE);
    assert_eq!(loaded.output, Some(("steel".into(), 216)));
    assert_eq!((loaded.tank_in.fluid.as_str(), loaded.tank_in.mb), ("lightoil", 1234));
    assert_eq!((loaded.steam.mb, loaded.spent_steam.mb), (7777, 42));
    assert_eq!((loaded.burn_time, loaded.max_burn_time, loaded.burn_heat), (500, 800, 1.5));
    drop(g);

    // 2. Journal (`Restored`) wins over stale BE (stale BE has 0 output, journal has 144).
    let mut g = Game::new(w.clone());
    g.run(2);
    {
        let r = g.ready();
        let Some(Data::RotaryFurnace(f)) = r.world.machines.get_mut(&(OW.to_string(), CORE)).map(|m| &mut m.data) else {
            panic!()
        };
        f.output = Some(("copper".into(), 144));
        f.burn_time = 0;
        r.restored.insert((OW.to_string(), CORE));
    }
    // Stale BE still says 216 steel, 500 burn_time: Restored must ignore stale BE and overwrite it.
    w.borrow_mut().engine.insert((OW.to_string(), CORE), saved_engine);
    g.run(2);
    let after = furnace(&mut g, CORE);
    assert_eq!(after.output, Some(("copper".into(), 144)));
    assert_eq!(after.burn_time, 0);
}

/// Chunk-border pour: core at `[1, 70, 0]` (chunk `(0, 0)`) facing north pours at
/// `[-2, 71, 0]` in chunk `(-1, 0)`. Unloading chunk `(-1, 0)` holds the metal;
/// reloading resumes pouring into the mold at `[-2, 69, 0]`.
#[test]
fn rotary_furnace_pours_across_chunk_border_only_when_target_loaded() {
    let mut g = game();
    let core = [1, 70, 0]; // chunk (0, 0)
    let mold_at = [-2, 69, 0]; // chunk (-1, 0)
    rotary(&mut g, core, "north");
    mold(&mut g, mold_at, "ntm:mold__ingot");
    {
        let r = g.ready();
        let Some(Data::RotaryFurnace(f)) = r.world.machines.get_mut(&(OW.to_string(), core)).map(|m| &mut m.data) else {
            panic!()
        };
        f.output = Some(("steel".into(), 144));
    }
    // Unload chunk (-1, 0) before any pour.
    g.world.borrow_mut().unloaded.insert((OW.to_string(), -1, 0));
    g.run(10);
    assert_eq!(furnace(&mut g, core).output, Some(("steel".into(), 144)), "não deveria vazar em chunk descarregado");

    // Reload chunk (-1, 0) and mold -> pours 72 -> casts steel ingot.
    g.world.borrow_mut().unloaded.remove(&(OW.to_string(), -1, 0));
    g.run(220);
    assert_eq!(slots(&g, mold_at)[1], Some(Stack::new("ntm:ingot_steel", 1)));
    assert_eq!(furnace(&mut g, core).output, Some(("steel".into(), 72)));
}

/// F1: a jet that strikes a funnel cell of the strand caster resolves to its core,
/// on exactly the four `getMetalPourPos` cells (`MachineStrandCaster.findCore`).
#[test]
fn a_jet_on_a_funnel_cell_resolves_to_the_caster_core() {
    use crate::server::foundry_server::pour_core;
    let mut g = game();
    let caster = [0, 68, -3];
    g.place(caster, "ntm:machine_strand_caster[facing=north,honey_level=0]");
    g.run(2);
    let cells = crate::strand_caster::pour_cells(caster, "north");
    let key = (OW.to_string(), caster);
    let machines = g.ready().world.machines.clone();
    for c in &cells {
        assert_eq!(pour_core(&machines, &(OW.to_string(), *c)), Some(key.clone()), "{c:?}");
    }
    // A cell of the funnel that is no pour cell, the core itself and a far cell resolve to nothing.
    for c in [[-1, 70, -3], [0, 69, -3], [0, 70, -6], [5, 70, 5]] {
        assert_eq!(pour_core(&machines, &(OW.to_string(), c)), None, "{c:?}");
    }
}

/// F1 + `safe`: the jet of a rotary furnace over a funnel cell is parked (the
/// caster model is a later slice), and not a quantum is lost.
#[test]
fn a_rotary_jet_on_a_funnel_cell_parks_without_loss() {
    let mut g = game();
    rotary(&mut g, CORE, "east");
    let caster = [0, 68, -3];
    g.place(caster, "ntm:machine_strand_caster[facing=north,honey_level=0]");
    g.place([0, 70, -3], "minecraft:stone");
    g.run(2);
    {
        let r = g.ready();
        let Some(Data::RotaryFurnace(f)) = r.world.machines.get_mut(&(OW.to_string(), CORE)).map(|m| &mut m.data) else {
            panic!()
        };
        f.output = Some(("steel".into(), 288));
    }
    g.run(20);
    assert_eq!(furnace(&mut g, CORE).output, Some(("steel".into(), 288)));
}

/// The steam ports are `PortDef`s open to `dir.getOpposite()`, the fluid ones to `rot` (`getSteamPorts`, `getFluidPorts`):
/// a duct on another side of the port cell does not join the furnace.
#[test]
fn a_duct_reaches_a_rotary_port_only_from_the_connection_side() {
    let place_steam = |g: &mut Game, duct_at: Pos, src: Pos| {
        duct(g, duct_at, "steam");
        barrel(g, src, "steam", 50_000, Mode::Output);
    };
    // North: dir = (0,0,-1), rot = (-1,0,0); the steam port cell (2,70,1) is open to the south (0,0,1).
    let mut g = game();
    rotary(&mut g, CORE, "north");
    place_steam(&mut g, [3, 70, 1], [4, 70, 1]); // from the east: not its connection
    g.run(30);
    assert_eq!(furnace(&mut g, CORE).steam.mb, 0, "the wrong side takes nothing");
    let mut g = game();
    rotary(&mut g, CORE, "north");
    place_steam(&mut g, [2, 70, 2], [2, 70, 3]); // from the south: its connection
    g.run(30);
    assert!(furnace(&mut g, CORE).steam.mb > 0, "the connection side reaches it");
}
