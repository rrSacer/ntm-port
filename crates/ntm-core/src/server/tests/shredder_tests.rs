//! The shredder in the fake world (docs/maquinas.md): energy from the
//! network, the NTM's table, blade damage in the item, its items dropped
//! when broken. Its items are the engine's slots (`container_bridge`); here
//! they are set on the machine directly.
use super::*;
use crate::shredder::{self, Shredder, Stack};

const AT: Pos = [4, 70, 0];

fn shredder_at(g: &mut Game) -> Shredder {
    match g.machine(AT).data {
        Data::Shredder(s) => s,
        other => panic!("{other:?}"),
    }
}

fn set_slots(g: &mut Game, f: impl FnOnce(&mut Shredder)) {
    let r = g.ready();
    let Some(Data::Shredder(s)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
    f(s);
}

/// A full battery in output mode beside the shredder.
fn powered(g: &mut Game) {
    let bat = [3, 70, 0];
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    g.place(AT, "ntm:machine_shredder[facing=north,honey_level=0]");
    g.run(3);
    let r = g.ready();
    if let Some(Data::Battery(b)) = r.world.machines.get_mut(&(OW.to_string(), bat)).map(|m| &mut m.data) {
        b.power = b.max;
        b.mode = crate::machine::BatteryMode::Output;
    }
}

#[test]
fn the_shredder_grinds_with_network_power_and_wears_its_blades() {
    let mut g = game();
    powered(&mut g);
    set_slots(&mut g, |s| {
        s.slots[0] = Some(Stack::new("minecraft:iron_ingot", 3));
        s.slots[1] = Some(Stack::new("minecraft:cobblestone", 1));
        s.slots[2] = Some(Stack::new("minecraft:rotten_flesh", 1));
        s.slots[27] = Some(Stack::new("ntm:blades_steel", 1));
        s.slots[28] = Some(Stack::new("ntm:blades_desh", 1));
    });
    let mut worked = false;
    for _ in 0..3 * shredder::TICKS + 20 {
        g.run(1);
        worked |= g.world.borrow().get(OW, AT).contains("honey_level=") && !g.world.borrow().get(OW, AT).contains("honey_level=0");
    }
    let s = shredder_at(&mut g);
    let outs: Vec<(String, u32)> = s.slots[shredder::OUTPUTS].iter().flatten().map(|x| (x.id.clone(), x.count)).collect();
    // Three cycles: 3 iron dust; gravel and scrap (no recipe) in the first.
    assert_eq!(outs, vec![("ntm:powder_iron".into(), 3), ("minecraft:gravel".into(), 1), ("ntm:scrap".into(), 1)]);
    assert_eq!(s.slots[27].as_ref().map(|b| b.damage), Some(3));
    assert_eq!(s.slots[28].as_ref().map(|b| b.damage), Some(0));
    assert!(worked, "o bloco não mostrou o progresso");
    // The damage is in the world's save.
    g.run(250);
    let saved: Machine = serde_json::from_str(&g.world.borrow().saved[&(OW.to_string(), AT)]).unwrap();
    let Data::Shredder(s) = saved.data else { panic!() };
    assert_eq!(s.slots[27].as_ref().map(|b| b.damage), Some(3));
}

#[test]
fn without_power_nothing_happens_and_breaking_drops_its_items() {
    let mut g = game();
    g.place(AT, "ntm:machine_shredder[facing=north,honey_level=0]");
    g.run(3);
    set_slots(&mut g, |s| {
        s.slots[0] = Some(Stack::new("minecraft:iron_ingot", 2));
        s.slots[27] = Some(Stack::new("ntm:blades_steel", 1));
        s.slots[28] = Some(Stack::new("ntm:blades_titanium", 1));
    });
    g.run(2 * shredder::TICKS as usize);
    assert_eq!(shredder_at(&mut g).slots[0], Some(Stack::new("minecraft:iron_ingot", 2)));
    g.world.borrow_mut().set(OW, AT, "minecraft:air");
    g.break_block(AT);
    g.run(60);
    assert_eq!(g.world.borrow().dropped.len(), 3, "{:?}", g.world.borrow().dropped);
}
