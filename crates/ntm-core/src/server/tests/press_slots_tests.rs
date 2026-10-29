//! The presses on the engine's slots (patch `ntm-visual`): their fields
//! (moved by transactions on an engine without slots) mirror the slots.
use super::*;
use crate::press::slot;
use crate::server::container_bridge::{Slots, Stack};

const AT: Pos = [4, 70, 0];

fn engine(g: &mut Game, n: usize) {
    g.world.borrow_mut().engine.insert((OW.to_string(), AT), (vec![None; n], Vec::new()));
}

fn slots(g: &Game) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), AT)].0.clone()
}

fn put(g: &mut Game, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0[i] = s;
}

fn count(s: &[Option<Stack>], id: &str) -> u32 {
    s.iter().flatten().filter(|x| x.id == id).map(|x| x.count).sum()
}

/// A burner press filled by transactions before the slots (a stamp worn
/// 10, ingots, coal): seen with empty engine slots, its items go to them,
/// the wear as the stamp's damage.
#[test]
fn a_press_filled_before_the_slots_shows_its_items_in_them() {
    let mut g = game();
    g.place(AT, "ntm:machine_press[facing=north,honey_level=0]");
    g.run(2);
    {
        let r = g.ready();
        let Some(Data::BurnerPress(p)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
        p.core.stamp = Some(crate::press::Stamp { item: "ntm:stamp_iron_plate".into(), used: 10 });
        p.core.input = Some(("minecraft:iron_ingot".into(), 5));
        p.fuel = Some(("minecraft:coal".into(), 3));
    }
    engine(&mut g, 4);
    g.run(2);
    let s = slots(&g);
    assert_eq!(s[slot::STAMP], Some(Stack { id: "ntm:stamp_iron_plate".into(), count: 1, damage: 10 }));
    assert_eq!(count(&s, "minecraft:iron_ingot") + count(&s, "ntm:plate_iron"), 5);
    assert!(count(&s, "minecraft:coal") <= 3 && count(&s, "minecraft:coal") >= 2, "{s:?}");
}

/// Through the slots only: a stamp worn 10, 8 ingots, coal; the player
/// takes the plates at odd times. Ingots + plates are always 8; the stamp
/// wears one a pressing, in its damage.
#[test]
fn a_burner_press_works_from_its_slots_and_wears_the_stamp() {
    let mut g = game();
    g.place(AT, "ntm:machine_press[facing=north,honey_level=0]");
    engine(&mut g, 4);
    g.run(2);
    put(&mut g, slot::STAMP, Some(Stack { id: "ntm:stamp_iron_plate".into(), count: 1, damage: 10 }));
    put(&mut g, slot::INPUT, Some(Stack::new("minecraft:iron_ingot", 8)));
    put(&mut g, slot::FUEL, Some(Stack::new("minecraft:coal", 4)));
    let mut taken = 0;
    for t in 0..3000 {
        g.run(1);
        let s = slots(&g);
        assert_eq!(count(&s, "minecraft:iron_ingot") + count(&s, "ntm:plate_iron") + taken, 8, "tick {t}: {s:?}");
        if t % 41 == 7 && s[slot::OUTPUT].is_some() {
            taken += s[slot::OUTPUT].as_ref().unwrap().count;
            put(&mut g, slot::OUTPUT, None);
        }
    }
    let s = slots(&g);
    assert_eq!(count(&s, "minecraft:iron_ingot"), 0, "não terminou: {s:?}");
    assert_eq!(s[slot::STAMP].as_ref().map(|x| x.damage), Some(18));
}

/// The stamp taken out and put back: the wear comes back with it; a worn
/// out stamp breaks in its slot.
#[test]
fn a_stamps_wear_goes_out_and_back_with_it() {
    let mut g = game();
    g.place(AT, "ntm:machine_epress[facing=north,honey_level=0]");
    engine(&mut g, 5);
    g.run(2);
    {
        let r = g.ready();
        let Some(Data::ElectricPress(p)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
        p.power = crate::press::ElectricPress::MAX;
    }
    // The battery and upgrade slots hold what the player put there.
    put(&mut g, slot::BATTERY, Some(Stack::new("ntm:battery_spark", 1)));
    put(&mut g, slot::UPGRADE, Some(Stack::new("minecraft:dirt", 1)));
    put(&mut g, slot::STAMP, Some(Stack { id: "ntm:stamp_iron_wire".into(), count: 1, damage: 62 }));
    put(&mut g, slot::INPUT, Some(Stack::new("ntm:ingot_copper", 5)));
    g.run(400);
    let s = slots(&g);
    assert_eq!(s[slot::STAMP], None, "gasta em 64");
    assert_eq!(count(&s, "ntm:wire_fine__copper"), 16);
    assert_eq!(count(&s, "ntm:ingot_copper"), 3);
    assert_eq!((s[slot::BATTERY].clone(), s[slot::UPGRADE].clone()), (Some(Stack::new("ntm:battery_spark", 1)), Some(Stack::new("minecraft:dirt", 1))));
    // A stamp worn 30 back in: it presses 34 more.
    put(&mut g, slot::STAMP, Some(Stack { id: "ntm:stamp_iron_wire".into(), count: 1, damage: 30 }));
    g.run(100);
    let s = slots(&g);
    assert!(s[slot::STAMP].as_ref().is_some_and(|x| x.damage > 30), "{s:?}");
}

/// The gas centrifuge on the slots: what it had (a product, the upgrade)
/// goes to them; with UF6 and power it fills the output slots, a product
/// taken leaves the others where they are, and the upgrade comes out of
/// its slot.
#[test]
fn a_gas_centrifuge_keeps_its_products_and_upgrade_in_slots() {
    use crate::gascent::{GasCent, MAX_POWER, UPGRADE, slot as gs};
    let mut g = game();
    g.place(AT, "ntm:machine_gascent[facing=north,honey_level=0]");
    g.run(2);
    let edit = |g: &mut Game, f: &dyn Fn(&mut GasCent)| {
        let r = g.ready();
        let Some(Data::GasCent(c)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
        f(c);
    };
    edit(&mut g, &|c| {
        c.out = vec![("ntm:nugget_u238".into(), 3)];
        c.upgrade = true;
    });
    engine(&mut g, gs::COUNT);
    g.run(2);
    let s = slots(&g);
    assert_eq!(s[0], Some(Stack::new("ntm:nugget_u238", 3)));
    assert_eq!(s[gs::UPGRADE], Some(Stack::new(UPGRADE, 1)));
    // Enriching: UF6 in its tank, power every tick.
    edit(&mut g, &|c| {
        c.tank.fluid = "uf6".into();
        c.tank.mb = 2_000;
    });
    for _ in 0..(4 * crate::gascent::SPEED_UPGRADED) {
        edit(&mut g, &|c| c.power = MAX_POWER);
        g.run(1);
    }
    let s = slots(&g);
    assert!(count(&s, "ntm:nugget_u238") > 3, "{s:?}");
    // The first slot taken: the others stay; the upgrade taken out.
    let others: Vec<_> = s[1..4].to_vec();
    put(&mut g, 0, None);
    put(&mut g, gs::UPGRADE, None);
    g.run(2);
    let s = slots(&g);
    assert_eq!(s[1..4].to_vec(), others);
    match g.machine(AT).data {
        Data::GasCent(c) => assert!(!c.upgrade),
        other => panic!("{other:?}"),
    }
}
