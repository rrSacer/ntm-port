//! The blast furnace in the fake world (docs/maquinas.md): hot air from a
//! barrel through a duct speeds it up, its flue gas goes out to another;
//! steel from iron and sand on coal. Its items are the engine's slots
//! (`container_bridge`); here they are set on the machine directly.
use super::*;
use crate::blastfurnace::{self as bf, BlastFurnace};
use crate::fluid::Tank;
use crate::fluidbox::{FluidStore, Mode, actions as fa};
use crate::server::container_bridge::Stack;

const AT: Pos = [2, 70, 0];

fn furnace(g: &mut Game) -> BlastFurnace {
    match g.machine(AT).data {
        Data::BlastFurnace(b) => b,
        other => panic!("{other:?}"),
    }
}

fn set_furnace(g: &mut Game, f: impl FnOnce(&mut BlastFurnace)) {
    let r = g.ready();
    let Some(Data::BlastFurnace(b)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
    f(b);
    r.urgent = true;
}

fn set_store(g: &mut Game, at: Pos, f: impl FnOnce(&mut FluidStore)) {
    let r = g.ready();
    if let Some(Data::FluidStore(s)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) {
        f(s);
    }
}

fn store(g: &mut Game, at: Pos) -> FluidStore {
    match g.machine(at).data {
        Data::FluidStore(s) => s,
        other => panic!("{other:?}"),
    }
}

fn duct(g: &mut Game, at: Pos, fluid: &str) {
    g.place(at, "ntm:fluid_duct_neo");
    g.run(2);
    g.open(at);
    g.click(fa::PICK + u32::from(crate::fluid::get(fluid).unwrap().id));
}

/// Steel in iron, sand and coal; `air`: a big tank of hot air on its
/// left (the air in the furnace loses 5% a tick: the network keeps
/// putting ~200 mB a tick back, a barrel's 16 000 mB last 80 ticks).
fn steel_run(air: bool) -> (u32, BlastFurnace, FluidStore, u32) {
    let mut g = game();
    let (air_at, flue_at) = ([0, 70, 0], [4, 70, 0]);
    g.place(AT, "ntm:machine_blast_furnace[facing=north,honey_level=0]");
    g.place(air_at, "ntm:machine_fluidtank[facing=north,honey_level=0]");
    g.place(flue_at, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    duct(&mut g, [1, 70, 0], bf::AIRBLAST);
    duct(&mut g, [3, 70, 0], bf::FLUE);
    set_store(&mut g, air_at, |s| {
        if air {
            s.tank.fill(bf::AIRBLAST, 100_000);
        }
        s.mode = Mode::Output;
    });
    set_store(&mut g, flue_at, |s| s.tank = Tank::typed(bf::FLUE, 16_000));
    set_furnace(&mut g, |b| {
        b.slots[0] = Some(Stack::new("minecraft:coal", 2));
        b.slots[1] = Some(Stack::new("minecraft:iron_ingot", 2));
        b.slots[2] = Some(Stack::new("minecraft:sand", 1));
    });
    let mut ticks = 0;
    while furnace(&mut g).slots[3].is_none() && ticks < 1000 {
        g.run(1);
        ticks += 1;
    }
    let air_used = 100_000u32.saturating_sub(store(&mut g, air_at).tank.mb);
    g.run(30);
    (ticks, furnace(&mut g), store(&mut g, flue_at), air_used)
}

#[test]
fn hot_air_from_the_network_makes_steel_faster() {
    let (slow, b, flue, _) = steel_run(false);
    assert_eq!(b.slots[3], Some(Stack::new("ntm:ingot_steel", 2)));
    assert_eq!(b.slots[4], Some(Stack::new("ntm:ingot_raw__slag", 1)));
    assert!((790..=830).contains(&slow), "sem ar quente: {slow} ticks (800 do NTM)");
    assert!(flue.tank.mb > 0, "o gás de exaustão não saiu pela rede");
    let (fast, b, _, air) = steel_run(true);
    assert_eq!(b.slots[3], Some(Stack::new("ntm:ingot_steel", 2)));
    // Up to 5×: 800 / 5 = 160, plus the ticks before the network forms.
    assert!(fast * 4 < slow, "com ar quente: {fast} ticks contra {slow}");
    // About 200 mB a tick of work (the 5% lost from a full tank).
    assert!((150 * fast..=260 * fast).contains(&air), "{air} mB de ar em {fast} ticks");
}

#[test]
fn the_block_shows_its_work_and_drops_its_items() {
    let mut g = game();
    g.place(AT, "ntm:machine_blast_furnace[facing=north,honey_level=0]");
    g.run(2);
    set_furnace(&mut g, |b| {
        b.slots[0] = Some(Stack::new("minecraft:coal", 1));
        b.slots[1] = Some(Stack::new("minecraft:iron_ingot", 2));
        b.slots[2] = Some(Stack::new("minecraft:sand", 1));
    });
    g.run(400);
    let s = g.world.borrow().get(OW, AT);
    assert!(!s.contains("honey_level=0"), "{s}");
    g.world.borrow_mut().set(OW, AT, "minecraft:air");
    g.server.block_broken(OW, AT, "ntm:machine_blast_furnace[facing=north,honey_level=2]");
    g.run(60);
    // Iron and sand (the coal went in as fuel).
    assert_eq!(g.world.borrow().dropped.len(), 2, "{:?}", g.world.borrow().dropped);
}
