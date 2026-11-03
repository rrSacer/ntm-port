//! The oil chain's batch A in the fake world (docs/maquinas.md): the
//! liquefactor (`TileEntityMachineLiquefactor`) and the solidifier
//! (`TileEntityMachineSolidifier`), crafter stations.
use super::*;
use crate::crafter::Crafter;
use crate::fluid::FluidNode;
use crate::server::container_bridge::{Slots, Stack};

const LIQ: Pos = [6, 70, 0];
const SOL: Pos = [12, 70, 0];

fn machine(g: &mut Game, at: Pos, block: &str, slots: usize) {
    g.place(at, &format!("{block}[facing=north,honey_level=0]"));
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; slots], Vec::new()));
    g.run(2);
}

fn put(g: &mut Game, at: Pos, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = s;
}

fn slots(g: &Game, at: Pos) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), at)].0.clone()
}

fn edit(g: &mut Game, at: Pos, f: &dyn Fn(&mut Crafter)) {
    let r = g.ready();
    let Some(Data::Crafter(c)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) else { panic!() };
    f(c);
}

fn crafter(g: &mut Game, at: Pos) -> Crafter {
    match g.machine(at).data {
        Data::Crafter(c) => c,
        other => panic!("{other:?}"),
    }
}

/// Coal into 250 mB of coal oil a piece, in 60 ticks at 250 HE (the
/// creative battery in slot 1); the tank refuses another fluid's recipe
/// while it holds one.
#[test]
fn the_liquefactor_turns_coal_into_coal_oil() {
    let mut g = game();
    machine(&mut g, LIQ, "ntm:machine_liquefactor", 4);
    put(&mut g, LIQ, 1, Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1)));
    put(&mut g, LIQ, 0, Some(Stack::new("minecraft:coal", 2)));
    g.run(130);
    let c = crafter(&mut g, LIQ);
    assert_eq!((c.tout[0].fluid.as_str(), c.tout[0].mb), ("coaloil", 500));
    assert_eq!(slots(&g, LIQ)[0], None);
    // Ice melts into water: not into a tank of coal oil.
    put(&mut g, LIQ, 0, Some(Stack::new("minecraft:ice", 1)));
    g.run(70);
    let c = crafter(&mut g, LIQ);
    assert_eq!((c.tout[0].fluid.as_str(), c.tout[0].mb), ("coaloil", 500));
    assert_eq!(slots(&g, LIQ)[0], Some(Stack::new("minecraft:ice", 1)));
    // Emptied, the ice goes.
    edit(&mut g, LIQ, &|c| c.tout[0].mb = 0);
    g.run(70);
    let c = crafter(&mut g, LIQ);
    assert_eq!(c.tout[0].fluid, "water");
    assert!(c.tout[0].mb > 0);
}

/// Its tank takes the first fluid in; 200 mB of coal oil a coal tar, in 60
/// ticks; another item in the output stops it.
#[test]
fn the_solidifier_turns_coal_oil_into_tar() {
    let mut g = game();
    machine(&mut g, SOL, "ntm:machine_solidifier", 5);
    put(&mut g, SOL, 1, Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1)));
    edit(&mut g, SOL, &|c| {
        assert!(c.accepts_any());
        c.fluid_give("coaloil", 500);
    });
    g.run(130);
    assert_eq!(slots(&g, SOL)[0], Some(Stack::new("ntm:oil_tar__coal", 2)));
    assert_eq!(crafter(&mut g, SOL).tin[0].mb, 100);
    put(&mut g, SOL, 0, Some(Stack::new("minecraft:ice", 1)));
    edit(&mut g, SOL, &|c| c.fluid_give("coaloil", 400));
    g.run(70);
    assert_eq!(slots(&g, SOL)[0], Some(Stack::new("minecraft:ice", 1)));
    assert_eq!(crafter(&mut g, SOL).tin[0].mb, 500);
}

/// A world saved with them inert loads them as the crafters they are now.
#[test]
fn inert_oil_machines_load_as_crafters() {
    let m = crate::machine::Machine { generation: 0,
        block: "ntm:machine_liquefactor".into(),
        facing: Some("north".into()),
        data: Data::Inert { name: "ntm:machine_liquefactor".into() },
    };
    assert!(matches!(m.upgraded().data, Data::Crafter(c) if c.station == crate::crafter::Station::Liquefactor));
}
