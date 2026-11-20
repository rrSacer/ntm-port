//! The wood burner's fluid (`TileEntityMachineWoodBurner`: a tank at its back, the liquid burn): a duct at its back fills
//! it, one at its side does not, the tank goes with the slots in the block entity, and the fluid it burns is the tank's.
use super::*;
use crate::{fluidbox::Mode, machine::WoodBurner};

const AT: Pos = [0, 70, 0];

fn burner(g: &mut Game) -> WoodBurner {
    match g.machine(AT).data {
        Data::WoodBurner(w) => w,
        other => panic!("{other:?}"),
    }
}

fn barrel_of_oil(g: &mut Game, at: Pos, mb: u32) {
    g.place(at, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    let r = g.ready();
    let Some(Data::FluidStore(s)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) else { panic!() };
    s.tank.fill("woodoil", mb);
    s.mode = Mode::Output;
}

/// Facing north, its back is south: a duct there (typed woodoil, from a barrel) fills the tank; the same barrel at its side
/// does not.
#[test]
fn a_duct_at_the_back_fills_the_tank_and_one_at_the_side_does_not() {
    let mut g = game();
    g.place(AT, "ntm:machine_wood_burner[facing=north,honey_level=0]");
    g.run(2);
    g.place([0, 70, 1], "ntm:fluid_duct_neo");
    g.place([1, 70, 0], "ntm:fluid_duct_neo");
    g.run(2);
    g.type_duct([0, 70, 1], "woodoil");
    g.type_duct([1, 70, 0], "woodoil");
    barrel_of_oil(&mut g, [0, 70, 2], 9_000);
    barrel_of_oil(&mut g, [2, 70, 0], 9_000);
    g.run(30);
    let w = burner(&mut g);
    assert_eq!((w.tank.fluid.as_str(), w.tank.mb), ("woodoil", 9_000), "only the back barrel's");
}

/// Liquid burn in the world: 110 HE and 2 mB a tick, the tank in the block entity's data and record at the same moment.
#[test]
fn the_liquid_burn_runs_and_the_tank_is_in_the_block_entity() {
    let mut g = game();
    g.place(AT, "ntm:machine_wood_burner[facing=north,honey_level=0]");
    g.world.borrow_mut().engine.insert((OW.to_string(), AT), (vec![None; 6], Vec::new()));
    g.run(2);
    {
        let r = g.ready();
        let Some(Data::WoodBurner(w)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
        w.liquid_burn = true;
        w.tank.fill("woodoil", 1_000);
    }
    g.run(11);
    let w = burner(&mut g);
    assert!(w.power > 0 && w.tank.mb < 1_000, "{} {}", w.power, w.tank.mb);
    // Every mB the tank lost became 55 HE (110 a tick for 2): the sum is exact.
    assert_eq!(u64::from(1_000 - w.tank.mb) * 55, w.power);
    let data = g.world.borrow().engine[&(OW.to_string(), AT)].1.clone();
    assert_eq!(data.len(), 11, "the screen's nine and the tank's two");
    assert_eq!(data[9..], [i32::from(crate::fluid::get("woodoil").unwrap().id), w.tank.mb as i32]);
    let saved: Machine = serde_json::from_str(&g.world.borrow().saved[&(OW.to_string(), AT)]).unwrap();
    let Data::WoodBurner(s) = &saved.data else { panic!() };
    assert_eq!(s.tank.mb, w.tank.mb);
}
