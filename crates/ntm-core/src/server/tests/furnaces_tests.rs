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
