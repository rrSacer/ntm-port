//! The crystallizer in the fake world (docs/maquinas.md, "Cristalizador"):
//! a crafter station whose recipe is its input's and its tank's fluid's
//! (`CrystallizerRecipes.getOutput(item, tank type)`), the tank typed by
//! the first fluid in.
use super::*;
use crate::crafter::Crafter;
use crate::fluid::FluidNode;
use crate::server::container_bridge::{Slots, Stack};

const AT: Pos = [6, 70, 0];

fn crystallizer(g: &mut Game) {
    g.place(AT, "ntm:machine_crystallizer[facing=north,honey_level=0]");
    g.world.borrow_mut().engine.insert((OW.to_string(), AT), (vec![None; 8], Vec::new()));
    g.run(2);
}

fn edit(g: &mut Game, f: &dyn Fn(&mut Crafter)) {
    let r = g.ready();
    let Some(Data::Crafter(c)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
    f(c);
}

fn slots(g: &Game) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), AT)].0.clone()
}

fn put(g: &mut Game, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0[i] = s;
}

fn run(g: &mut Game, ticks: u32) {
    for _ in 0..ticks {
        edit(g, &|c| c.power = c.max_power());
        g.run(1);
    }
}

/// Washing a BASE ore: 250 mB of water, 101 ticks (`progress > 100`); the
/// empty tank takes the first fluid that comes, and its type.
#[test]
fn the_crystallizer_washes_bedrock_ore_with_whatever_fluid_its_tank_got() {
    let mut g = game();
    crystallizer(&mut g);
    edit(&mut g, &|c| {
        assert!(c.accepts_any(), "o tanque vazio aceita qualquer fluido");
        c.fluid_give("water", 1_000);
    });
    put(&mut g, 0, Some(Stack::new("ntm:bedrock_ore__base_light", 2)));
    run(&mut g, 101);
    assert_eq!(slots(&g)[2], Some(Stack::new("ntm:bedrock_ore__base_washed_light", 1)), "101 ticks: {:?}", slots(&g));
    run(&mut g, 101);
    let s = slots(&g);
    assert_eq!((s[0].clone(), s[2].clone()), (None, Some(Stack::new("ntm:bedrock_ore__base_washed_light", 2))));
    match g.machine(AT).data {
        Data::Crafter(c) => assert_eq!((c.tin[0].fluid.as_str(), c.tin[0].mb), ("water", 500)),
        other => panic!("{other:?}"),
    }
}

/// The same PRIMARY ore takes the tank's acid's recipe; a different item in
/// the output slot stops it until it is taken.
#[test]
fn the_tanks_fluid_picks_the_line_and_the_output_slot_holds_one_item() {
    let mut g = game();
    crystallizer(&mut g);
    edit(&mut g, &|c| c.fluid_give("sulfuric_acid", 1_000));
    put(&mut g, 2, Some(Stack::new("minecraft:cobblestone", 1)));
    put(&mut g, 0, Some(Stack::new("ntm:bedrock_ore__primary_light", 1)));
    run(&mut g, 250);
    assert_eq!(slots(&g)[0], Some(Stack::new("ntm:bedrock_ore__primary_light", 1)), "a saída ocupada por outro item");
    put(&mut g, 2, None);
    run(&mut g, 205);
    let s = slots(&g);
    assert_eq!(s[2], Some(Stack::new("ntm:bedrock_ore__primary_sulfuric_light", 1)), "{s:?}");
    // Hydrogen instead: the hydrogen line.
    put(&mut g, 2, None);
    edit(&mut g, &|c| {
        c.tin[0].mb = 0;
        c.fluid_give("hydrogen", 1_000);
    });
    put(&mut g, 0, Some(Stack::new("ntm:bedrock_ore__primary_light", 1)));
    run(&mut g, 205);
    assert_eq!(slots(&g)[2], Some(Stack::new("ntm:bedrock_ore__primary_first_light", 1)));
}
