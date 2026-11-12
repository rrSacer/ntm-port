//! The brick furnace in the fake world (docs/onda-2-sublote-1.md): its items
//! are the engine's slots, its burn, progress and ash levels ride with them.
use super::*;
use crate::furnace_brick::{ASH, FUEL, INPUT, OUTPUT};
use crate::server::container_bridge::{Slots, Stack};

const AT: Pos = [4, 70, 0];
const LIT: &str = "ntm:machine_furnace_brick_off[facing=north,honey_level=0]";

fn engine_block(g: &mut Game, state: &str) {
    g.place(AT, state);
    g.world
        .borrow_mut()
        .engine
        .insert((OW.to_string(), AT), (vec![None; 4], Vec::new()));
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
fn a_player_taking_the_output_never_loses_or_duplicates_items() {
    let mut g = game();
    engine_block(&mut g, LIT);
    put(&mut g, INPUT, Some(Stack::new("minecraft:raw_iron", 24)));
    put(&mut g, FUEL, Some(Stack::new("minecraft:coal", 4)));
    let (mut taken_ingots, mut taken_ash) = (0, 0);
    for t in 0..6000 {
        g.run(1);
        let s = slots(&g);
        assert_eq!(
            count(&s, "minecraft:raw_iron") + count(&s, "minecraft:iron_ingot") + taken_ingots,
            24,
            "tick {t}: {s:?}"
        );
        if t % 211 == 7 {
            let s = slots(&g);
            if let Some(o) = s[OUTPUT].clone() {
                taken_ingots += o.count;
                put(&mut g, OUTPUT, None);
            }
        }
        if t % 997 == 3
            && let Some(a) = slots(&g)[ASH].clone()
        {
            taken_ash += a.count;
            put(&mut g, ASH, None);
        }
    }
    let s = slots(&g);
    assert_eq!(count(&s, "minecraft:raw_iron"), 0, "não terminou: {s:?}");
    assert_eq!(count(&s, "minecraft:iron_ingot") + taken_ingots, 24);
    // Three coals (4800 burn ticks of coal ash) made 2 ashes (one at the second
    // coal, one at the third), the fourth coal stayed.
    assert_eq!(count(&s, "minecraft:coal"), 1);
    assert_eq!(count(&s, "ntm:powder_ash__coal") + taken_ash, 2);
}

#[test]
fn the_block_glows_while_it_burns() {
    let mut g = game();
    engine_block(&mut g, LIT);
    assert!(g.world.borrow().get(OW, AT).contains("honey_level=0"));
    put(&mut g, INPUT, Some(Stack::new("minecraft:raw_iron", 2)));
    put(&mut g, FUEL, Some(Stack::new("minecraft:coal", 1)));
    g.run(5);
    assert!(
        !g.world.borrow().get(OW, AT).contains("honey_level=0"),
        "{}",
        g.world.borrow().get(OW, AT)
    );
}

#[test]
fn the_state_goes_to_the_engine_with_the_slots_and_comes_back() {
    let mut g = game();
    engine_block(&mut g, LIT);
    put(&mut g, INPUT, Some(Stack::new("minecraft:raw_iron", 5)));
    put(&mut g, FUEL, Some(Stack::new("minecraft:coal", 2)));
    g.run(300);
    let (saved_slots, saved_data) = g.world.borrow().engine[&(OW.to_string(), AT)].clone();
    // screen: burn, max, progress; then the same three and the ash levels.
    assert_eq!(saved_data.len(), 3 + 6);
    assert!(
        saved_data[0] > 0
            && saved_data[1] == 1600
            && saved_data[2] > 0
            && saved_data[3..6] == saved_data[0..3],
        "{saved_data:?}"
    );
    // A new session over the same block entity: the machine takes it as it was.
    let mut h = game();
    h.place(AT, LIT);
    h.world.borrow_mut().engine.insert(
        (OW.to_string(), AT),
        (saved_slots.clone(), saved_data.clone()),
    );
    h.run(2);
    let Data::BrickFurnace(f) = h.machine(AT).data else {
        panic!()
    };
    assert_eq!(f.max_burn_time, 1600);
    assert_eq!(f.ash_coal, 1600);
    let before = saved_data[2];
    h.run(10);
    let Data::BrickFurnace(f) = h.machine(AT).data else {
        panic!()
    };
    assert!(
        f.progress as i32 > before % 200 - 1,
        "perdeu o progresso: {} depois de {before}",
        f.progress
    );
    assert_eq!(
        count(&slots(&h), "minecraft:raw_iron") + count(&slots(&h), "minecraft:iron_ingot"),
        5
    );
}

#[test]
fn breaking_it_drops_the_engines_slots_once() {
    let mut g = game();
    engine_block(&mut g, LIT);
    put(&mut g, INPUT, Some(Stack::new("minecraft:raw_iron", 5)));
    put(&mut g, FUEL, Some(Stack::new("minecraft:coal", 3)));
    g.run(3);
    let (s, _) = g
        .world
        .borrow_mut()
        .engine
        .remove(&(OW.to_string(), AT))
        .unwrap();
    {
        let mut w = g.world.borrow_mut();
        for x in s.into_iter().flatten() {
            w.dropped.push(format!("engine {} {}", x.id, x.count));
        }
    }
    g.break_block(AT);
    g.run(5);
    let dropped = g.world.borrow().dropped.clone();
    assert!(
        dropped.iter().all(|d| d.starts_with("engine ")),
        "o mod soltou de novo: {dropped:?}"
    );
}

/// `machine_electric_furnace_on` is the same machine as the unlit block: its
/// slots, its battery slot and its smelting.
#[test]
fn the_lit_electric_furnace_smelts_like_the_unlit_one() {
    for block in ["off", "on"] {
        let mut g = game();
        engine_block(&mut g, &format!("ntm:machine_electric_furnace_{block}[facing=north,honey_level=0]"));
        g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0.truncate(3);
        put(&mut g, 0, Some(Stack::new("minecraft:raw_iron", 2)));
        put(&mut g, 1, Some(Stack::new("ntm:battery_creative", 1)));
        g.run(230);
        let s = slots(&g);
        assert_eq!(count(&s, "minecraft:iron_ingot"), 2, "{block}: {s:?}");
        assert!(matches!(g.machine(AT).data, Data::ElectricFurnace(_)), "{block}");
    }
}
