//! T02: the RBMK columns as blocks. Each stands as tall as it was placed (the dial `dialColumnHeight`, kept with the
//! world and given to the engine); `/ntmrbmk migrate` rebuilds standing ones; an orphaned dummy goes; a lid goes on by
//! hand in one transfer and comes off with a screwdriver as one item owed with the change; the eighteen columns are
//! adopted, only T01's step.
use super::*;
use crate::rbmk::persist::{self, Lid, RbmkColumn};
use container_bridge::Stack;

const Y: i32 = 71;
const DUMMY: &str = "ntm:dummy_rbmk";

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

/// A column as the engine places it: its core and `h - 1` dummies above.
fn stand(g: &mut Game, core: Pos, id: &str, h: i32) {
    g.place(core, &column(id));
    for dy in 1..h {
        g.place([core[0], core[1] + dy, core[2]], DUMMY);
    }
}

fn hold(g: &mut Game, item: &str, n: u32, creative: bool) {
    let stack = Stack::new(item, n);
    let held = HeldStack { encoded: format!("{stack:?}"), stack };
    g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Inventory(0)), inventory: vec![Some(held), None], creative, ..Default::default() });
}

fn in_hand(g: &Game) -> Option<Stack> {
    g.world.borrow().hands.as_ref().and_then(|h| h.held().map(|s| s.stack.clone()))
}

fn rbmk(g: &mut Game, p: Pos) -> RbmkColumn {
    let Data::Rbmk(c) = g.machine(p).data else { panic!("{p:?} is not an RBMK column") };
    *c
}

fn command(g: &mut Game, line: &str) -> Result<String, String> {
    let reply = g.server.native_rbmk_command(&mut g.sh, &player(), 2, line).expect("an ntmrbmk command");
    g.world.borrow_mut().apply_writes();
    reply
}

fn height(g: &Game) -> Option<i32> {
    g.world.borrow().column_height
}

fn dropped(g: &Game) -> Vec<String> {
    g.world.borrow().dropped.clone()
}

/// The server opened again on the same storage, after a save.
fn reopen(g: &mut Game) {
    g.run(1);
    g.server.native_world_saved(&mut g.sh, true);
    let world = g.world.clone();
    *g = Game::new(world);
}

#[test]
fn the_dials_are_created_kept_and_set_as_java_game_rules() {
    let mut g = game();
    // `createDials` as the server starts: the 25 rules at their defaults, the engine told 4 cells.
    assert_eq!(height(&g), Some(4));
    assert_eq!(g.ready().world.rbmk_rules.string("dialColumnHeight"), "4");
    assert_eq!(g.ready().world.rbmk_rules.string("dialSaveDials"), "true");
    assert!(command(&mut g, "ntmrbmk dial").unwrap().lines().count() == 25);
    // Operator level 2 only, as `/gamerule`; a rule the world has not is refused.
    assert!(g.server.native_rbmk_command(&mut g.sh, &player(), 1, "ntmrbmk dial dialColumnHeight 6").unwrap().is_err());
    assert!(command(&mut g, "ntmrbmk dial dialNoSuchRule 6").is_err());
    assert_eq!(height(&g), Some(4));
    // `getClampedInt(2, 16)`: `Integer.parseInt` or the default 4, clamped.
    for (value, h) in [("6", 6), ("1", 2), ("-3", 2), ("99", 16), ("+7", 7), ("abc", 4), ("6.0", 4), ("", 4), ("16", 16), ("2", 2)] {
        assert!(command(&mut g, &format!("ntmrbmk dial dialColumnHeight {value}")).is_ok() || value.is_empty());
        if !value.is_empty() {
            assert_eq!(height(&g), Some(h), "{value:?}");
        }
    }
    command(&mut g, "ntmrbmk dial dialColumnHeight 6").unwrap();
    assert_eq!(command(&mut g, "ntmrbmk dial dialColumnHeight").unwrap(), "dialColumnHeight = 6");
    // Kept with the world: the server opened again reads 6.
    reopen(&mut g);
    assert_eq!(height(&g), Some(6));
    assert_eq!(g.ready().world.rbmk_rules.string("dialColumnHeight"), "6");
    // `dialSaveDials` false: the next start writes the defaults back, the cache keeps the old values until a refresh
    // (`createDials` refreshes before it writes, D2).
    command(&mut g, "ntmrbmk dial dialSaveDials false").unwrap();
    reopen(&mut g);
    assert_eq!(height(&g), Some(6), "the cache from before the reset");
    assert_eq!(g.ready().world.rbmk_rules.string("dialColumnHeight"), "4");
    command(&mut g, "ntmrbmk dial dialFluxRange 5").unwrap();
    assert_eq!(height(&g), Some(4), "refreshed");
}

#[test]
fn a_column_is_found_from_any_cell_and_rebuilt_only_on_demand() {
    let mut g = game();
    stand(&mut g, [0, Y, 0], "rbmk_blank", 4);
    stand(&mut g, [0, Y + 4, 0], "rbmk_rod", 2);
    stand(&mut g, [2, Y, 0], "rbmk_moderator", 16);
    g.run(2);
    for y in Y..Y + 4 {
        assert_eq!(rbmk_columns::column_core(&mut g.sh, OW, [0, y, 0]), Some([0, Y, 0]), "y {y}");
    }
    assert_eq!(rbmk_columns::column_core(&mut g.sh, OW, [0, Y + 5, 0]), Some([0, Y + 4, 0]));
    assert_eq!(rbmk_columns::column_core(&mut g.sh, OW, [2, Y + 15, 0]), Some([2, Y, 0]));
    assert_eq!(rbmk_columns::column_core(&mut g.sh, OW, [0, Y + 6, 0]), None, "air");
    assert_eq!(rbmk_columns::standing_height(&mut g.sh, OW, [2, Y, 0]), 16);
    // The dial changes the columns placed from then on, never the standing ones.
    command(&mut g, "ntmrbmk dial dialColumnHeight 6").unwrap();
    assert_eq!(rbmk_columns::standing_height(&mut g.sh, OW, [0, Y, 0]), 4);
    // The 4 high column cannot grow into the column above it: refused, nothing written.
    let reply = command(&mut g, &format!("ntmrbmk migrate 0 {Y} 0")).unwrap();
    assert!(reply.contains("0 refeitas") && reply.contains("1 recusadas") && reply.contains(&format!("[0, {}, 0]", Y + 4)), "{reply}");
    assert_eq!(rbmk_columns::standing_height(&mut g.sh, OW, [0, Y, 0]), 4);
    // A box: the 2 high one grows to 6, the 16 high one shrinks to 6, the 4 high one is still blocked.
    let reply = command(&mut g, &format!("ntmrbmk migrate 0 {Y} 0 2 {} 0", Y + 15)).unwrap();
    assert!(reply.contains("2 refeitas") && reply.contains("1 recusadas"), "{reply}");
    assert_eq!(rbmk_columns::standing_height(&mut g.sh, OW, [0, Y + 4, 0]), 6);
    assert_eq!(rbmk_columns::standing_height(&mut g.sh, OW, [2, Y, 0]), 6);
    assert_eq!(g.world.borrow().get(OW, [2, Y + 6, 0]), "minecraft:air");
    // Again: nothing more to do.
    let reply = command(&mut g, &format!("ntmrbmk migrate 2 {} 0", Y + 3)).unwrap();
    assert!(reply.contains("0 refeitas") && reply.contains("1 já"), "{reply}");
    // A cell not loaded: refused, nothing written.
    stand(&mut g, [20, Y, 20], "rbmk_blank", 2);
    g.world.borrow_mut().unloaded.insert((OW.into(), 1, 1));
    let before = g.world.borrow().blocks.clone();
    let reply = command(&mut g, &format!("ntmrbmk migrate 0 {Y} 0 31 {Y} 31")).unwrap();
    assert!(reply.contains("recusadas"), "{reply}");
    assert_eq!(g.world.borrow().blocks, before);
    assert!(command(&mut g, "ntmrbmk migrate 0 0 0 100 100 100").is_err(), "a box too big");
}

#[test]
fn an_orphaned_dummy_goes_up_to_the_end_of_its_run() {
    let mut g = game();
    stand(&mut g, [0, Y, 0], "rbmk_blank", 6);
    stand(&mut g, [3, Y, 0], "rbmk_blank", 6);
    stand(&mut g, [6, Y, 0], "rbmk_blank", 4);
    g.run(2);
    // The core taken away by a command: all its dummies go.
    g.place([0, Y, 0], "minecraft:stone");
    g.run(2);
    for y in Y + 1..Y + 6 {
        assert_eq!(g.world.borrow().get(OW, [0, y, 0]), "minecraft:air", "y {y}");
    }
    // A middle dummy taken away: the run above it goes, the column below stands, shorter.
    g.place([3, Y + 2, 0], "minecraft:air");
    g.run(2);
    assert_eq!(rbmk_columns::standing_height(&mut g.sh, OW, [3, Y, 0]), 2);
    for y in Y + 3..Y + 6 {
        assert_eq!(g.world.borrow().get(OW, [3, y, 0]), "minecraft:air", "y {y}");
    }
    // A column whose dummies stand on another column's top: they are not orphans.
    stand(&mut g, [6, Y + 4, 0], "rbmk_rod", 2);
    g.place([6, Y + 3, 0], "minecraft:air");
    g.run(2);
    assert_eq!(g.world.borrow().get(OW, [6, Y + 5, 0]), DUMMY);
    assert_eq!(rbmk_columns::standing_height(&mut g.sh, OW, [6, Y, 0]), 3);
}

#[test]
fn a_lid_goes_on_by_hand_and_comes_off_with_a_screwdriver_once() {
    let mut g = game();
    let core = [0, Y, 0];
    stand(&mut g, core, "rbmk_rod", 4);
    stand(&mut g, [3, Y, 0], "rbmk_control", 4);
    g.run(2);
    // Not a lid: the click goes on. A control rod has its own lid: the menu opens (`openInv`).
    hold(&mut g, "minecraft:stone", 1, false);
    assert!(!g.server.native_rbmk_lid(&mut g.sh, OW, core, core, "p1"));
    hold(&mut g, "ntm:rbmk_lid", 2, false);
    assert!(!g.server.native_rbmk_lid(&mut g.sh, OW, [3, Y + 2, 0], [3, Y, 0], "p1"));
    // Refused after its checks: neither side moves.
    g.world.borrow_mut().refuse_commits = true;
    assert!(g.server.native_rbmk_lid(&mut g.sh, OW, [0, Y + 3, 0], core, "p1"));
    assert_eq!((rbmk(&mut g, core).lid, in_hand(&g).map(|s| s.count)), (Lid::None, Some(2)));
    g.world.borrow_mut().refuse_commits = false;
    // From the top cell: one lid on, one out of the hand, with the concrete's sound at the cell used.
    assert!(g.server.native_rbmk_lid(&mut g.sh, OW, [0, Y + 3, 0], core, "p1"));
    assert_eq!((rbmk(&mut g, core).lid, in_hand(&g).map(|s| s.count)), (Lid::Standard, Some(1)));
    assert!(g.world.borrow().shown.iter().any(|(_, s)| *s == crate::blast::visuals::Show::Sound { id: "minecraft:block.stone.place", source: "block", at: [0.5, f64::from(Y + 3) + 0.5, 0.5], volume: 1., pitch: 0.8 }));
    assert!(rbmk(&mut g, core).has_lid("ntm:rbmk_rod"));
    // The model shows it: the first value its block entity sends (`lid_cover`).
    assert_eq!(g.world.borrow().engine[&(OW.to_string(), core)].1[0], 1);
    // A lid on it: no second one (the menu opens).
    hold(&mut g, "ntm:rbmk_lid_glass", 1, false);
    assert!(!g.server.native_rbmk_lid(&mut g.sh, OW, core, core, "p1"));
    assert_eq!(in_hand(&g).map(|s| s.count), Some(1));
    // Something else than the screwdriver held: the publication is refused, the lid stays.
    hold(&mut g, "minecraft:stone", 1, false);
    assert!(!g.server.native_rbmk_screw(&mut g.sh, OW, core, "ntm:screwdriver"));
    assert_eq!(rbmk(&mut g, core).lid, Lid::Standard);
    // The screwdriver: the lid off as one item at the top as the dial reads (4 cells: core + 3), the tool worn.
    hold(&mut g, "ntm:screwdriver", 1, false);
    assert!(g.server.native_rbmk_screw(&mut g.sh, OW, core, "ntm:screwdriver"));
    assert_eq!(rbmk(&mut g, core).lid, Lid::None);
    assert_eq!(dropped(&g), vec![format!("{OW} 0 {} 0 ntm:rbmk_lid 1", Y + 3)]);
    assert_eq!(g.world.borrow().worn, vec!["ntm:screwdriver".to_string()]);
    // Nothing to take off: no wear, no drop. A control rod's own lid never comes off.
    assert!(!g.server.native_rbmk_screw(&mut g.sh, OW, core, "ntm:screwdriver"));
    assert!(!g.server.native_rbmk_screw(&mut g.sh, OW, [3, Y, 0], "ntm:screwdriver"));
    assert_eq!(g.world.borrow().worn.len(), 1);
    // The glass lid, creative: the hand keeps it; its sound is the glass's (`step.stone`). Off again: a second drop
    // under a name of its own (the same column, the same generation).
    hold(&mut g, "ntm:rbmk_lid_glass", 1, true);
    assert!(g.server.native_rbmk_lid(&mut g.sh, OW, core, core, "p1"));
    assert_eq!((rbmk(&mut g, core).lid, in_hand(&g).map(|s| s.count)), (Lid::Glass, Some(1)));
    assert_eq!(g.world.borrow().engine[&(OW.to_string(), core)].1[0], 2, "lid_glass");
    assert!(g.world.borrow().shown.iter().any(|(_, s)| matches!(s, crate::blast::visuals::Show::Sound { id: "minecraft:block.stone.step", .. })));
    hold(&mut g, "ntm:screwdriver_desh", 1, false);
    assert!(g.server.native_rbmk_screw(&mut g.sh, OW, core, "ntm:screwdriver_desh"));
    assert_eq!(dropped(&g).last().unwrap(), &format!("{OW} 0 {} 0 ntm:rbmk_lid_glass 1", Y + 3));
    assert_eq!(dropped(&g).len(), 2);
    // The drop goes where the dial says now, not where the column ends (Java's `getColumnHeight`).
    command(&mut g, "ntmrbmk dial dialColumnHeight 9").unwrap();
    hold(&mut g, "ntm:rbmk_lid", 1, false);
    assert!(g.server.native_rbmk_lid(&mut g.sh, OW, core, core, "p1"));
    hold(&mut g, "ntm:screwdriver", 1, false);
    assert!(g.server.native_rbmk_screw(&mut g.sh, OW, core, "ntm:screwdriver"));
    assert_eq!(dropped(&g).last().unwrap(), &format!("{OW} 0 {} 0 ntm:rbmk_lid 1", Y + 8));
    // Kept with the block entity: the server opened again reads the lid.
    hold(&mut g, "ntm:rbmk_lid", 1, false);
    assert!(g.server.native_rbmk_lid(&mut g.sh, OW, core, core, "p1"));
    reopen(&mut g);
    g.run(2);
    assert_eq!(rbmk(&mut g, core).lid, Lid::Standard);
}

#[test]
fn a_broken_column_drops_its_lid_once_at_the_top_as_the_dial_reads() {
    let mut g = game();
    let core = [0, Y, 0];
    stand(&mut g, core, "rbmk_blank", 4);
    g.run(2);
    hold(&mut g, "ntm:rbmk_lid", 1, false);
    assert!(g.server.native_rbmk_lid(&mut g.sh, OW, core, core, "p1"));
    command(&mut g, "ntmrbmk dial dialColumnHeight 5").unwrap();
    g.break_block(core);
    g.run(3);
    assert_eq!(dropped(&g), vec![format!("{OW} 0 {} 0 ntm:rbmk_lid 1", Y + 4)]);
    // A column without a lid drops none.
    stand(&mut g, [3, Y, 0], "rbmk_blank", 4);
    g.run(2);
    g.break_block([3, Y, 0]);
    g.run(3);
    assert_eq!(dropped(&g).len(), 1);
}

#[test]
fn the_eighteen_columns_are_adopted_and_only_t01s_step() {
    let mut g = game();
    for (i, id) in persist::COLUMNS.iter().enumerate() {
        g.place([2 * i as i32 - 18, Y, 4], &column(id.strip_prefix("ntm:").unwrap()));
    }
    g.run(3);
    for (i, id) in persist::COLUMNS.iter().enumerate() {
        let c = rbmk(&mut g, [2 * i as i32 - 18, Y, 4]);
        assert_eq!(c.slots.len(), persist::slot_count(id), "{id}");
        assert_eq!(usize::from(c.screen), persist::screen_len(id), "{id}");
        assert_eq!(c.has_lid(id), persist::has_own_lid(id), "{id}");
    }
    let members: Vec<String> = g.ready().world.rbmk.values().flat_map(|l| l.members.values().map(|m| m.block.clone())).collect();
    assert_eq!(members.len(), 8, "{members:?}");
    assert!(members.iter().all(|b| persist::kind_of(b).is_some()));
    // A save of T01 (no lid, no screen) reads as before.
    let json = r#"{"schema":2,"heat":4625196817309925376,"reasim_water":0,"reasim_steam":0,"flux_quantity":0,"flux_fast_ratio":0,"last_flux_quantity":0,"last_flux_ratio":0,"has_rod":false,"slots":[null]}"#;
    let old: RbmkColumn = serde_json::from_str(json).unwrap();
    assert_eq!((old.lid, old.screen), (Lid::None, 0));
    assert_eq!(container_bridge::Slotted::screen_data(&old).len(), 7);
}

#[test]
fn automation_reaches_the_columns_as_java_lets_it() {
    use crate::native::{slot_allows, SlotAction::*, SlotOrigin::*};
    let rod = Stack::new("ntm:rbmk_fuel_ueu", 1);
    let stone = Stack::new("minecraft:stone", 1);
    let none: Vec<Option<Stack>> = vec![None; 12];
    // The fuel channels and the heater show no slot (`TileEntityRBMKSlottedBase`); the hand is not restricted here.
    for b in ["ntm:rbmk_rod", "ntm:rbmk_rod_reasim_mod", "ntm:rbmk_heater"] {
        assert!(!slot_allows(b, &none, 0, &rod, Insert, Automation) && !slot_allows(b, &none, 0, &rod, Extract, Automation), "{b}");
        assert!(slot_allows(b, &none, 0, &rod, Insert, Manual) && slot_allows(b, &none, 0, &rod, Extract, Manual), "{b}");
    }
    // The storage takes rods only and gives anything (`TileEntityRBMKStorage`).
    assert!(slot_allows("ntm:rbmk_storage", &none, 5, &rod, Insert, Automation));
    assert!(!slot_allows("ntm:rbmk_storage", &none, 5, &stone, Insert, Automation));
    assert!(slot_allows("ntm:rbmk_storage", &none, 11, &stone, Extract, Automation));
    // The outgasser gives its output only; its input waits for T09's recipes.
    assert!(slot_allows("ntm:rbmk_outgasser", &none, 1, &stone, Extract, Automation));
    assert!(!slot_allows("ntm:rbmk_outgasser", &none, 0, &stone, Extract, Automation));
    assert!(!slot_allows("ntm:rbmk_outgasser", &none, 0, &stone, Insert, Automation));
}

#[test]
fn a_retained_drop_of_an_earlier_version_reads_without_a_name() {
    let guard = CellGuard { dim: OW.into(), pos: [0, 1, 2], generation: 3, state: "ntm:x".into() };
    let d = RetainedDrop { origin: guard, items: vec!["{id:\"minecraft:stone\",count:1}".into()], id: None };
    let json = serde_json::to_string(&d).unwrap();
    assert!(!json.contains("\"id\":null"), "{json}");
    assert_eq!(serde_json::from_str::<RetainedDrop>(&json).unwrap(), d);
}
