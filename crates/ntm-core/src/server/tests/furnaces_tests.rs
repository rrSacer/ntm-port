//! The K3 furnaces of `crate::furnaces` in the fake world: their items are
//! the engine's slots; their burn and progress ride with them.
use super::*;
use crate::server::container_bridge::{Slots, Stack};

const AT: Pos = [4, 70, 0];
const IRON: &str = "ntm:furnace_iron[facing=north,honey_level=0]";

fn engine_block(g: &mut Game, state: &str, n: usize) {
    g.place(AT, state);
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), AT), (vec![None; n], Vec::new()));
    g.run(2);
}

fn slots(g: &Game) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), AT)].0.clone()
}

fn put(g: &mut Game, i: usize, s: Option<Stack>) {
    g.world
        .borrow_mut()
        .engine
        .get_mut(&(OW.to_string(), AT))
        .unwrap()
        .0[i] = s;
}

fn count(s: &Slots, id: &str) -> u32 {
    s.iter()
        .flatten()
        .filter(|x| x.id == id)
        .map(|x| x.count)
        .sum()
}

#[test]
fn the_iron_furnace_conserves_items_while_the_player_takes_the_output() {
    let mut g = game();
    engine_block(&mut g, IRON, 5);
    put(&mut g, 0, Some(Stack::new("minecraft:raw_iron", 30)));
    put(&mut g, 1, Some(Stack::new("minecraft:coal", 3)));
    let mut taken = 0;
    for t in 0..6500 {
        g.run(1);
        let s = slots(&g);
        assert_eq!(
            count(&s, "minecraft:raw_iron") + count(&s, "minecraft:iron_ingot") + taken,
            30,
            "tick {t}: {s:?}"
        );
        if t % 301 == 11
            && let Some(o) = slots(&g)[3].clone()
        {
            taken += o.count;
            put(&mut g, 3, None);
        }
    }
    let s = slots(&g);
    assert_eq!(count(&s, "minecraft:raw_iron"), 0, "não terminou: {s:?}");
    assert_eq!(count(&s, "minecraft:iron_ingot") + taken, 30);
    // 30 ingots at 160 ticks, 2000 burn ticks a coal: 4800 ticks, 3 coals needed (2.4).
    assert_eq!(count(&s, "minecraft:coal"), 0);
}

#[test]
fn the_iron_furnace_reads_the_speed_upgrade_from_its_slot() {
    let mut g = game();
    engine_block(&mut g, IRON, 5);
    put(&mut g, 0, Some(Stack::new("minecraft:raw_iron", 4)));
    put(&mut g, 1, Some(Stack::new("minecraft:coal", 1)));
    put(&mut g, 4, Some(Stack::new("ntm:upgrade_speed_3", 1)));
    g.run(4 * 80 + 5);
    let s = slots(&g);
    assert_eq!(count(&s, "minecraft:iron_ingot"), 4, "{s:?}");
    assert_eq!(
        count(&s, "ntm:upgrade_speed_3"),
        1,
        "o upgrade não se gasta"
    );
}

#[test]
fn the_iron_furnace_state_comes_back_with_its_slots() {
    let mut g = game();
    engine_block(&mut g, IRON, 5);
    put(&mut g, 0, Some(Stack::new("minecraft:raw_iron", 5)));
    put(&mut g, 1, Some(Stack::new("minecraft:coal", 1)));
    g.run(300);
    let (saved_slots, saved_data) = g.world.borrow().engine[&(OW.to_string(), AT)].clone();
    // The screen's five values, then the burn, its maximum and the progress.
    assert_eq!(saved_data.len(), 5 + 3);
    assert_eq!(
        saved_data[5..8],
        [saved_data[2], saved_data[3], saved_data[0]]
    );
    let mut h = game();
    h.place(AT, IRON);
    h.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), AT), (saved_slots, saved_data.clone()));
    h.run(2);
    let Data::Furnace(f) = h.machine(AT).data else {
        panic!()
    };
    let crate::furnaces::Furnace::Iron(f) = *f else {
        panic!()
    };
    assert_eq!(f.max_burn_time, 2000);
    assert!(f.burn_time > 0 && f.burn_time <= saved_data[2] as u32);
    assert!(f.progress > 0);
}

/// A firebox under the steel furnace heats it through `heat_tick`; the three
/// lines smelt and the items are conserved while the player empties outputs.
#[test]
fn the_steel_furnace_smelts_on_the_heat_of_the_firebox_under_it() {
    let mut g = game();
    let (fire, top) = (AT, [AT[0], AT[1] + 1, AT[2]]);
    g.place(fire, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), fire), (vec![None; 2], Vec::new()));
    g.place(top, "ntm:furnace_steel[facing=north,honey_level=0]");
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), top), (vec![None; 6], Vec::new()));
    g.run(2);
    g.world
        .borrow_mut()
        .engine
        .get_mut(&(OW.to_string(), fire))
        .unwrap()
        .0[0] = Some(Stack::new("minecraft:coal", 8));
    for i in 0..3 {
        g.world
            .borrow_mut()
            .engine
            .get_mut(&(OW.to_string(), top))
            .unwrap()
            .0[i] = Some(Stack::new("minecraft:raw_iron", 6));
    }
    let count_top =
        |g: &Game, id: &str| count(&g.world.borrow().engine[&(OW.to_string(), top)].0, id);
    let mut taken = 0;
    for t in 0..9000 {
        g.run(1);
        assert_eq!(
            count_top(&g, "minecraft:raw_iron") + count_top(&g, "minecraft:iron_ingot") + taken,
            18,
            "tick {t}"
        );
        if t % 401 == 17 {
            for o in 3..6 {
                let mut w = g.world.borrow_mut();
                let slot = &mut w.engine.get_mut(&(OW.to_string(), top)).unwrap().0[o];
                if let Some(s) = slot.take() {
                    taken += s.count;
                }
            }
        }
    }
    assert!(
        count_top(&g, "minecraft:iron_ingot") + taken > 0,
        "nunca fundiu"
    );
    let Data::Furnace(f) = g.machine(top).data else {
        panic!()
    };
    let crate::furnaces::Furnace::Steel(f) = *f else {
        panic!()
    };
    assert!(f.heat > 0, "não puxou calor");
}

/// The combination oven on a firebox: coal becomes coke and creosote; the
/// creosote leaves through a duct into a barrel, and items are conserved.
#[test]
fn the_combination_oven_makes_coke_and_sends_its_fluid_down_a_duct() {
    let mut g = game();
    let (fire, top, duct, barrel) = (
        AT,
        [AT[0], AT[1] + 1, AT[2]],
        [AT[0] + 1, AT[1] + 1, AT[2]],
        [AT[0] + 2, AT[1] + 1, AT[2]],
    );
    g.place(fire, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), fire), (vec![None; 2], Vec::new()));
    g.place(top, "ntm:furnace_combination[facing=north,honey_level=0]");
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), top), (vec![None; 4], Vec::new()));
    g.place(duct, "ntm:fluid_duct_neo");
    g.place(barrel, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    g.type_duct(duct, "coalcreosote");
    let slots_of = |g: &Game, at: Pos| g.world.borrow().engine[&(OW.to_string(), at)].0.clone();
    g.world
        .borrow_mut()
        .engine
        .get_mut(&(OW.to_string(), fire))
        .unwrap()
        .0[0] = Some(Stack::new("minecraft:coal", 16));
    g.world
        .borrow_mut()
        .engine
        .get_mut(&(OW.to_string(), top))
        .unwrap()
        .0[0] = Some(Stack::new("minecraft:coal", 6));
    let mut coke = 0;
    for t in 0..4000 {
        g.run(1);
        let s = slots_of(&g, top);
        assert_eq!(
            count(&s, "minecraft:coal") + count(&s, "ntm:coke__coal") + coke,
            6,
            "tick {t}"
        );
        if t % 333 == 5
            && let Some(o) = g
                .world
                .borrow_mut()
                .engine
                .get_mut(&(OW.to_string(), top))
                .unwrap()
                .0[1]
                .take()
        {
            coke += o.count;
        }
    }
    let s = slots_of(&g, top);
    assert!(
        count(&s, "ntm:coke__coal") + coke >= 1,
        "nunca coqueificou: {s:?}"
    );
    let Data::FluidStore(b) = g.machine(barrel).data else {
        panic!()
    };
    let Data::Furnace(f) = g.machine(top).data else {
        panic!()
    };
    let crate::furnaces::Furnace::Combination(f) = *f else {
        panic!()
    };
    // Each coal gives 100 mB: what is not in the oven's tank is in the barrel.
    let made = (count(&s, "ntm:coke__coal") + coke) * 100;
    assert_eq!(f.tank.mb + b.tank.mb, made);
    assert_eq!(b.tank.fluid, "coalcreosote");
    assert!(b.tank.mb > 0, "o creosoto não saiu pelo duto");
}

/// The arc welder on a creative battery: the first recipe (steel plates and
/// dense wire into two motors), items conserved while the output is taken.
#[test]
fn the_arc_welder_welds_on_a_creative_battery_and_keeps_its_state() {
    let mut g = game();
    engine_block(&mut g, "ntm:machine_arc_welder[facing=north,honey_level=0]", 8);
    let r = &crate::welders::recipes(crate::welders::Model::Arc)[0];
    for (i, ing) in r.arc.iter().enumerate() {
        g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0[i] = Some(Stack::new(&ing.ids[0], ing.n * 3));
    }
    put(&mut g, 4, Some(Stack::new("ntm:battery_creative", 1)));
    let mut taken = 0;
    for t in 0..(3 * r.duration + 40) {
        g.run(1);
        if t % 77 == 3
            && let Some(o) = g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0[3].take()
        {
            taken += o.count;
        }
    }
    let s = slots(&g);
    assert_eq!(count(&s, &r.out.0) + taken, 3 * r.out.1, "{s:?}");
    assert!(s[..3].iter().all(Option::is_none), "{s:?}");
    let data = g.world.borrow().engine[&(OW.to_string(), AT)].1.clone();
    // The screen's ten values, then the saved ones (eight, and the high words of the three energy longs).
    assert_eq!(data.len(), 10 + 11);
}

/// The locksmith table through the bridge: the pins ride in the item's `custom_data` into the machine's slots and back to
/// the engine's, copied from the first to the second and re-rolled in the third.
#[test]
fn the_locksmith_table_copies_and_randomizes_pins_through_the_bridge() {
    use crate::keyforge::{pins, set_pins};
    let mut g = game();
    engine_block(&mut g, "ntm:machine_keyforge[facing=north,honey_level=0]", 3);
    let mut key = Stack::new("ntm:key", 1);
    set_pins(&mut key, 456);
    put(&mut g, 0, Some(key));
    put(&mut g, 1, Some(Stack::new("ntm:padlock", 1)));
    put(&mut g, 2, Some(Stack::new("ntm:padlock_reinforced", 1)));
    g.run(4);
    let s = slots(&g);
    assert_eq!(pins(s[0].as_ref().unwrap()), 456, "the first is untouched");
    assert_eq!(pins(s[1].as_ref().unwrap()), 456, "the second got its pins");
    let first = pins(s[2].as_ref().unwrap());
    assert!((100..=999).contains(&first), "{first}");
    g.run(8);
    let later = pins(slots(&g)[2].as_ref().unwrap());
    assert!((100..=999).contains(&later));
    // A fake key in the second slot is left as it was.
    let mut fake = Stack::new("ntm:key_fake", 1);
    set_pins(&mut fake, 7);
    put(&mut g, 1, Some(fake));
    g.run(4);
    assert_eq!(pins(slots(&g)[1].as_ref().unwrap()), 7);
}

/// The funnel through the bridge: nine items of a column become the result in the bottom row, and the button cycles
/// the mode (the screen's value).
#[test]
fn the_funnel_compresses_a_column_and_its_button_cycles_the_mode() {
    let mut g = game();
    engine_block(&mut g, "ntm:machine_funnel[facing=north,honey_level=0]", 18);
    put(&mut g, 4, Some(Stack::new("minecraft:gold_ingot", 20)));
    put(&mut g, 7, Some(Stack::new("minecraft:glowstone_dust", 5)));
    g.run(3);
    let s = slots(&g);
    assert_eq!(count(&s, "minecraft:gold_block"), 2, "{s:?}");
    assert_eq!(s[4].as_ref().map(|x| x.count), Some(2), "{s:?}");
    assert_eq!(s[4 + 9].as_ref().map(|x| (x.id.as_str(), x.count)), Some(("minecraft:gold_block", 2)));
    assert_eq!(s[7 + 9].as_ref().map(|x| (x.id.as_str(), x.count)), Some(("minecraft:glowstone", 1)), "{s:?}");
    assert_eq!(s[7].as_ref().map(|x| x.count), Some(1));
    // Mode 3x3 only: a stack of four is left alone.
    g.world.borrow_mut().presses.push((OW.to_string(), AT, 0));
    g.run(1);
    put(&mut g, 7, Some(Stack::new("minecraft:glowstone_dust", 4)));
    g.run(3);
    assert_eq!(slots(&g)[7].as_ref().map(|x| x.count), Some(4));
    assert_eq!(g.world.borrow().engine[&(OW.to_string(), AT)].1.first(), Some(&1));
}

/// The radiolysis chamber through the bridge: ten polonium pellets give 500 heat, the identifier in the first fluid slot types the
/// input and goes to the second, an item with the contagion comes out of the sterilization slot clean, and the pellets lose life.
#[test]
fn the_radiolysis_chamber_runs_its_pellets_identifier_and_sterilization_through_the_bridge() {
    let mut g = game();
    engine_block(&mut g, "ntm:machine_radiolysis[facing=north,honey_level=0]", 15);
    for i in 0..10 {
        put(&mut g, i, Some(Stack::new("ntm:pellet_rtg_polonium", 1)));
    }
    let mut id = Stack::new("ntm:fluid_identifier_multi", 1);
    id.custom = Some("{ntm_fluid:\"water\"}".into());
    put(&mut g, 10, Some(id.clone()));
    let mut sword = Stack::new("minecraft:iron_sword", 1);
    sword.custom = Some("{ntmContagion:1b}".into());
    put(&mut g, 12, Some(sword));
    g.run(250);
    let s = slots(&g);
    assert_eq!((s[10].clone(), s[11].clone()), (None, Some(id)), "{s:?}");
    assert_eq!((s[12].clone(), s[13].clone()), (None, Some(Stack::new("minecraft:iron_sword", 1))), "{s:?}");
    assert!(s[..10].iter().all(|p| p.as_ref().is_some_and(|p| p.id == "ntm:pellet_rtg_polonium" && p.custom.as_deref().is_some_and(|c| c.contains("PELLET_DEPLETION")))), "{s:?}");
    let data = g.world.borrow().engine[&(OW.to_string(), AT)].1.clone();
    assert_eq!((data.len(), data[1], data[2]), (12, 1_000_000, 500), "{data:?}");
    assert!(data[0] > 0, "the heat's power is in the store");
}

/// The exposure chamber through the bridge: a higgs capsule and a uranium ingot on a creative battery make an ingot, the empty
/// capsule comes out in its slot, and the screen's seven values follow.
#[test]
fn the_exposure_chamber_runs_a_capsule_through_the_engine_slots() {
    let mut g = game();
    engine_block(&mut g, "ntm:machine_exposure_chamber[facing=north,honey_level=0]", 7);
    put(&mut g, 0, Some(Stack::new("ntm:particle_higgs", 2)));
    put(&mut g, 2, Some(Stack::new("ntm:ingot_uranium", 5)));
    put(&mut g, 4, Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1)));
    g.run(260);
    let s = slots(&g);
    assert_eq!(s[0], Some(Stack::new("ntm:particle_higgs", 1)), "{s:?}");
    assert_eq!(s[1], Some(Stack::new("ntm:particle_empty", 1)), "{s:?}");
    assert_eq!(s[2], Some(Stack::new("ntm:ingot_uranium", 4)), "{s:?}");
    assert_eq!(s[3], Some(Stack::new("ntm:ingot_schraranium", 1)), "{s:?}");
    let data = g.world.borrow().engine[&(OW.to_string(), AT)].1.clone();
    // The screen's seven values, then the four saved ones (power, progress, uses, the particle's code).
    assert_eq!((data.len(), data[1], data[4], data[5], data[9], data[10]), (11, 1_000_000, 7, 8, 7, 1), "{data:?}");
}
