//! Recipe machines in the fake world (docs/maquinas.md): a recipe picked by
//! its menu button (`native::recipes`), items through the block entity's
//! slots, energy, the output back; the centrifuge picks by its input.
use super::*;
use crate::crafter::{Crafter, Station};
use crate::native::{RECIPE_BASE, RECIPE_CLEAR};
use crate::server::container_bridge::{Slots, Stack};

const ASSEMBLY: &str = "ntm:machine_assembly_machine[facing=north,honey_level=0]";

fn crafter(g: &mut Game, at: Pos) -> Crafter {
    match g.machine(at).data {
        Data::Crafter(c) => c,
        other => panic!("{other:?}"),
    }
}

fn charge(g: &mut Game, at: Pos) {
    let r = g.ready();
    if let Some(Data::Crafter(c)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) {
        c.power = 10_000_000;
    }
}

fn row(st: Station, name: &str) -> u32 {
    st.recipes().iter().position(|r| r.name == name).unwrap_or_else(|| panic!("sem {name}")) as u32
}

/// The machine at `at` with `n` block entity slots.
fn with_slots(g: &mut Game, at: Pos, state: &str, n: usize) {
    g.place(at, state);
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; n], Vec::new()));
    g.run(2);
}

fn slots(g: &Game, at: Pos) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), at)].0.clone()
}

fn press(g: &mut Game, at: Pos, button: u32) {
    g.world.borrow_mut().presses.push((OW.to_string(), at, button));
    g.run(1);
}

#[test]
fn the_assembly_machine_makes_iron_plates_from_its_slots() {
    let mut g = game();
    let at = [0, 70, 0];
    with_slots(&mut g, at, ASSEMBLY, 17);
    let plate = row(Station::Assembly, "ass.plateiron");
    press(&mut g, at, RECIPE_BASE + plate);
    assert_eq!(crafter(&mut g, at).recipe.as_deref(), Some("ass.plateiron"));
    g.run(1);
    let data = g.world.borrow().engine[&(OW.to_string(), at)].1.clone();
    assert_eq!(data[crate::native::recipe_slot("ntm:machine_assembly_machine").unwrap()], plate as i32 + 1, "a receita na tela");
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[4] = Some(Stack::new("minecraft:iron_ingot", 5));
    g.run(2);
    assert_eq!(crafter(&mut g, at).items, vec![("minecraft:iron_ingot".to_string(), 5)]);
    // Without energy nothing happens; with it, 60 ticks a plate.
    g.run(80);
    assert!(crafter(&mut g, at).out.is_empty());
    charge(&mut g, at);
    g.run(5 * 60 + 10);
    let c = crafter(&mut g, at);
    assert_eq!(c.out, vec![("ntm:plate_iron".to_string(), 5)]);
    assert!(c.items.is_empty());
    assert_eq!(slots(&g, at)[16], Some(Stack::new("ntm:plate_iron", 5)));
    // Cleared: no recipe, nothing made.
    press(&mut g, at, RECIPE_CLEAR);
    assert_eq!(crafter(&mut g, at).recipe, None);
}

#[test]
fn the_chemical_plant_turns_air_into_oxygen() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_chemical_plant[facing=north,honey_level=0]");
    g.run(2);
    press(&mut g, at, RECIPE_BASE + row(Station::ChemicalPlant, "chem.oxygen"));
    charge(&mut g, at);
    {
        let r = g.ready();
        let Some(Data::Crafter(c)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) else { panic!() };
        assert_eq!(c.tin[0].fill("air", 8000), 8000, "o tanque tem o tipo da receita");
    }
    g.run(25);
    let c = crafter(&mut g, at);
    assert_eq!((c.tout[0].fluid.as_str(), c.tout[0].mb), ("oxygen", 500));
    assert_eq!(c.tin[0].mb, 0);
}

/// The centrifuge has no list: a pick by button changes nothing.
#[test]
fn the_centrifuge_picks_its_recipe_by_the_input() {
    let mut g = game();
    let at = [0, 70, 0];
    with_slots(&mut g, at, "ntm:machine_centrifuge[facing=north,honey_level=0]", 5);
    let r = &Station::Centrifuge.recipes()[0];
    press(&mut g, at, RECIPE_BASE + 1);
    assert_eq!(crafter(&mut g, at).recipe, None);
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[0] = Some(Stack::new(&r.items_in[0].ids[0], 2));
    g.run(2);
    assert_eq!(crafter(&mut g, at).recipe.as_deref(), Some(r.name.as_str()));
}

#[test]
fn a_crafter_broken_drops_only_what_its_slots_cant_show() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, ASSEMBLY);
    g.run(2);
    {
        let r = g.ready();
        let Some(Data::Crafter(c)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) else { panic!() };
        c.items = vec![("minecraft:iron_ingot".into(), 3)];
    }
    g.break_block(at);
    g.run(40);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped, ["minecraft:overworld 0 70 0 minecraft:iron_ingot 3"], "sem slots: tudo cai");
}

#[test]
fn gas_centrifuges_in_a_row_cascade_uf6() {
    use crate::gascent::{GasCent, Pseudo};
    let mut g = game();
    // Facing north: the one behind is to the south (+z).
    let (front, back) = ([0, 70, 0], [0, 70, 1]);
    g.place(front, "ntm:machine_gascent[facing=north,honey_level=0]");
    g.place(back, "ntm:machine_gascent[facing=north,honey_level=0]");
    g.run(2);
    {
        let r = g.ready();
        let Some(Data::GasCent(c)) = r.world.machines.get_mut(&(OW.to_string(), front)).map(|m| &mut m.data) else { panic!() };
        c.tank.fill("uf6", 2000);
    }
    let gc = |g: &mut Game, at: Pos| -> GasCent {
        match g.machine(at).data {
            Data::GasCent(c) => c,
            other => panic!("{other:?}"),
        }
    };
    for _ in 0..(3 * crate::gascent::SPEED) {
        for at in [front, back] {
            let r = g.ready();
            if let Some(Data::GasCent(c)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) {
                c.power = crate::gascent::MAX_POWER;
            }
        }
        g.run(1);
    }
    let (a, b) = (gc(&mut g, front), gc(&mut g, back));
    assert!(a.out.iter().any(|o| o.0 == "ntm:nugget_u238"), "{:?}", a.out);
    assert_eq!(b.input.0, Pseudo::Leuf6, "o de trás recebeu o LEUF6");
    assert!(b.input.1 > 0 || b.out.iter().any(|o| o.0 == "ntm:nugget_u238"));
}
