//! Integration tests for `ntm:machine_microwave` (`TileEntityMicrowave.java`).
use super::*;
use crate::machine::BatteryMode;
use crate::microwave::{self, Microwave};
use crate::server::container_bridge::Stack;

const CORE: Pos = [24, 70, 24];
const STATE: &str = "ntm:machine_microwave[facing=north,honey_level=0]";

fn mw(g: &mut Game) -> Microwave {
    let Data::Microwave(m) = g.machine(CORE).data else { panic!("no microwave") };
    *m
}

fn engine_slot(g: &mut Game, i: usize) -> Option<Stack> {
    g.world.borrow().engine[&(OW.to_string(), CORE)].0[i].clone()
}

fn put(g: &mut Game, i: usize, stack: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), CORE)).unwrap().0[i] = stack;
}

fn press(g: &mut Game, id: u32) {
    g.world.borrow_mut().presses.push((OW.to_string(), CORE, id));
    g.run(1);
}

/// A battery on a cable beside the microwave (an east port: the block has all six).
fn powered(g: &mut Game) -> Pos {
    g.place(CORE, STATE);
    let (cable, bat) = ([CORE[0] + 1, CORE[1], CORE[2]], [CORE[0] + 2, CORE[1], CORE[2]]);
    g.place(cable, "ntm:red_cable");
    g.place(bat, "ntm:machine_battery[facing=north]");
    g.run(3);
    let r = g.ready();
    let Data::Battery(b) = &mut r.world.machines.get_mut(&(OW.to_string(), bat)).unwrap().data else { panic!() };
    b.mode = BatteryMode::Output;
    b.power = 1_000_000;
    bat
}

fn bat_power(g: &mut Game, at: Pos) -> u64 {
    let Data::Battery(b) = g.machine(at).data else { panic!() };
    b.power
}

#[test]
fn it_cooks_food_on_the_nets_power_at_the_speed_set_by_its_buttons() {
    let mut g = game();
    let bat = powered(&mut g);
    put(&mut g, microwave::INPUT, Some(Stack::new("minecraft:beef", 3)));
    press(&mut g, 0);
    press(&mut g, 0);
    assert_eq!(mw(&mut g).speed, 2, "two presses of the first button");
    g.run(400);
    assert_eq!(engine_slot(&mut g, microwave::OUTPUT), Some(Stack::new("minecraft:cooked_beef", 3)));
    assert_eq!(engine_slot(&mut g, microwave::INPUT), None);
    // 3 items of 75 ticks at 50 HE each; the buffer got the rest of the battery's output.
    let loss = 1_000_000 - bat_power(&mut g, bat);
    assert_eq!(loss - mw(&mut g).power, 3 * 75 * 50, "what the battery lost went to the buffer or the work");
    // The screen follows.
    let w = g.world.borrow();
    let data = &w.engine[&(OW.to_string(), CORE)].1;
    assert_eq!((data[8], data[9]), (2, 0), "speed, and no spin once idle");
}

#[test]
fn what_is_not_food_stays_and_the_top_speed_does_not_run() {
    let mut g = game();
    powered(&mut g);
    put(&mut g, microwave::INPUT, Some(Stack::new("minecraft:iron_ore", 2)));
    for _ in 0..3 {
        press(&mut g, 0);
    }
    g.run(100);
    assert_eq!(engine_slot(&mut g, microwave::OUTPUT), None, "not food: nothing is made");
    put(&mut g, microwave::INPUT, Some(Stack::new("minecraft:beef", 2)));
    for _ in 0..2 {
        press(&mut g, 0);
    }
    assert_eq!(mw(&mut g).speed, 5);
    g.run(100);
    assert_eq!(engine_slot(&mut g, microwave::OUTPUT), None, "at 5 the Java blows up, here it stands still");
    assert_eq!(engine_slot(&mut g, microwave::INPUT), Some(Stack::new("minecraft:beef", 2)));
    press(&mut g, 1);
    g.run(200);
    assert!(engine_slot(&mut g, microwave::OUTPUT).is_some(), "back to 4: it cooks");
}

#[test]
fn a_battery_in_its_slot_charges_it_and_a_hand_can_put_one_there_but_a_hopper_cannot() {
    use crate::native::{SlotAction, SlotOrigin, slot_allows, slot_filters};
    let rules = slot_filters(microwave::BLOCK);
    assert_eq!(rules.len(), 3);
    assert!(rules[0].accept.is_none() && rules[2].accept.is_none(), "plain `Slot`s: the menu takes anything");
    assert!(rules[1].output && !rules[0].output && !rules[2].output);
    let beef = Stack::new("minecraft:beef", 1);
    let stick = Stack::new("minecraft:stick", 1);
    let battery = Stack::new(crate::machine::CREATIVE_BATTERY, 1);
    let can = |slot, item: &Stack, action, origin| slot_allows(microwave::BLOCK, &[], slot, item, action, origin);
    // a hand: anything into the plain slots 0 and 2 (the battery, by hand), nothing into the output
    for item in [&beef, &stick, &battery] {
        assert!(can(microwave::INPUT, item, SlotAction::Insert, SlotOrigin::Manual));
        assert!(can(microwave::BATTERY, item, SlotAction::Insert, SlotOrigin::Manual));
        assert!(!can(microwave::OUTPUT, item, SlotAction::Insert, SlotOrigin::Manual));
    }
    // a hopper: only smeltables into slot 0, and only the output comes out
    assert!(can(microwave::INPUT, &beef, SlotAction::Insert, SlotOrigin::Automation));
    assert!(!can(microwave::INPUT, &stick, SlotAction::Insert, SlotOrigin::Automation));
    assert!(!can(microwave::BATTERY, &battery, SlotAction::Insert, SlotOrigin::Automation));
    assert!(can(microwave::OUTPUT, &beef, SlotAction::Extract, SlotOrigin::Automation));
    assert!(!can(microwave::INPUT, &beef, SlotAction::Extract, SlotOrigin::Automation));
    assert!(!can(microwave::BATTERY, &battery, SlotAction::Extract, SlotOrigin::Automation));
    // a battery put there by a command still charges the buffer
    let mut g = game();
    g.place(CORE, STATE);
    g.run(3);
    put(&mut g, microwave::BATTERY, Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1)));
    g.run(2);
    assert_eq!(mw(&mut g).power, microwave::MAX_POWER);
}

#[test]
fn the_speed_and_the_power_survive_the_world_being_saved_and_opened_again() {
    let mut g = game();
    g.place(CORE, STATE);
    g.run(3);
    for _ in 0..3 {
        press(&mut g, 0);
    }
    put(&mut g, microwave::BATTERY, Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1)));
    g.run(4);
    let w = g.world.clone();
    drop(g);
    let mut g = Game::new(w);
    g.run(4);
    let m = mw(&mut g);
    assert_eq!((m.speed, m.power), (3, microwave::MAX_POWER));
}
