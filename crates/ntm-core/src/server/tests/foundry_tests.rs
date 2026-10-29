//! The foundry, stage 1, in the fake world (docs/maquinas.md, "Fundição"):
//! a firebox heats the crucible above it, items melt, an alloy is picked on
//! the old screen, and the waste stream pours into what is under it. Items
//! are set on the machines directly (their slots are the engine's).
use super::*;
use crate::foundry::{crucible::{Crucible, actions as ca}, firebox::Firebox};
use crate::server::container_bridge::Stack;

const FIRE: Pos = [0, 70, 0];
const CORE: Pos = [0, 71, 0];
/// Facing north: the waste pours 2 blocks south of the core.
const UNDER_WASTE: Pos = [0, 70, 2];

fn crucible(g: &mut Game, at: Pos) -> Crucible {
    match g.machine(at).data {
        Data::Crucible(c) => c,
        other => panic!("{other:?}"),
    }
}

fn firebox(g: &mut Game) -> Firebox {
    match g.machine(FIRE).data {
        Data::Firebox(f) => f,
        other => panic!("{other:?}"),
    }
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
    r.urgent = true;
}

fn foundry(g: &mut Game) {
    g.place(FIRE, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.place(CORE, "ntm:machine_crucible[facing=north,honey_level=0]");
    g.run(2);
    edit(g, FIRE, |d| {
        let Data::Firebox(f) = d else { panic!() };
        f.slots[0] = Some(Stack::new("minecraft:coal", 16));
    });
}

#[test]
fn a_firebox_heats_the_crucible_and_iron_melts_and_pours() {
    let mut g = game();
    foundry(&mut g);
    // A second crucible under the waste stream catches what pours.
    g.place(UNDER_WASTE, "ntm:machine_crucible[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.slots[1] = Some(Stack::new("minecraft:iron_ingot", 4));
    });
    // Coal in the firebox (200 TU a tick) warms it up; the crucible takes a
    // quarter of the difference a tick, so both climb at ~100 a tick: the
    // 50 000 to melt come after ~500 ticks.
    g.run(650);
    let (c, f) = (crucible(&mut g, CORE), firebox(&mut g));
    assert!(f.on, "a fornalha não acendeu");
    assert!(c.heat >= 50_000, "o cadinho não esquentou: {} (fornalha {})", c.heat, f.heat);
    // Melted, poured out the waste side into the crucible below.
    let mut melted = 0;
    for _ in 0..600 {
        g.run(1);
        let below = crucible(&mut g, UNDER_WASTE);
        melted = below.waste_stack.iter().find(|m| m.0 == "iron").map_or(0, |m| m.1);
        if melted == 4 * 72 {
            break;
        }
    }
    assert_eq!(melted, 4 * 72, "o ferro não chegou ao cadinho de baixo");
    assert!(crucible(&mut g, CORE).waste_stack.is_empty());
    assert_eq!(crucible(&mut g, CORE).slots[1], None);
}

#[test]
fn an_alloy_is_picked_on_the_old_screen_and_made() {
    let mut g = game();
    foundry(&mut g);
    g.open(CORE);
    g.click(ca::PICK);
    assert!(g.screen_text().contains("Cadinho: liga"), "{}", g.screen_text());
    // Row 1: the first alloy (steel); row 0 is none.
    g.click(ca::LIST + 1);
    assert_eq!(crucible(&mut g, CORE).recipe.as_deref(), Some("crucible.steel"));
    // Iron, carbon and flux in the recipe stack become steel; nothing to
    // pour into, so it stays.
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.recipe_stack = vec![("iron".into(), 16), ("carbon".into(), 24), ("flux".into(), 8)];
    });
    g.run(25);
    assert_eq!(crucible(&mut g, CORE).recipe_stack, vec![("steel".to_string(), 16)]);
}

#[test]
fn breaking_a_crucible_drops_its_items_and_scraps() {
    let mut g = game();
    foundry(&mut g);
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.slots[2] = Some(Stack::new("minecraft:iron_ingot", 1));
        c.waste_stack = vec![("iron".into(), 150), ("copper".into(), 30)];
    });
    g.world.borrow_mut().set(OW, CORE, "minecraft:air");
    g.server.block_broken(OW, CORE, "ntm:machine_crucible[facing=north,honey_level=0]");
    g.run(60);
    let dropped = g.world.borrow().dropped.clone();
    // The ingot, and two iron scraps (150 quanta); copper is under an ingot.
    assert_eq!(dropped.len(), 2, "{dropped:?}");
    assert!(dropped.iter().any(|d| d.contains("scraps__iron") && d.contains("count:2")), "{dropped:?}");
}
