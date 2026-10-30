//! The excavator's rings and switches in the fake world (docs/maquinas.md,
//! "Bedrock"; `TileEntityMachineExcavator`): the effect upgrade's wider
//! rings, the walls, the vein miner, the crusher and silk touch.
use super::*;
use crate::excavator::{Excavator, MAX_POWER, slot};
use crate::server::container_bridge::{Slots, Stack};

/// The fake world's overworld: bedrock at −64, deepslate −63..−51.
const CORE: Pos = [4, -50, 3];

fn excavator(g: &mut Game, drill: &str, upgrade: Option<&str>) {
    g.place(CORE, "ntm:machine_excavator[facing=north,honey_level=0]");
    g.world.borrow_mut().engine.insert((OW.to_string(), CORE), (vec![None; slot::COUNT], Vec::new()));
    g.run(2);
    let mut w = g.world.borrow_mut();
    let s = &mut w.engine.get_mut(&(OW.to_string(), CORE)).unwrap().0;
    s[slot::DRILL] = Some(Stack::new(drill, 1));
    s[2] = upgrade.map(|u| Stack::new(u, 1));
}

/// `ticks` with the power full, the drill on and `set` applied.
fn run(g: &mut Game, ticks: u32, set: &dyn Fn(&mut Excavator)) {
    for _ in 0..ticks {
        let r = g.ready();
        if let Some(Data::Excavator(e)) = r.world.machines.get_mut(&(OW.to_string(), CORE)).map(|m| &mut m.data) {
            e.power = MAX_POWER;
            e.enabled = true;
            set(e);
        }
        g.run(1);
    }
}

fn slots(g: &Game) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), CORE)].0.clone()
}

fn count(s: &[Option<Stack>], id: &str) -> u32 {
    s.iter().flatten().filter(|x| x.id == id).map(|x| x.count).sum()
}

fn at(g: &Game, dx: i32, y: i32, dz: i32) -> String {
    g.world.borrow().get(OW, [CORE[0] + dx, y, CORE[2] + dz])
}

fn excavator_state(g: &mut Game) -> Excavator {
    match g.machine(CORE).data {
        Data::Excavator(e) => e,
        other => panic!("{other:?}"),
    }
}

/// With the effect upgrade (level 1: 3 rings) the first layer is 3×3 and
/// the next 7×7, ring by ring from the axis.
#[test]
fn the_effect_upgrade_widens_the_shaft_to_7x7() {
    let mut g = game();
    excavator(&mut g, "ntm:drillbit__ferro", Some("ntm:upgrade_effect_1"));
    // The first layer (−54) and two more.
    let mut t = 0;
    while excavator_state(&mut g).depth < 3 && t < 2_000 {
        run(&mut g, 1, &|_| {});
        t += 1;
    }
    assert_eq!(at(&g, 2, -54, 0), "minecraft:deepslate", "a primeira camada é 3×3");
    for y in [-55, -56] {
        assert_eq!(at(&g, 3, y, -3), "minecraft:air", "y {y}");
        assert_eq!(at(&g, 4, y, 0), "minecraft:deepslate", "y {y}");
    }
    assert_eq!(count(&slots(&g), "minecraft:cobbled_deepslate"), 9 + 2 * 49);
    // Without power it draws back up (`targetDepth = 0`).
    let r = g.ready();
    if let Some(Data::Excavator(e)) = r.world.machines.get_mut(&(OW.to_string(), CORE)).map(|m| &mut m.data) {
        e.enabled = false;
    }
    g.run(1);
    assert_eq!(excavator_state(&mut g).depth, 0);
}

/// The walls: fluid round the shaft is walled off always, air only with
/// the switch on; fluid inside is emptied.
#[test]
fn walls_stop_fluid_and_close_the_shaft_with_the_switch_on() {
    for walling in [false, true] {
        let mut g = game();
        {
            let mut w = g.world.borrow_mut();
            w.set(OW, [CORE[0] + 2, -54, CORE[2]], "minecraft:water");
            w.set(OW, [CORE[0] - 2, -54, CORE[2] + 1], "minecraft:air");
            w.set(OW, [CORE[0] + 1, -54, CORE[2] + 1], "minecraft:lava");
        }
        excavator(&mut g, "ntm:drillbit__ferro", None);
        run(&mut g, 40, &move |e| e.walling = walling);
        assert!(excavator_state(&mut g).depth >= 1);
        assert_eq!(at(&g, 2, -54, 0), "ntm:barricade", "a água na borda");
        assert_eq!(at(&g, -2, -54, 1), if walling { "ntm:barricade" } else { "minecraft:air" }, "o ar na borda, paredes {walling}");
        assert_eq!(at(&g, 1, -54, 1), "minecraft:air", "a lava de dentro");
    }
}

/// An iron vein reaching out of the shaft: alone, the drill takes the ore
/// in the shaft and the one just past it; the vein miner (an HSS bit and
/// up) the whole vein, filled with sandbags if the walls are on.
#[test]
fn the_vein_miner_takes_the_whole_vein() {
    let vein = [[1, -54, 0], [2, -54, 0], [3, -54, 0], [3, -55, 0], [3, -55, 1]];
    for (miner, walling) in [(false, false), (true, false), (true, true)] {
        let mut g = game();
        for v in vein {
            g.world.borrow_mut().set(OW, [CORE[0] + v[0], v[1], CORE[2] + v[2]], "minecraft:iron_ore");
        }
        excavator(&mut g, "ntm:drillbit__hss", None);
        run(&mut g, 40, &move |e| (e.vein, e.walling) = (miner, walling));
        let ores = count(&slots(&g), "minecraft:iron_ore");
        let left: Vec<_> = vein.iter().map(|v| at(&g, v[0], v[1], v[2])).collect();
        if !miner {
            assert_eq!(ores, 2, "{left:?}");
            assert_eq!(left[2..], ["minecraft:iron_ore"; 3]);
        } else {
            assert_eq!(ores, 5, "{left:?}");
            let fill = if walling { "ntm:barricade" } else { "minecraft:air" };
            assert_eq!(left[2..], [fill; 3], "paredes {walling}");
        }
    }
}

/// The crusher shreds the drops (deepslate has no recipe: it stays), but
/// not what's turned to dust; silk touch keeps the block itself (a bit
/// with silk only); a sandbag gives nothing.
#[test]
fn crusher_and_silk_touch_change_the_drops() {
    let mut e = Excavator::new();
    let drops = |e: &Excavator, b: &str| e.drops(b).into_iter().map(|s| (s.id, s.count)).collect::<Vec<_>>();
    e.slots[slot::DRILL] = Some(Stack::new("ntm:drillbit__steel", 1));
    assert_eq!(drops(&e, "minecraft:stone"), vec![("minecraft:cobblestone".into(), 1)]);
    e.crusher = true;
    assert_eq!(drops(&e, "minecraft:stone"), vec![("minecraft:gravel".into(), 1)]);
    assert_eq!(drops(&e, "minecraft:iron_ore"), vec![("ntm:powder_iron".into(), 2)]);
    assert_eq!(drops(&e, "minecraft:dirt"), vec![("minecraft:dirt".into(), 1)], "pó (dust) não");
    assert_eq!(drops(&e, "minecraft:deepslate"), vec![("minecraft:cobbled_deepslate".into(), 1)]);
    e.crusher = false;
    e.silk = true;
    assert!(!e.can_silk_touch(), "a broca de aço não tem toque de seda");
    assert_eq!(e.screen_data()[11], 2, "lâmpada âmbar");
    e.slots[slot::DRILL] = Some(Stack::new("ntm:drillbit__steel_diamond", 1));
    assert_eq!(e.screen_data()[11], 1);
    assert_eq!(drops(&e, "minecraft:stone"), vec![("minecraft:stone".into(), 1)]);
    assert_eq!(drops(&e, "minecraft:grass_block[snowy=false]"), vec![("minecraft:grass_block".into(), 1)]);
    assert!(drops(&e, "ntm:barricade").is_empty());
}

/// The switches are buttons 1–4, kept in the save; the upgrades change
/// the power drawn and the bit's speed.
#[test]
fn switches_are_saved_and_upgrades_change_power_and_speed() {
    use crate::server::container_bridge::Slotted;
    let mut e = Excavator::new();
    for b in 1..=4 {
        e.button(b);
    }
    let mut back = Excavator::new();
    back.load_state(&e.save_state());
    assert_eq!((back.crusher, back.walling, back.vein, back.silk), (true, true, true, true));
    assert_eq!(e.screen_data().len(), 13);
    // Speed 2 and power 1: 10 000 × 3 ÷ 2; the bit ×2.
    e.slots[2] = Some(Stack::new("ntm:upgrade_speed_2", 1));
    e.slots[3] = Some(Stack::new("ntm:upgrade_power_1", 1));
    assert_eq!(e.consumption(), 15_000);
    let d = crate::excavator::drill("ntm:drillbit__steel").unwrap();
    assert_eq!(e.speed(d), 2.0);
    // Two effect upgrades add up, to 3 at most.
    e.slots[2] = Some(Stack::new("ntm:upgrade_effect_3", 1));
    e.slots[3] = Some(Stack::new("ntm:upgrade_effect_2", 1));
    assert_eq!(e.radius(), 7);
}
