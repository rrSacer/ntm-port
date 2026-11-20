//! Fluids in the fake world (docs/fluidos.md): ducts set to a type join
//! barrels (their containers pour in and fill through their slots:
//! `native::screens`).
use super::*;
use crate::fluidbox::{FluidStore, Mode};

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
    g.type_duct(d1, "water");
    g.type_duct(d2, "uf6"); // the water doesn't cross it
    g.run(5);
    assert_eq!(store(&mut g, b).tank.mb, 0);
    g.type_duct(d2, "water");
    // The networks are checked every 10 ticks.
    g.run(12);
    assert_eq!(store(&mut g, b).tank.mb, 9000);
    assert_eq!(store(&mut g, a).tank.mb, 0);
    assert_eq!(store(&mut g, b).tank.fluid, "water");
    // The level shows the fill, and the block entity keeps it.
    g.run(2);
    let saved: Machine = serde_json::from_str(&g.world.borrow().saved[&(OW.to_string(), b)]).unwrap();
    let Data::FluidStore(s) = &saved.data else { panic!() };
    assert_eq!(s.tank.mb, 9000);
    assert!(g.world.borrow().get(OW, b).contains("honey_level=2"), "{}", g.world.borrow().get(OW, b));
}

/// A barrel whose block entity comes back from the save with its items only up to the last kept (the empty container
/// in slot 3: four of its six) opens with the rest empty, and goes on (it read slot 4 out of bounds: RBMK T08's proof).
#[test]
fn a_barrel_read_back_with_fewer_slots_opens_with_the_rest_empty() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    let key = (OW.to_string(), at);
    g.world.borrow_mut().engine.get_mut(&key).unwrap().0[2] = Some(crate::server::container_bridge::Stack::new("ntm:fluid_barrel_full__water", 1));
    g.run(2);
    assert_eq!(store(&mut g, at).tank.mb, 16_000);
    let mut world = g.world.borrow().clone();
    let slots = &mut world.engine.get_mut(&key).unwrap().0;
    assert!(slots[3].is_some(), "the empty container in slot 3: {slots:?}");
    slots.truncate(4);
    let mut back = Game::new(Rc::new(RefCell::new(world)));
    back.run(3);
    assert_eq!(store(&mut back, at).tank.mb, 16_000);
}
