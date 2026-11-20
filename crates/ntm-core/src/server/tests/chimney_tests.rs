//! Integration tests for `ntm:chimney_brick` and `ntm:chimney_industrial` (`TileEntityChimneyBase.java`).
use super::*;
use crate::chimney::{self, Chimney};
use crate::fluidbox::Mode;

const CORE: Pos = [24, 70, 24];
const BRICK: &str = "ntm:chimney_brick[facing=north,honey_level=0]";
const INDUSTRIAL: &str = "ntm:chimney_industrial[facing=north,honey_level=0]";
const PIT: &str = "ntm:machine_ashpit[facing=north,honey_level=0]";

fn chimney(g: &mut Game) -> Chimney {
    let Data::Chimney(c) = g.machine(CORE).data else { panic!("no chimney") };
    c
}

fn pit(g: &mut Game) -> crate::ashpit::Ashpit {
    let Data::Ashpit(a) = g.machine([CORE[0], CORE[1] - 1, CORE[2]]).data else { panic!("no ashpit") };
    a
}

fn barrel(g: &mut Game, at: Pos) -> crate::fluid::Tank {
    let Data::FluidStore(f) = g.machine(at).data else { panic!() };
    f.tank
}

fn fill(g: &mut Game, at: Pos, fluid: &str, mb: u32) {
    let r = g.ready();
    let Data::FluidStore(f) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else { panic!() };
    f.tank.fill(fluid, mb);
    f.mode = Mode::Output;
}

/// A chimney over an ashpit, a barrel against one of the four port cells (the east one).
fn rig(g: &mut Game, state: &str) -> Pos {
    g.place([CORE[0], CORE[1] - 1, CORE[2]], PIT);
    g.world.borrow_mut().engine.insert((OW.to_string(), [CORE[0], CORE[1] - 1, CORE[2]]), (vec![None; 5], Vec::new()));
    g.place(CORE, state);
    let barrel = [CORE[0] + 2, CORE[1], CORE[2]];
    g.place(barrel, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(3);
    barrel
}

#[test]
fn a_brick_chimney_eats_smoke_through_a_port_and_gives_its_ash_to_the_pit_under_it() {
    let mut g = game();
    let barrel_at = rig(&mut g, BRICK);
    fill(&mut g, barrel_at, "smoke", 1_500);
    g.run(30);
    assert_eq!(barrel(&mut g, barrel_at).mb, 0, "all of it went in");
    let a = pit(&mut g);
    assert_eq!((a.fly, a.soot), (1_500, 0), "the brick one captures ash and no soot");
    assert!(chimney(&mut g).on <= chimney::ON_TICKS);
}

#[test]
fn the_industrial_chimney_gives_soot_too_and_takes_the_other_two_smokes() {
    let mut g = game();
    let barrel_at = rig(&mut g, INDUSTRIAL);
    for (fluid, mb) in [("smoke", 600), ("smoke_leaded", 500), ("smoke_poison", 400)] {
        fill(&mut g, barrel_at, fluid, 0);
        let r = g.ready();
        let Data::FluidStore(f) = &mut r.world.machines.get_mut(&(OW.to_string(), barrel_at)).unwrap().data else { panic!() };
        f.tank.set_type(fluid);
        f.tank.mb = mb;
        g.run(20);
        assert_eq!(barrel(&mut g, barrel_at).mb, 0, "{fluid}");
    }
    let a = pit(&mut g);
    assert_eq!((a.fly, a.soot), (1_500, 1_500));
}

#[test]
fn it_takes_no_other_fluid_and_no_smoke_from_a_cell_that_is_no_port() {
    let mut g = game();
    let barrel_at = rig(&mut g, BRICK);
    fill(&mut g, barrel_at, "water", 1_000);
    g.run(20);
    assert_eq!(barrel(&mut g, barrel_at).mb, 1_000, "water is no smoke");
    // above the core, beside a corner (no ports there) and under a port (its only open face is the one outwards)
    for at in [[CORE[0], CORE[1] + 1, CORE[2]], [CORE[0] + 1, CORE[1], CORE[2] + 2], [CORE[0] + 2, CORE[1], CORE[2] + 1], [CORE[0] + 1, CORE[1] - 1, CORE[2]]] {
        g.place(at, "ntm:barrel_steel[facing=north,honey_level=0]");
        g.run(2);
        fill(&mut g, at, "smoke", 800);
        g.run(20);
        assert_eq!(barrel(&mut g, at).mb, 800, "{at:?}");
        g.break_block(at);
        g.run(2);
    }
    assert_eq!(pit(&mut g).fly, 0);
}

#[test]
fn every_one_of_the_four_ports_takes_smoke_and_a_chimney_without_a_pit_just_loses_it() {
    for side in [[2, 0, 0], [-2, 0, 0], [0, 0, 2], [0, 0, -2]] {
        let mut g = game();
        g.place(CORE, BRICK);
        let at = [CORE[0] + side[0], CORE[1], CORE[2] + side[2]];
        g.place(at, "ntm:barrel_steel[facing=north,honey_level=0]");
        g.run(3);
        fill(&mut g, at, "smoke", 700);
        g.run(20);
        assert_eq!(barrel(&mut g, at).mb, 0, "{side:?}");
        assert_eq!(chimney(&mut g).ash, 0, "the ash went to the pit under it, which is none: nowhere");
    }
}

#[test]
fn the_ash_wraps_like_the_int_it_is_added_to() {
    // `ashLevelFly += ashTick`: an int plus a long, cut back to an int. Called at the step itself (the pit also turns its level into ash each tick).
    let mut g = game();
    rig(&mut g, BRICK);
    let pit_key = (OW.to_string(), [CORE[0], CORE[1] - 1, CORE[2]]);
    {
        let r = g.ready();
        let Data::Ashpit(a) = &mut r.world.machines.get_mut(&pit_key).unwrap().data else { panic!() };
        a.fly = i32::MAX - 100;
        a.soot = 7;
        let Data::Chimney(c) = &mut r.world.machines.get_mut(&(OW.to_string(), CORE)).unwrap().data else { panic!() };
        c.ash = 500;
        c.soot = 0;
    }
    g.server.ash_tick();
    let a = pit(&mut g);
    assert_eq!(a.fly, (i64::from(i32::MAX - 100) + 500) as i32);
    assert!(a.fly < 0);
    assert_eq!(a.soot, 7, "the brick one adds no soot");
    assert_eq!((chimney(&mut g).ash, chimney(&mut g).soot), (0, 0), "taken once");
}

#[test]
fn the_body_survives_the_world_being_saved_and_opened_again() {
    let mut g = game();
    g.place(CORE, INDUSTRIAL);
    g.run(3);
    let w = g.world.clone();
    drop(g);
    let mut g = Game::new(w);
    g.run(3);
    assert_eq!(chimney(&mut g).body, chimney::Body::Industrial);
}
