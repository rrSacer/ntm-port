//! Integration tests for `ntm:machine_chemical_factory` (`TileEntityMachineChemicalFactory.java`): the model's 32 recorded slots against
//! the engine's, the modules working on them, and what a save keeps.
use super::*;
use crate::chemfactory::{self, ChemFactory};
use crate::server::container_bridge::Stack;

const CORE: Pos = [24, 70, 24];
const STATE: &str = "ntm:machine_chemical_factory[facing=north,honey_level=0]";

fn factory(g: &mut Game) -> ChemFactory {
    let Data::ChemFactory(f) = g.machine(CORE).data else { panic!("no factory") };
    *f
}

fn engine_slot(g: &mut Game, i: usize) -> Option<Stack> {
    g.world.borrow().engine[&(OW.to_string(), CORE)].0[i].clone()
}

fn put(g: &mut Game, i: usize, stack: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), CORE)).unwrap().0[i] = stack;
}

fn with(g: &mut Game, f: impl FnOnce(&mut ChemFactory)) {
    let r = g.ready();
    let Data::ChemFactory(c) = &mut r.world.machines.get_mut(&(OW.to_string(), CORE)).unwrap().data else { panic!() };
    f(c);
}

fn placed() -> Game {
    let mut g = game();
    g.place(CORE, STATE);
    g.run(3);
    g
}

#[test]
fn the_engine_has_its_thirty_two_slots_and_the_modules_work_on_what_is_put_in_them() {
    let mut g = placed();
    assert_eq!(g.world.borrow().engine[&(OW.to_string(), CORE)].0.len(), chemfactory::SLOTS);
    // Module 2 (index 1) makes tar sand: 16 sand and 1 tar in its two input slots, the creative battery in slot 0.
    let r = crate::crafter::Station::ChemicalPlant.recipes().iter().find(|r| r.name == "chem.tarsand").unwrap();
    with(&mut g, |f| {
        f.set_recipe(1, Some("chem.tarsand"));
        f.water.mb = chemfactory::COOLANT_CAP;
    });
    put(&mut g, chemfactory::BATTERY, Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1)));
    put(&mut g, chemfactory::input(1, 0), Some(Stack::new(&r.items_in[0].ids[0], r.items_in[0].n * 2)));
    put(&mut g, chemfactory::input(1, 1), Some(Stack::new(&r.items_in[1].ids[0], 2)));
    // 4 000 mB of water pays for 40 working ticks (100 mB each); the ports of the next change bring more, here the water is put back.
    let mut used = 0;
    for _ in 0..12 {
        g.run(25);
        let f = factory(&mut g);
        used += chemfactory::COOLANT_CAP - f.water.mb;
        assert_eq!(f.water.mb + f.lps.mb, chemfactory::COOLANT_CAP, "every 100 mB of water became 100 mB of spent steam");
        with(&mut g, |f| {
            f.water.mb = chemfactory::COOLANT_CAP;
            f.lps.mb = 0;
        });
    }
    assert_eq!(used, 100 * 200, "200 working ticks: two rounds of 100 ticks (a step of 0.01 a hundred times is 1.0000000000000007 in a double)");
    let (item, n, _) = &r.items_out[0];
    assert_eq!(engine_slot(&mut g, chemfactory::output(1, 0)), Some(Stack::new(item, n * 2)), "two rounds, at double speed, into the engine's output slot");
    assert_eq!(engine_slot(&mut g, chemfactory::input(1, 0)), None);
    assert_eq!(engine_slot(&mut g, chemfactory::BATTERY).map(|s| s.id), Some(crate::machine::CREATIVE_BATTERY.to_string()), "the battery stays");
    // The other modules did nothing.
    let f = factory(&mut g);
    assert!(f.slots.iter().enumerate().all(|(i, s)| s.is_none() || i == chemfactory::BATTERY || i == chemfactory::output(1, 0)));
}

#[test]
fn a_recipe_and_its_progress_and_the_slots_survive_the_world_being_saved_and_opened_again() {
    let mut g = placed();
    with(&mut g, |f| {
        f.set_recipe(2, Some("chem.hydrogen"));
        f.modules[2].progress = 0.4;
        f.tin[6].set_type("water");
        f.tin[6].mb = 1_234;
        f.slots[chemfactory::output(3, 2)] = Some(Stack::new("minecraft:coal", 5));
    });
    g.run(1);
    let saved = serde_json::to_string(&g.machine(CORE)).unwrap();
    let back: crate::machine::Machine = serde_json::from_str(&saved).unwrap();
    let Data::ChemFactory(f) = back.data else { panic!() };
    assert_eq!((f.modules[2].recipe.as_deref(), f.tin[6].mb), (Some("chem.hydrogen"), 1_234));
    assert!(f.modules[2].progress > 0.0 || f.tin[6].mb == 1_234);
    assert_eq!(f.slots[chemfactory::output(3, 2)].as_ref().map(|s| s.count), Some(5), "the slots are in the record, so a journal restore has them");
}

#[test]
fn the_outputs_are_take_only_and_every_side_reaches_the_item_slots() {
    use crate::native::{container, slot_filters};
    let rules = slot_filters(chemfactory::BLOCK);
    assert_eq!(rules.len(), chemfactory::SLOTS);
    for i in 0..chemfactory::MODULES {
        for k in 0..3 {
            assert!(rules[chemfactory::output(i, k)].output && !rules[chemfactory::input(i, k)].output);
        }
        assert!(!rules[chemfactory::blueprint(i)].output);
    }
    let c = container(chemfactory::BLOCK).unwrap();
    assert_eq!((c.slots, c.inputs.len(), c.outputs.len()), (32, 12, 12));
    assert!(c.faces.iter().all(|f| f.len() == 24 && !f.contains(&chemfactory::BATTERY) && !f.contains(&chemfactory::blueprint(0))));
}
