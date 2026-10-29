//! Fluids in the fake world (docs/fluidos.md): ducts set to a type join
//! barrels; containers pour in and are filled through item transactions.
use super::*;
use crate::fluidbox::{FluidStore, Mode, actions as fa};

fn store(g: &mut Game, at: Pos) -> FluidStore {
    match g.machine(at).data {
        Data::FluidStore(s) => s,
        other => panic!("{other:?}"),
    }
}

fn set_store(g: &mut Game, at: Pos, f: impl FnOnce(&mut FluidStore)) {
    let r = g.ready();
    if let Some(Data::FluidStore(s)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) {
        f(s);
    }
    r.urgent = true;
}

/// Opens the duct at `at` and picks fluid `id` in its screen.
fn duct_type(g: &mut Game, at: Pos, id: u32) {
    g.open(at);
    g.click(fa::PICK + id);
}

fn until_txs_done(g: &mut Game) {
    for _ in 0..200 {
        if g.server.txs().is_empty() {
            return;
        }
        g.run(1);
    }
    panic!("transações paradas");
}

#[test]
fn a_duct_of_the_right_type_carries_water_between_barrels() {
    let mut g = game();
    let (a, d1, d2, b) = ([0, 70, 0], [1, 70, 0], [2, 70, 0], [3, 70, 0]);
    g.place(a, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.place(d1, "ntm:fluid_duct_neo");
    g.place(d2, "ntm:fluid_duct_neo");
    g.place(b, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    set_store(&mut g, a, |s| {
        s.tank.fill("water", 9000);
        s.mode = Mode::Output;
    });
    g.run(5);
    assert_eq!(store(&mut g, b).tank.mb, 0, "dutos sem tipo não ligam nada");
    duct_type(&mut g, d1, 1);
    duct_type(&mut g, d2, 29); // UF6: the water doesn't cross it
    g.run(5);
    assert_eq!(store(&mut g, b).tank.mb, 0);
    duct_type(&mut g, d2, 1);
    // The networks are checked every 10 ticks.
    g.run(12);
    assert_eq!(store(&mut g, b).tank.mb, 9000);
    assert_eq!(store(&mut g, a).tank.mb, 0);
    assert_eq!(store(&mut g, b).tank.fluid, "water");
    // The level shows the fill, and it is saved with energy and progress
    // (within `SAVE_EVERY` ticks).
    g.run(SAVE_EVERY as usize + 10);
    let saved = g.saved();
    let Data::FluidStore(s) = &saved.machines[&(OW.to_string(), b)].data else { panic!() };
    assert_eq!(s.tank.mb, 9000);
    assert!(g.world.borrow().get(OW, b).contains("honey_level=2"), "{}", g.world.borrow().get(OW, b));
}

#[test]
fn containers_pour_in_and_fill_through_transactions() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    g.open(at);
    g.hold("ntm:fluid_tank_full__water", 1);
    g.click(fa::POUR);
    until_txs_done(&mut g);
    assert_eq!(store(&mut g, at).tank.mb, 1000);
    assert_eq!(g.held(), None);
    assert_eq!(g.inv("ntm:fluid_tank_empty"), 1, "o vazio voltou");
    g.hold("minecraft:bucket", 1);
    g.click(fa::FILL);
    until_txs_done(&mut g);
    assert_eq!(store(&mut g, at).tank.mb, 0);
    assert_eq!(g.inv("minecraft:water_bucket"), 1);
    // Nothing more to fill: refused before anything leaves the hand.
    g.hold("minecraft:bucket", 1);
    g.click(fa::FILL);
    until_txs_done(&mut g);
    assert_eq!(g.held(), Some(("minecraft:bucket".to_string(), 1)));
}
