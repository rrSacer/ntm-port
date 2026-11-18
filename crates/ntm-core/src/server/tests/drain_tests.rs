//! Integration tests for `ntm:machine_drain` (`TileEntityMachineDrain.java`).
use super::*;
use crate::drain::{self, Drain};
use crate::fluidbox::Mode;

const CORE: Pos = [24, 70, 24];
const STATE: &str = "ntm:machine_drain[facing=north,honey_level=0]";

fn drain(g: &mut Game) -> Drain {
    let Data::Drain(d) = g.machine(CORE).data else { panic!("no drain") };
    d
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

fn barrel(g: &mut Game, at: Pos) -> crate::fluid::Tank {
    let Data::FluidStore(f) = g.machine(at).data else { panic!() };
    f.tank
}

#[test]
fn the_identifier_types_the_drain_from_any_cell_of_its_volume_and_an_untyped_one_takes_nothing() {
    let mut g = game();
    g.place(CORE, STATE);
    // A barrel of water beside its east side (north: the sides are east and west).
    let src = [CORE[0] + 1, CORE[1], CORE[2]];
    g.place(src, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, src, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.fill("water", 8_000);
        f.mode = Mode::Output;
    });
    g.run(20);
    assert_eq!(drain(&mut g).tank.fluid, "", "untyped");
    assert_eq!(barrel(&mut g, src).mb, 8_000, "an untyped drain takes nothing");
    // A fluid identifier on the core (the glue gives the core for any cell of the volume).
    assert!(g.server.native_use_item(OW, CORE, crate::server::native_server::IDENTIFIER, Some("water")));
    assert_eq!(drain(&mut g).tank.fluid, "water");
    // Not an identifier, or no fluid in it: not taken.
    assert!(!g.server.native_use_item(OW, CORE, "ntm:ingot_steel", Some("water")));
    assert!(!g.server.native_use_item(OW, CORE, crate::server::native_server::IDENTIFIER, None));
    assert!(!g.server.native_use_item(OW, [CORE[0] + 5, CORE[1], CORE[2]], crate::server::native_server::IDENTIFIER, Some("water")));
    g.run(40);
    let left = barrel(&mut g, src).mb;
    assert!(left < 8_000, "it took water: {left}");
}

#[test]
fn a_typed_drain_empties_a_barrel_through_a_side_and_keeps_only_a_fraction() {
    let mut g = game();
    g.place(CORE, STATE);
    let src = [CORE[0] - 1, CORE[1], CORE[2]];
    g.place(src, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    g.server.native_use_item(OW, CORE, crate::server::native_server::IDENTIFIER, Some("water"));
    edit(&mut g, src, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.fill("water", 6_000);
        f.mode = Mode::Output;
    });
    let mut peak = 0;
    for _ in 0..80 {
        g.run(1);
        peak = peak.max(drain(&mut g).tank.mb);
        assert!(drain(&mut g).tank.mb <= drain::CAP);
    }
    assert_eq!(barrel(&mut g, src).mb, 0, "the whole barrel went in and out");
    assert!(peak > 0 && drain(&mut g).tank.mb < 10, "it holds what it has not spilled yet: peak {peak}");
}

#[test]
fn the_front_and_both_sides_are_ports_and_the_back_is_not() {
    let mut g = game();
    g.place(CORE, STATE);
    g.run(2);
    g.server.native_use_item(OW, CORE, crate::server::native_server::IDENTIFIER, Some("water"));
    // Facing north: ports north (front), east, west. The back (south) is the volume's own cell.
    for (side, port) in [([0, 0, -1], true), ([1, 0, 0], true), ([-1, 0, 0], true), ([0, 0, 1], false), ([0, 1, 0], false)] {
        let src = [CORE[0] + side[0], CORE[1] + side[1], CORE[2] + side[2]];
        g.place(src, "ntm:barrel_steel[facing=north,honey_level=0]");
        g.run(2);
        edit(&mut g, src, |d| {
            let Data::FluidStore(f) = d else { panic!() };
            f.tank.fill("water", 4_000);
            f.mode = Mode::Output;
        });
        g.run(30);
        assert_eq!(barrel(&mut g, src).mb == 0, port, "{side:?}");
        g.break_block(src);
        g.run(2);
    }
}

#[test]
fn a_radioactive_fluid_spilled_goes_to_the_chunk() {
    let mut g = game();
    g.place(CORE, STATE);
    g.run(2);
    g.server.native_chunk_radiation_loaded(OW, [1, 1]);
    g.server.native_use_item(OW, CORE, crate::server::native_server::IDENTIFIER, Some("uf6"));
    edit(&mut g, CORE, |d| {
        let Data::Drain(x) = d else { panic!() };
        x.tank.mb = 1_000;
    });
    g.run(1);
    let per_mb = crate::fluid::get("uf6").unwrap().vent_radiation;
    let rad = g.server.native_chunk_radiation_raw(OW, [1, 1]).unwrap_or(0.0);
    assert!((rad - per_mb * 500.0).abs() < 1e-3, "{rad} vs {}", per_mb * 500.0);
    // Water adds none.
    g.server.native_use_item(OW, CORE, crate::server::native_server::IDENTIFIER, Some("water"));
    edit(&mut g, CORE, |d| {
        let Data::Drain(x) = d else { panic!() };
        x.tank.mb = 1_000;
    });
    let before = g.server.native_chunk_radiation_raw(OW, [1, 1]).unwrap_or(0.0);
    g.run(1);
    assert!((g.server.native_chunk_radiation_raw(OW, [1, 1]).unwrap_or(0.0) - before).abs() < before * 0.01 + 1e-6);
}

#[test]
fn the_type_and_the_fill_survive_the_world_being_saved_and_opened_again() {
    let mut g = game();
    g.place(CORE, STATE);
    g.run(2);
    g.server.native_use_item(OW, CORE, crate::server::native_server::IDENTIFIER, Some("lava"));
    edit(&mut g, CORE, |d| {
        let Data::Drain(x) = d else { panic!() };
        x.tank.mb = 0;
    });
    g.run(4);
    let w = g.world.clone();
    drop(g);
    let mut g = Game::new(w);
    g.run(4);
    assert_eq!(drain(&mut g).tank.fluid, "lava");
}
