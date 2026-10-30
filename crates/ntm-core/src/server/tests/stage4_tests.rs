//! The bedrock chain's stage 4 in the fake world (docs/maquinas.md): the
//! large arc furnace (`TileEntityMachineArcFurnaceLarge`) and the
//! electrolyser's metal line (`TileEntityElectrolyser`), their molten metal
//! poured into a foundry mold that casts it.
use super::*;
use crate::fluid::FluidNode;
use crate::server::container_bridge::Stack;

fn machine(g: &mut Game, at: Pos, block: &str, slots: usize) {
    g.place(at, &format!("{block}[facing=north,honey_level=0]"));
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; slots], Vec::new()));
}

fn put(g: &mut Game, at: Pos, i: usize, s: Stack) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = Some(s);
}

fn slot(g: &Game, at: Pos, i: usize) -> Option<Stack> {
    g.world.borrow().engine[&(OW.to_string(), at)].0[i].clone()
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
    r.urgent = true;
}

/// An ingot mold on `at`, its mold in.
fn mold(g: &mut Game, at: Pos) {
    machine(g, at, "ntm:foundry_mold", 2);
    put(g, at, 0, Stack::new("ntm:mold__ingot", 1));
}

/// Ticks until the mold's output holds `item`, up to `max`.
fn cast(g: &mut Game, at: Pos, item: &str, max: u32) -> u32 {
    for t in 0..max {
        if slot(g, at, 1).is_some_and(|s| s.id == item) {
            return t;
        }
        g.run(1);
    }
    panic!("nenhum {item} em {max} ticks: {:?}", slot(g, at, 1));
}

const ARC: Pos = [0, 70, 0];
/// Facing north: the spout 3 blocks north of the core, a block up.
const UNDER_SPOUT: Pos = [0, 70, -3];

/// A bedrock ore melted in fluid mode (720 quanta of iron, 288 of copper
/// from two), poured with the lid open into a mold: an iron ingot.
#[test]
fn the_arc_furnace_melts_bedrock_ore_into_a_mold() {
    let mut g = game();
    machine(&mut g, ARC, "ntm:machine_arc_furnace_large", 30);
    mold(&mut g, UNDER_SPOUT);
    for e in 0..3 {
        put(&mut g, ARC, e, Stack::new("ntm:arc_electrode__graphite", 1));
    }
    put(&mut g, ARC, 3, Stack::new(crate::machine::CREATIVE_BATTERY, 1));
    put(&mut g, ARC, 25, Stack::new("ntm:bedrock_ore__primary_first_light", 2));
    g.run(2);
    edit(&mut g, ARC, |d| {
        let Data::ArcFurnace(f) = d else { panic!() };
        f.liquid_mode = true;
    });
    // 60 ticks of lid, 400 of work, 60 to open again.
    let t = cast(&mut g, UNDER_SPOUT, "minecraft:iron_ingot", 1_200);
    assert!(t > 460, "{t} ticks");
    let Data::ArcFurnace(f) = g.machine(ARC).data else { panic!() };
    assert_eq!(slot(&g, ARC, 0).map(|s| s.damage), Some(1), "o eletrodo gastou uma vez");
    assert!(f.liquids.iter().map(|m| m.1).sum::<u32>() < 720 + 288);
}

/// Solid mode: an item becomes its output where it lies; nothing pours.
#[test]
fn the_arc_furnace_in_solid_mode_keeps_its_output_in_the_grid() {
    let mut g = game();
    machine(&mut g, ARC, "ntm:machine_arc_furnace_large", 30);
    mold(&mut g, UNDER_SPOUT);
    for e in 0..3 {
        put(&mut g, ARC, e, Stack::new("ntm:arc_electrode__graphite", 1));
    }
    put(&mut g, ARC, 3, Stack::new(crate::machine::CREATIVE_BATTERY, 1));
    put(&mut g, ARC, 25, Stack::new("minecraft:sand", 2));
    g.run(600);
    assert_eq!((slot(&g, ARC, 5), slot(&g, ARC, 6)), (Some(Stack::new("ntm:nugget_silicon", 1)), Some(Stack::new("ntm:nugget_silicon", 1))));
    assert_eq!(slot(&g, UNDER_SPOUT, 1), None);
}

const CELL: Pos = [20, 70, 0];
/// Facing north: the left stack pours 6 blocks south, the right 6 north,
/// two blocks up.
const LEFT: Pos = [20, 70, 6];
const RIGHT: Pos = [20, 70, -6];

/// A bedrock ore through the metal line: iron to the left mold, copper to
/// the right one, crumbs in slot 15, 100 mB of nitric acid each.
#[test]
fn the_electrolyser_pours_its_two_metals_into_molds() {
    let mut g = game();
    machine(&mut g, CELL, "ntm:machine_electrolyser", 21);
    mold(&mut g, LEFT);
    mold(&mut g, RIGHT);
    put(&mut g, CELL, 0, Stack::new(crate::machine::CREATIVE_BATTERY, 1));
    put(&mut g, CELL, 14, Stack::new("ntm:bedrock_ore__primary_first_light", 1));
    g.run(2);
    edit(&mut g, CELL, |d| {
        let Data::Electrolyser(e) = d else { panic!() };
        e.fluid.fluid_give("nitric_acid", 1_000);
    });
    cast(&mut g, LEFT, "minecraft:iron_ingot", 400);
    cast(&mut g, RIGHT, "ntm:ingot_copper", 400);
    assert_eq!(slot(&g, CELL, 15), Some(Stack::new("ntm:bedrock_ore__crumbs_light", 1)));
    assert_eq!(slot(&g, CELL, 14), None);
    let Data::Electrolyser(e) = g.machine(CELL).data else { panic!() };
    assert_eq!(e.fluid.tin[1].mb, 900);
}
