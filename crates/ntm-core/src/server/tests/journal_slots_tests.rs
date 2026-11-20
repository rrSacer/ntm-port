//! The P2 of the Onda 2 integration's review: a machine the journal restored (its chunk older than the journal) takes
//! its block entity's slots whenever its record has none (`Slotted::slots_recorded`), not only the sawmill. Each test
//! here restores one of the machines whose record skips its slots, with an item in the block entity, and reads that the
//! item is still there after the bridge's next steps (the old rule wrote the restored model's empty slots over it).
use super::*;
use crate::server::container_bridge::Stack;
use crate::server::Survivor;

const FACING: &str = "[facing=north,honey_level=0]";
const CORE: Pos = [0, 70, 0];

/// `block` placed, an item in its block entity's first slot, then the world opened again from a chunk older than the
/// journal: the engine's slots afterwards.
fn restored_slots(block: &str) -> (Vec<Option<Stack>>, Vec<Option<Stack>>) {
    let mut g = game();
    g.place(CORE, &format!("{block}{FACING}"));
    g.run(2);
    let key = (OW.to_string(), CORE);
    let mut slots = g.world.borrow().engine.get(&key).unwrap_or_else(|| panic!("{block} has no block entity")).0.clone();
    assert!(!slots.is_empty(), "{block} has slots");
    slots[0] = Some(Stack::new("minecraft:stick", 3));
    g.world.borrow_mut().engine.get_mut(&key).unwrap().0 = slots.clone();
    g.run(1);
    // The chunk as last saved, then a journal step newer than it (the same machine, one revision on).
    let (old_json, old_slots) = (g.world.borrow().saved[&key].clone(), g.world.borrow().engine[&key].0.clone());
    assert_eq!(old_slots[0], Some(Stack::new("minecraft:stick", 3)), "{block} keeps the item before the failure");
    let mut m = g.machine(CORE);
    m.rev += 1;
    let survivor = Survivor { dim: OW.into(), pos: CORE, generation: m.generation, rev: m.rev, json: serde_json::to_string(&m).unwrap() };
    g.ready().world.survivors.insert(key.clone(), survivor);
    let earlier = g.ready().world.kept_as_an_earlier_version(0);
    let world = Rc::new(RefCell::new(g.world.borrow().clone()));
    world.borrow_mut().kv.set(crate::persist::KEY, &earlier).unwrap();
    world.borrow_mut().saved.insert(key.clone(), old_json);
    world.borrow_mut().engine.get_mut(&key).unwrap().0 = old_slots.clone();
    let mut reopened = Game::new(world);
    assert!(reopened.server.ready.restored.contains(&key), "{block}: restored from the journal");
    reopened.run(6);
    assert!(reopened.server.ready.bridge.last.contains_key(&key), "{block}: the bridge has seen it");
    let after = reopened.world.borrow().engine[&key].0.clone();
    (old_slots, after)
}

macro_rules! restored_keeps_its_block_entity_slots {
    ($($name:ident: $block:expr),* $(,)?) => {$(
        #[test]
        fn $name() {
            let (before, after) = restored_slots($block);
            assert_eq!(after, before, "{}: the block entity's item survives the restore", $block);
        }
    )*};
}

restored_keeps_its_block_entity_slots! {
    an_arc_furnace_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_arc_furnace_large",
    a_coker_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_coker",
    an_electrolyser_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_electrolyser",
    a_diesel_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_diesel",
    a_combustion_engine_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_combustion_engine",
    a_turbofan_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_turbofan",
    a_gas_turbine_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_turbinegas",
    an_ore_slopper_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_ore_slopper",
    a_battery_socket_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_battery_socket",
    a_battery_redd_restored_from_the_journal_keeps_its_block_entitys_slots: "ntm:machine_battery_redd",
}
