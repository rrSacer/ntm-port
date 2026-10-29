//! Recipe machines in the fake world (docs/maquinas.md): a recipe picked in
//! the list, items from the hand through transactions, energy, the output
//! back; fluids by container; the centrifuge picks by its input.
use super::*;
use crate::crafter::{Crafter, Station, actions as ca};

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

fn settle(g: &mut Game) {
    for _ in 0..300 {
        if g.server.txs().is_empty() {
            return;
        }
        g.run(1);
    }
    panic!("transações paradas");
}

fn row(st: Station, name: &str) -> u32 {
    st.recipes().iter().position(|r| r.name == name).unwrap_or_else(|| panic!("sem {name}")) as u32
}

#[test]
fn the_assembly_machine_makes_iron_plates_from_the_hand() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_assembly_machine[facing=north,honey_level=0]");
    g.run(2);
    g.open(at);
    g.click(ca::PICK_SCREEN);
    assert!(g.ch.screen.as_ref().unwrap().widgets.iter().any(|w| matches!(w, UiWidget::List { .. })), "lista de receitas");
    // A pick in the list is `select(LIST, row)`.
    let held = g.held_now();
    g.client.select_row(&mut g.ch, ca::LIST, row(Station::Assembly, "ass.plateiron"), held);
    g.run(2);
    assert_eq!(crafter(&mut g, at).recipe.as_deref(), Some("ass.plateiron"));
    assert!(g.screen_text().contains("Falta: 1×"), "{}", g.screen_text());
    g.hold("minecraft:iron_ingot", 5);
    g.click(ca::INSERT);
    settle(&mut g);
    assert_eq!(g.held(), None, "os 5 lingotes entraram");
    assert_eq!(crafter(&mut g, at).items, vec![("minecraft:iron_ingot".to_string(), 5)]);
    // Without energy nothing happens; with it, 60 ticks a plate.
    g.run(80);
    assert!(crafter(&mut g, at).out.is_empty());
    charge(&mut g, at);
    g.run(5 * 60 + 10);
    let c = crafter(&mut g, at);
    assert_eq!(c.out, vec![("ntm:plate_iron".to_string(), 5)]);
    assert!(c.items.is_empty());
    g.open(at);
    g.click(ca::TAKE_OUT);
    settle(&mut g);
    assert_eq!(g.inv("ntm:plate_iron"), 5);
    // An item the recipe doesn't take stays in the hand.
    g.hold("minecraft:dirt", 1);
    g.click(ca::INSERT);
    settle(&mut g);
    assert_eq!(g.held(), Some(("minecraft:dirt".to_string(), 1)));
}

#[test]
fn the_chemical_plant_turns_air_into_oxygen_through_containers() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_chemical_plant[facing=north,honey_level=0]");
    g.run(2);
    g.open(at);
    g.click(ca::PICK_SCREEN);
    let held = g.held_now();
    g.client.select_row(&mut g.ch, ca::LIST, row(Station::ChemicalPlant, "chem.oxygen"), held);
    g.run(2);
    charge(&mut g, at);
    g.click(ca::FLUID_SCREEN);
    for _ in 0..8 {
        g.hold("ntm:fluid_tank_full__air", 1);
        g.click(ca::POUR);
        settle(&mut g);
    }
    assert_eq!(g.inv("ntm:fluid_tank_empty"), 8);
    assert_eq!(crafter(&mut g, at).tin[0].mb, 8000);
    g.run(25);
    let c = crafter(&mut g, at);
    assert_eq!((c.tout[0].fluid.as_str(), c.tout[0].mb), ("oxygen", 500));
    assert_eq!(c.tin[0].mb, 0);
    // Not enough oxygen for a gas tank of 1 000: refused, nothing taken.
    g.hold("ntm:gas_empty", 1);
    g.click(ca::FILL);
    settle(&mut g);
    assert_eq!(g.held(), Some(("ntm:gas_empty".to_string(), 1)));
}

#[test]
fn the_centrifuge_picks_its_recipe_by_the_input() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_centrifuge[facing=north,honey_level=0]");
    g.run(2);
    g.open(at);
    let r = &Station::Centrifuge.recipes()[0];
    let input = r.items_in[0].ids[0].clone();
    g.hold(&input, 2);
    g.click(ca::INSERT);
    settle(&mut g);
    assert_eq!(crafter(&mut g, at).recipe.as_deref(), Some(r.name.as_str()));
    charge(&mut g, at);
    g.run(2 * 200 + 10);
    let c = crafter(&mut g, at);
    for (item, n, _) in &r.items_out {
        let got = c.out.iter().filter(|o| o.0 == *item).map(|o| o.1).sum::<u32>();
        assert!(got >= 2 * n, "{item}: {got}");
    }
}

#[test]
fn a_crafter_broken_drops_what_it_holds() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_assembly_machine[facing=north,honey_level=0]");
    g.run(2);
    g.open(at);
    g.click(ca::PICK_SCREEN);
    let held = g.held_now();
    g.client.select_row(&mut g.ch, ca::LIST, row(Station::Assembly, "ass.plateiron"), held);
    g.run(2);
    g.hold("minecraft:iron_ingot", 3);
    g.click(ca::INSERT);
    settle(&mut g);
    g.world.borrow_mut().set(OW, at, "minecraft:air");
    g.server.block_broken(OW, at, "ntm:machine_assembly_machine[facing=north,honey_level=0]");
    g.run(40);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert!(dropped[0].contains("iron_ingot") && dropped[0].contains("Count:3") || dropped[0].contains("count:3"), "{dropped:?}");
}

#[test]
fn gas_centrifuges_in_a_row_cascade_uf6() {
    use crate::gascent::{GasCent, Pseudo, actions as ga};
    let mut g = game();
    // Facing north: the one behind is to the south (+z).
    let (front, back) = ([0, 70, 0], [0, 70, 1]);
    g.place(front, "ntm:machine_gascent[facing=north,honey_level=0]");
    g.place(back, "ntm:machine_gascent[facing=north,honey_level=0]");
    g.run(2);
    g.open(front);
    for _ in 0..2 {
        g.hold("ntm:cell_uf6", 1);
        g.click(ga::POUR);
        settle(&mut g);
    }
    assert_eq!(g.inv("ntm:cell_empty"), 2);
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
