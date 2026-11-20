//! T05: a fuel channel's menu (`ContainerRBMKRod`) takes or gives nothing in survival while its rod's hull is above
//! 200 °C, every gesture alike (`slotClick` on index 0, `transferStackInSlot` from it); automation still sees no slot;
//! the other columns' menus are not touched. The creative bypass is the menu's (`ModSlotFilter::creative_bypass`).
use super::*;
use crate::native::{slot_allows, SlotAction::*, SlotOrigin::*};
use container_bridge::Stack;

const Y: i32 = 71;

fn rod(hull: f64) -> Stack {
    let mut s = Stack::new("ntm:rbmk_fuel_ueu", 1);
    s.custom = Some(format!("{{hull:{}d}}", crate::snbt::java_double(hull)));
    s.tagged = true;
    s
}

const CHANNELS: [&str; 4] = ["ntm:rbmk_rod", "ntm:rbmk_rod_mod", "ntm:rbmk_rod_reasim", "ntm:rbmk_rod_reasim_mod"];

#[test]
fn a_channel_menu_follows_its_rods_hull() {
    let stone = Stack::new("minecraft:stone", 1);
    for b in CHANNELS {
        for (hull, open) in [(20.0, true), (200.0, true), (200.000_000_1, false), (1_200.0, false), (f64::NAN, false)] {
            let slots = vec![Some(rod(hull))];
            for (item, action) in [(&rod(hull), Extract), (&stone, Insert), (&rod(20.0), Insert)] {
                assert_eq!(slot_allows(b, &slots, 0, item, action, Manual), open, "{b} {hull} {action:?}");
                assert!(!slot_allows(b, &slots, 0, item, action, Automation), "{b} {hull} {action:?}");
            }
        }
        // An empty channel takes anything by hand (a vanilla `Slot`); another item in it is never hot.
        let hot_stone = Stack { custom: Some("{hull:9000.0d}".into()), ..stone.clone() };
        for slots in [vec![None], vec![Some(hot_stone)]] {
            assert!(slot_allows(b, &slots, 0, &stone, Insert, Manual) && slot_allows(b, &slots, 0, &stone, Extract, Manual), "{b}");
        }
    }
}

#[test]
fn the_other_columns_menus_ignore_a_hot_rod() {
    let slots = vec![Some(rod(1_200.0)); 12];
    for b in ["ntm:rbmk_heater", "ntm:rbmk_storage", "ntm:rbmk_boiler", "ntm:rbmk_blank"] {
        assert!(slot_allows(b, &slots, 0, &rod(1_200.0), Extract, Manual), "{b}");
        assert!(slot_allows(b, &slots, 0, &rod(1_200.0), Insert, Manual), "{b}");
    }
}

fn hold(g: &mut Game, stack: Stack, creative: bool) {
    let held = HeldStack { encoded: format!("{stack:?}"), stack, plain: true };
    g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Inventory(0)), inventory: vec![Some(held), None], creative, ..Default::default() });
}

fn sounds(g: &Game) -> usize {
    g.world.borrow().shown.iter().filter(|(_, s)| matches!(s, crate::blast::visuals::Show::Sound { id: "ntm:item.upgradeplug", .. })).count()
}

#[test]
fn a_hot_rod_goes_in_by_hand_and_only_the_accepted_branch_sounds() {
    // `RBMKRod.onBlockActivated` asks only for an empty channel and a rod in hand: not the rod's heat.
    let mut g = game();
    let at = [0, Y, 0];
    g.place(at, "ntm:rbmk_rod[facing=north,honey_level=0]");
    g.run(2);
    let mut hot = rod(1_500.0);
    hot.count = 2;
    hold(&mut g, hot.clone(), false);
    g.world.borrow_mut().refuse_commits = true;
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, at, at, "p1"));
    assert_eq!(sounds(&g), 0, "a refused transfer is silent");
    g.world.borrow_mut().refuse_commits = false;
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, at, at, "p1"));
    let slot = g.world.borrow().engine[&(OW.to_string(), at)].0[0].clone().expect("the rod went in");
    assert_eq!((slot.count, slot.custom.as_deref()), (1, hot.custom.as_deref()), "one of the stack, its tag whole");
    assert_eq!(sounds(&g), 1);
    // A full channel: the click goes on to the menu, silent, the hand kept.
    assert!(!g.server.native_rbmk_use(&mut g.sh, OW, at, at, "p1"));
    assert_eq!(sounds(&g), 1);
    let held = g.world.borrow().hands.as_ref().and_then(|h| h.held().map(|s| s.stack.count));
    assert_eq!(held, Some(1));
}
