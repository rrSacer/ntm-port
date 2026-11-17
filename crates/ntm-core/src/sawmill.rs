//! The sawmill (`TileEntitySawmill`, `MachineSawmill`), Onda 2. Its record keeps what the Java's NBT keeps: the three
//! slots (`TileEntityMachineBase`), `hasBlade` and `progress` (`readFromNBT`, 190-200). `heat`, `overspeed` and
//! `warnCooldown` are not saved: a reopened sawmill counts its overspeed from 0 again (R10 of the plan).
//!
//! A tick (`updateEntity`, 56-152): with its blade it takes heat from the source below (`tryPullHeat`, 202-216); at
//! 100 heat or more the input saws, `heat / 10` progress a tick up to 600; over 300 heat for more than 300 ticks the
//! blade breaks with a blast and flies out (the server publishes both, `Server::sawmill_finish`). The blade's hurt to
//! living entities in its box (`turbofan` 100) needs the entity API: a nominal gap.
//!
//! Its three slots are its block entity's (`Layout::Native`), open to hoppers through every face as the Java's
//! `getAccessibleSlotsFromSide` (`{0, 1, 2}` on all sides): [`allows`] is `isItemValidForSlot` and `canExtractItem`.
//! The Java's extra ports (`MachineSawmill.getAllPorts` / `makeExtra`, the four cells beside the core of its 3x3x2,
//! at its height) need the multiblock, which the port's one-block sawmill does not have yet: a nominal gap. In the Java
//! a hopper reaches the slots through those four ports and from below the core; the top layer's cells have no
//! inventory. The one block is open on its top face too, where a hopper above feeds it (the in-game proof's): a
//! divergence of the gap, gone with the multiblock.
use crate::machine::Logic;
use crate::native::SlotAction;
use crate::server::container_bridge::{Slots, Slotted, Stack};
use crate::view::ViewModel;
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_sawmill";
pub const BLADE: &str = "ntm:sawblade";
pub const SAWDUST: &str = "ntm:powder_sawdust";
const STICK: &str = "minecraft:stick";
/// `processingTime`.
pub const PROCESSING_TIME: i32 = 600;
/// `heat > 300`: the overspeed threshold, and the count of hot ticks after which the blade breaks.
const OVERSPEED: i32 = 300;
const BREAK_AFTER: u32 = 300;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sawmill {
    /// `hasBlade`: a new sawmill has its blade; its item without one (the Java's damage 1, here
    /// `minecraft:custom_data {"ntm:no_blade":1b}`) places one without (`MachineSawmill.onBlockPlacedBy`).
    #[serde(default = "with_blade")]
    pub has_blade: bool,
    #[serde(default)]
    pub progress: i32,
    /// `slots`: 0 the input, 1 its output, 2 the sawdust. The block entity's are the truth, copied in before the
    /// machines run (`Slotted`): the record no longer writes them, and an older record that did hands them to empty
    /// block entity slots once (`migrates`).
    #[serde(default = "empty_slots", skip_serializing)]
    pub slots: Slots,
    /// Inputs sawn: the sawdust's dice, as the crafter's chance outputs (a fixed hash instead of the world's random).
    #[serde(default)]
    pub done: u64,
    #[serde(skip)]
    pub heat: i32,
    #[serde(skip)]
    pub overspeed: u32,
    /// Its jump to 100 plays `hbm:block.warnOverspeed` (crate::sounds, as the Stirling's).
    #[serde(skip)]
    pub warn_cooldown: u32,
    /// Whether its last tick sawed (blade and 100 heat): what its state shows after `finish` spent the heat.
    #[serde(skip)]
    pub spinning: bool,
}

fn with_blade() -> bool {
    true
}

fn empty_slots() -> Vec<Option<Stack>> {
    vec![None; 3]
}

impl Default for Sawmill {
    fn default() -> Self {
        Self {
            has_blade: true,
            progress: 0,
            slots: empty_slots(),
            done: 0,
            heat: 0,
            overspeed: 0,
            warn_cooldown: 0,
            spinning: false,
        }
    }
}

/// `getOutput` (226-257): by the input's OreDict names, in the Java's order.
pub fn output(input: &str) -> Option<(String, u32)> {
    let has = |name: &str| crate::oredict::has(input, name);
    if has("stickWood") {
        return Some((SAWDUST.into(), 1));
    }
    if has("logWood") {
        // The 1x1 crafting recipe of a log, wood, stem or hyphae: 4 planks of its wood, `* 6 / 4`.
        return planks_of(input).map(|p| (p, 4 * 6 / 4));
    }
    if has("plankWood") {
        return Some((STICK.into(), 6));
    }
    if has("treeSapling") {
        return Some((STICK.into(), 1));
    }
    None
}

/// The Java's rules for hoppers (`TileEntitySawmill`, 222-229), for every face: `isItemValidForSlot`, the input slot
/// only, with the three slots empty, a stack of one and an output for it; `canExtractItem`, the outputs only. The same
/// for a hand (the Java has no menu, and its uses by hand are `Server::native_sawmill_use`'s).
pub fn allows(slots: &[Option<Stack>], slot: usize, item: &Stack, action: SlotAction) -> bool {
    match action {
        SlotAction::Insert => {
            slot == 0 && slots.iter().all(Option::is_none) && item.count == 1 && output(&item.id).is_some()
        }
        SlotAction::Extract => slot > 0,
    }
}

/// The planks the 1x1 recipe of a log gives (`minecraft:stripped_oak_wood` -> `minecraft:oak_planks`).
fn planks_of(log: &str) -> Option<String> {
    let (ns, id) = log.split_once(':')?;
    let id = id.strip_prefix("stripped_").unwrap_or(id);
    let wood = ["_log", "_wood", "_stem", "_hyphae"].iter().find_map(|s| id.strip_suffix(s))?;
    let planks = format!("{ns}:{wood}_planks");
    crate::oredict::has(&planks, "plankWood").then_some(planks)
}

impl Sawmill {
    pub fn slot(&self, i: usize) -> Option<&Stack> {
        self.slots.get(i).and_then(Option::as_ref)
    }

    /// `tryPullHeat`, only with the blade: a tenth of the source below, truncated; with none taken the heat it has
    /// cools by `max(heat / 1000, 1)`. Returns the heat taken.
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        if !self.has_blade {
            return 0;
        }
        let n = (f64::from(source.unwrap_or(0)) * 0.1) as i32;
        if n > 0 {
            self.heat = self.heat.wrapping_add(n);
            return n as u32;
        }
        self.heat = (self.heat - (self.heat / 1000).max(1)).max(0);
        0
    }

    /// Whether this tick's `finish` breaks the blade (`overspeed > 300`, 128-147): the step that explodes and throws
    /// the blade, so it goes out through the server's post-phase publisher with both.
    pub fn fails_on_finish(&self) -> bool {
        self.has_blade && self.heat > OVERSPEED && self.overspeed.saturating_add(1) > BREAK_AFTER
    }

    /// Whether this tick's `finish` saws its input (`progress >= processingTime`, 73-88): the step that changes its
    /// slots, which are the block entity's, so it goes out through the same publisher, committed with them.
    pub fn saws_on_finish(&self) -> bool {
        self.has_blade
            && self.heat >= 100
            && self.slot(0).is_some_and(|s| output(&s.id).is_some())
            && self.progress + self.heat / 10 >= PROCESSING_TIME
    }

    /// The rest of `updateEntity` after `tryPullHeat`: sawing, the overspeed count and its warning, the blade lost
    /// past it; the heat is spent at the end of every tick.
    pub fn finish(&mut self) {
        if self.has_blade {
            self.warn_cooldown = self.warn_cooldown.saturating_sub(1);
            if self.heat >= 100 {
                match self.slot(0).and_then(|s| output(&s.id)) {
                    Some((item, count)) => {
                        self.progress += self.heat / 10;
                        if self.progress >= PROCESSING_TIME {
                            self.progress = 0;
                            self.slots[0] = None;
                            self.slots[1] = Some(Stack::new(&item, count));
                            if item != SAWDUST {
                                let chance = if item == STICK { 0.1 } else { 0.5 };
                                self.done += 1;
                                let roll = crate::explosion::crater::rand01(self.done as i32, 0, (self.done >> 31) as i32, 91);
                                if roll < chance {
                                    self.slots[2] = Some(Stack::new(SAWDUST, 1));
                                }
                            }
                        }
                    }
                    None => self.progress = 0,
                }
            } else {
                self.progress = 0;
            }
            if self.heat > OVERSPEED {
                self.overspeed += 1;
                if self.overspeed > 60 && self.warn_cooldown == 0 {
                    self.warn_cooldown = 100;
                }
                if self.overspeed > BREAK_AFTER {
                    self.has_blade = false;
                }
            } else {
                self.overspeed = 0;
            }
        } else {
            self.overspeed = 0;
            self.warn_cooldown = 0;
        }
        self.spinning = self.has_blade && self.heat >= 100;
        self.heat = 0;
    }
}

impl Logic for Sawmill {
    /// 1 while it saws (blade and heat), as the Java's spin: its last tick's, the heat being spent by then.
    fn level(&self, max: u8) -> u8 {
        u8::from(self.spinning).min(max)
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Sawmill")
    }
    /// The three slots, dropped when it is broken (`TileEntityMachineBase`): the engine spills them once the bridge
    /// has seen them; these only before.
    fn contents(&self) -> Vec<(String, u32)> {
        self.slots.iter().flatten().map(|s| (s.id.clone(), s.count)).collect()
    }
}

impl Slotted for Sawmill {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// No menu (the Java has none).
    fn screen_data(&self) -> Vec<i32> {
        Vec::new()
    }
    /// Always its three slots, whatever the block entity had (one from before its container has none).
    fn sync_in(&mut self) {
        self.slots.resize(3, None);
    }
    fn migrates(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }
    fn slots_recorded(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hot(s: &mut Sawmill, source: u32, ticks: u32) {
        for _ in 0..ticks {
            s.pull_heat(Some(source));
            s.finish();
        }
    }

    #[test]
    fn a_new_sawmill_has_its_blade_and_an_old_record_keeps_what_it_said() {
        assert!(Sawmill::default().has_blade);
        let without: Sawmill = serde_json::from_str(r#"{"has_blade":false,"progress":7}"#).unwrap();
        assert!(!without.has_blade && without.progress == 7 && without.slots == empty_slots());
        let bare: Sawmill = serde_json::from_str("{}").unwrap();
        assert!(bare.has_blade, "a record without the field keeps the blade, as a new sawmill");
        let mut s = Sawmill { heat: 400, overspeed: 120, warn_cooldown: 30, ..Default::default() };
        s.slots[0] = Some(Stack::new("minecraft:oak_log", 1));
        let back: Sawmill = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!((back.heat, back.overspeed, back.warn_cooldown), (0, 0, 0), "heat, overspeed and the warning are not saved");
        assert_eq!(back.slots, empty_slots(), "the slots are the block entity's");
        assert!(!back.migrates());
        // A record from before the container wrote them: they go to the empty block entity once.
        let old: Sawmill = serde_json::from_str(r#"{"slots":[{"id":"minecraft:oak_log","count":1},null,null]}"#).unwrap();
        assert!(old.migrates() && old.slot(0) == Some(&Stack::new("minecraft:oak_log", 1)));
        let mut none = Sawmill { slots: Vec::new(), ..Default::default() };
        none.sync_in();
        assert_eq!(none.slots, empty_slots(), "a block entity without slots gives it its three");
    }

    /// `isItemValidForSlot` and `canExtractItem` (222-229): one input of one item with an output into an empty
    /// sawmill; the outputs out; through any face (`getAccessibleSlotsFromSide`).
    #[test]
    fn hoppers_follow_the_javas_slot_rules() {
        let log = Stack::new("minecraft:oak_log", 1);
        let empty = empty_slots();
        assert!(allows(&empty, 0, &log, SlotAction::Insert));
        assert!(!allows(&empty, 0, &Stack::new("minecraft:oak_log", 2), SlotAction::Insert), "a stack of one only");
        assert!(!allows(&empty, 0, &Stack::new("minecraft:stone", 1), SlotAction::Insert), "no output");
        assert!(!allows(&empty, 1, &log, SlotAction::Insert) && !allows(&empty, 2, &log, SlotAction::Insert));
        for busy in 0..3 {
            let mut slots = empty_slots();
            slots[busy] = Some(Stack::new(SAWDUST, 1));
            assert!(!allows(&slots, 0, &log, SlotAction::Insert), "slot {busy} taken");
        }
        assert!(!allows(&empty, 0, &log, SlotAction::Extract));
        assert!(allows(&empty, 1, &log, SlotAction::Extract) && allows(&empty, 2, &log, SlotAction::Extract));
        let c = crate::native::container(BLOCK).unwrap();
        assert_eq!((c.slots, c.inputs, c.outputs, c.menu), (3, vec![0], vec![1, 2], None));
        assert!(c.faces.iter().all(|f| f == &vec![0, 1, 2]));
        let rules = crate::native::slot_filters(BLOCK);
        assert_eq!(rules.iter().map(|r| r.output).collect::<Vec<_>>(), vec![false, true, true]);
        assert!(rules.iter().all(|r| r.accept.is_none() && r.max.is_none()), "the rule is `allows`, not a list");
        use crate::native::{SlotOrigin::*, slot_allows};
        for origin in [Manual, Automation] {
            assert!(slot_allows(BLOCK, &empty, 0, &log, SlotAction::Insert, origin));
            assert!(!slot_allows(BLOCK, &empty, 0, &Stack::new("minecraft:oak_log", 2), SlotAction::Insert, origin));
            assert!(!slot_allows(BLOCK, &empty, 0, &log, SlotAction::Extract, origin));
        }
    }

    #[test]
    fn the_outputs_follow_the_java_by_oredict_name() {
        assert_eq!(output("minecraft:oak_log"), Some(("minecraft:oak_planks".into(), 6)));
        assert_eq!(output("minecraft:stripped_cherry_wood"), Some(("minecraft:cherry_planks".into(), 6)));
        assert_eq!(output("minecraft:crimson_hyphae"), Some(("minecraft:crimson_planks".into(), 6)));
        assert_eq!(output("minecraft:spruce_planks"), Some((STICK.into(), 6)));
        assert_eq!(output("minecraft:stick"), Some((SAWDUST.into(), 1)));
        assert_eq!(output("minecraft:birch_sapling"), Some((STICK.into(), 1)));
        assert_eq!(output("minecraft:stone"), None);
        for log in crate::oredict::items("logWood") {
            assert!(output(log).is_some(), "{log} has its planks");
        }
    }

    #[test]
    fn heat_saws_an_input_in_600_progress_and_spends_it() {
        let mut s = Sawmill::default();
        s.slots[0] = Some(Stack::new("minecraft:oak_log", 1));
        // A firebox at 2000: 200 a tick, 20 progress a tick, 30 ticks.
        hot(&mut s, 2000, 29);
        assert_eq!((s.progress, s.slot(1)), (580, None));
        hot(&mut s, 2000, 1);
        assert_eq!(s.progress, 0);
        assert_eq!(s.slot(0), None);
        assert_eq!(s.slot(1), Some(&Stack::new("minecraft:oak_planks", 6)));
        assert_eq!(s.done, 1);
        assert_eq!((s.heat, s.level(1)), (0, 1), "the heat spent, its state still spins (the review's P3)");
        // Under 100 heat nothing saws and the progress drops.
        let mut cold = Sawmill::default();
        cold.slots[0] = Some(Stack::new("minecraft:oak_log", 1));
        hot(&mut cold, 990, 5);
        assert_eq!((cold.progress, cold.level(1)), (0, 0));
        assert!(cold.slot(1).is_none());
    }

    /// The input keeps its whole stack while it saws (a name, `custom`): only the step that ends the sawing takes it.
    #[test]
    fn an_input_with_data_of_its_own_keeps_it_while_it_saws() {
        let named = Stack { custom: Some("{name:1b}".into()), ..Stack::new("minecraft:oak_log", 1) };
        let mut s = Sawmill::default();
        s.slots[0] = Some(named.clone());
        hot(&mut s, 2000, 29);
        assert_eq!((s.progress, s.slot(0)), (580, Some(&named)));
        let back: Sawmill = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.progress, 580, "its progress is saved; its slots are the block entity's");
    }

    #[test]
    fn the_sawdust_dice_are_about_the_javas_chances() {
        let mut planks = 0;
        let mut sticks = 0;
        for done in 1..=2000u64 {
            let roll = crate::explosion::crater::rand01(done as i32, 0, (done >> 31) as i32, 91);
            planks += u32::from(roll < 0.5);
            sticks += u32::from(roll < 0.1);
        }
        assert!((900..1100).contains(&planks), "{planks}");
        assert!((150..250).contains(&sticks), "{sticks}");
    }

    #[test]
    fn overspeed_warns_after_60_and_breaks_after_300_hot_ticks() {
        let mut s = Sawmill::default();
        // 3010 below: 301 heat a tick.
        for t in 1..=300u32 {
            s.pull_heat(Some(3010));
            assert!(!s.fails_on_finish(), "tick {t}");
            s.finish();
            assert_eq!(s.warn_cooldown == 100, t == 61 || t == 161 || t == 261, "tick {t}");
        }
        assert!(s.has_blade);
        s.pull_heat(Some(3010));
        assert!(s.fails_on_finish());
        s.finish();
        assert!(!s.has_blade && s.overspeed == 301);
        // Without a blade it takes no heat and counts nothing.
        assert_eq!(s.pull_heat(Some(3010)), 0);
        s.finish();
        assert!(!s.fails_on_finish() && s.overspeed == 0);
        // A cool tick resets the count, as the Java's `overspeed = 0`.
        let mut c = Sawmill::default();
        hot(&mut c, 3010, 200);
        hot(&mut c, 3000, 1);
        assert_eq!(c.overspeed, 0);
    }
}
