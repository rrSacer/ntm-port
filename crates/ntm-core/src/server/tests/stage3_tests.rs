//! The bedrock chain's stage 3 in the fake world (docs/maquinas.md): the
//! pyrolysis oven (`TileEntityMachinePyroOven`) and the electrolyser's fluid
//! line (`TileEntityElectrolyser`), both crafter stations whose input tank
//! takes the first fluid in.
use super::*;
use crate::crafter::Crafter;
use crate::fluid::FluidNode;
use crate::server::container_bridge::{Slots, Stack};

fn machine(g: &mut Game, at: Pos, block: &str, slots: usize) {
    g.place(at, &format!("{block}[facing=north,honey_level=0]"));
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; slots], Vec::new()));
    g.run(2);
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

fn slots(g: &Game, at: Pos) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), at)].0.clone()
}

fn put(g: &mut Game, at: Pos, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = s;
}

fn run(g: &mut Game, at: Pos, ticks: u32) {
    for _ in 0..ticks {
        edit(g, at, &|c| c.power = c.max_power());
        g.run(1);
    }
}

const OVEN: Pos = [6, 70, 0];

/// Roasting: a BASE ore into a roasted one and 50 mB of vitriol, in 10
/// ticks at 10 000 HE a tick.
#[test]
fn the_oven_roasts_bedrock_ore_into_vitriol() {
    let mut g = game();
    machine(&mut g, OVEN, "ntm:machine_pyrooven", 6);
    put(&mut g, OVEN, 1, Some(Stack::new("ntm:bedrock_ore__base_light", 2)));
    run(&mut g, OVEN, 21);
    let s = slots(&g, OVEN);
    assert_eq!(s[2], Some(Stack::new("ntm:bedrock_ore__base_roasted_light", 2)), "{s:?}");
    let c = crafter(&mut g, OVEN);
    assert_eq!((c.tout[0].fluid.as_str(), c.tout[0].mb), ("vitriol", 100));
}

/// A fluid-only recipe needs the input empty; its tank takes the first
/// fluid; its output tank takes the recipe's fluid, emptied of another.
#[test]
fn the_ovens_fluid_recipes_and_its_output_tank() {
    let mut g = game();
    machine(&mut g, OVEN, "ntm:machine_pyrooven", 6);
    edit(&mut g, OVEN, &|c| {
        assert!(c.accepts_any());
        c.fluid_give("heatingoil", 2_000);
    });
    run(&mut g, OVEN, 61);
    assert_eq!(slots(&g, OVEN)[2], Some(Stack::new("ntm:solid_fuel", 1)));
    // An ore in the input: its recipe, not the oil's.
    put(&mut g, OVEN, 2, None);
    edit(&mut g, OVEN, &|c| {
        c.tout[0].set_type("water");
        c.tout[0].mb = 1_000;
    });
    put(&mut g, OVEN, 1, Some(Stack::new("ntm:bedrock_ore__base_light", 1)));
    run(&mut g, OVEN, 11);
    let c = crafter(&mut g, OVEN);
    assert_eq!(slots(&g, OVEN)[2], Some(Stack::new("ntm:bedrock_ore__base_roasted_light", 1)));
    assert_eq!((c.tout[0].fluid.as_str(), c.tout[0].mb), ("vitriol", 50), "a água foi trocada pelo vitríolo");
    assert_eq!(c.tin[0].mb, 200, "o óleo ficou");
}

const CELL: Pos = [10, 70, 0];

/// Vitriol: 1 000 mB into 500 of sulfuric acid, 500 of chlorine, an iron
/// powder and a mercury nugget, in 20 ticks.
#[test]
fn the_electrolyser_splits_vitriol_into_sulfuric_acid_and_chlorine() {
    let mut g = game();
    machine(&mut g, CELL, "ntm:machine_electrolyser", 21);
    edit(&mut g, CELL, &|c| c.fluid_give("vitriol", 2_000));
    run(&mut g, CELL, 41);
    let c = crafter(&mut g, CELL);
    assert_eq!(c.tin[0].mb, 0);
    let mut outs: Vec<_> = c.tout.iter().map(|t| (t.fluid.clone(), t.mb)).collect();
    outs.sort();
    assert_eq!(outs, vec![("chlorine".to_string(), 1_000), ("sulfuric_acid".to_string(), 1_000)]);
    let s = slots(&g, CELL);
    let n = |id: &str| s.iter().flatten().filter(|x| x.id == id).map(|x| x.count).sum::<u32>();
    assert_eq!((n("ntm:powder_iron"), n("ntm:nugget_mercury")), (2, 2), "{s:?}");
}
