//! Onda 1, slice 6: the firebox and the heating oven over an ashpit fill it with their fuel's ash
//! (`TileEntityFireboxBase.java:83-91`), and the ashpit makes it into `powder_ash` (`TileEntityAshpit`).
use super::*;
use crate::server::container_bridge::Stack;

const PIT: Pos = [4, 70, 0];
const FIRE: Pos = [4, 71, 0];

fn engine_block(g: &mut Game, at: Pos, state: &str, n: usize) {
    g.place(at, state);
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; n], Vec::new()));
}

fn slots(g: &Game, at: Pos) -> Vec<Option<Stack>> {
    g.world.borrow().engine[&(OW.to_string(), at)].0.clone()
}

fn pit(g: &mut Game) -> crate::ashpit::Ashpit {
    match &g.ready().world.machines[&(OW.to_string(), PIT)].data {
        Data::Ashpit(a) => a.clone(),
        d => panic!("{d:?}"),
    }
}

/// A firebox lights its coal (1600 x 1.25: 2000 ticks) and the ashpit under it gets 2000 of coal ash: one ash the
/// next tick, the threshold off the coal's level and, with the new stack, off the wood's (the Java's bug).
fn burns_into_the_pit(block: &str) {
    let mut g = game();
    engine_block(&mut g, PIT, "ntm:machine_ashpit[facing=north,honey_level=0]", 5);
    engine_block(&mut g, FIRE, block, 2);
    g.run(2);
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), FIRE)).unwrap().0[0] = Some(Stack::new("minecraft:coal", 1));
    g.run(3);
    assert_eq!(slots(&g, PIT)[0], Some(Stack::new("ntm:powder_ash__coal", 1)));
    let a = pit(&mut g);
    assert_eq!((a.coal, a.wood), (0, -2000));
}

#[test]
fn a_firebox_leaves_its_fuels_ash_in_the_pit_under_it() {
    burns_into_the_pit("ntm:heater_firebox[facing=north,honey_level=0]");
}

/// The oven's burn is an eighth of the firebox's (`getTimeMult` 0.125: 250 ticks of coal), its ash is the fuel's
/// `baseTime`, the module's 2000 before the mult.
#[test]
fn the_heating_oven_leaves_the_base_time_not_its_burn() {
    burns_into_the_pit("ntm:heater_oven[facing=north,honey_level=0]");
}

/// Not right under the core: no ash anywhere.
#[test]
fn a_firebox_beside_the_pit_leaves_none() {
    let mut g = game();
    engine_block(&mut g, PIT, "ntm:machine_ashpit[facing=north,honey_level=0]", 5);
    let beside = [PIT[0] + 1, PIT[1] + 1, PIT[2]];
    engine_block(&mut g, beside, "ntm:heater_firebox[facing=north,honey_level=0]", 2);
    g.run(2);
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), beside)).unwrap().0[0] = Some(Stack::new("minecraft:coal", 1));
    g.run(3);
    assert!(slots(&g, PIT).iter().all(Option::is_none));
    assert_eq!(pit(&mut g).coal, 0);
}
